//! YouTube pins off the drawing thread: one worker resolves pasted links into pins and keeps
//! each pin's latest uploads, their thumbnails and the channels' pictures in a cache on the TV.
//! Only YouTube's public pages and feeds are asked; nothing is signed in.
use crate::{
    model::{Pin, PinKind},
    youtube::{self, Link},
};
use image::{ImageReader, imageops::FilterType};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

/// Each pin's newest this many uploads: a shelf row's worth.
const PER_PIN: usize = 8;
/// A card's picture: twice the shelf card, sharp on the 4K panel's 2x scale.
const CARD: (u32, u32) = (768, 432);
const AVATAR: u32 = 256;

#[derive(Debug, PartialEq)]
pub enum Command {
    /// A link pasted on the phone.
    Resolve(String),
    /// Bring every pin's uploads up to date (and drop the pictures no pin shows any more).
    Refresh(Vec<Pin>),
}

/// One upload on the shelf.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub video: String,
    pub title: String,
    pub published: String,
    /// Its thumbnail on the TV, once fetched.
    pub image: Option<String>,
}

pub enum Response {
    Resolved(Result<Pin, String>),
    /// A pin's latest uploads (a video pin: itself), and its channel's picture.
    Shelf {
        pin: String,
        cards: Vec<Card>,
        avatar: Option<String>,
    },
}

struct Client {
    agent: ureq::Agent,
    cache: PathBuf,
}

impl Client {
    fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        let mut response = self
            .agent
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (Lounge)")
            .header("Accept-Language", "en")
            .call()
            .map_err(|_| "YouTube is unavailable. Try again shortly.")?;
        if !response.status().is_success() {
            return Err("YouTube did not find that link.".into());
        }
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "YouTube's answer could not be read")?;
        if bytes.len() > limit {
            return Err("YouTube's answer was too large".into());
        }
        Ok(bytes)
    }

    fn text(&self, url: &str, limit: usize) -> Result<String, String> {
        String::from_utf8(self.get(url, limit)?).map_err(|_| "YouTube's answer was not text".into())
    }

    fn feed(&self, link: &Link) -> Result<youtube::Feed, String> {
        let url = youtube::feed_url(link).ok_or("Only channels and playlists have feeds")?;
        youtube::parse_feed(&self.text(&url, 2 * 1024 * 1024)?)
    }

    /// A channel's id, name and picture from its page.
    fn channel(&self, path: &str) -> Result<(String, String, Option<String>), String> {
        let page = self.text(&format!("https://www.youtube.com/{path}"), 6 * 1024 * 1024)?;
        youtube::parse_channel_page(&page).ok_or_else(|| "That is not a YouTube channel.".into())
    }

    fn resolve(&self, text: &str) -> Result<Pin, String> {
        // A JioHotstar show needs nothing fetched: its link is its pin.
        if let Some(path) = crate::hotstar::parse_show(text) {
            return Ok(Pin {
                kind: PinKind::Show,
                title: crate::hotstar::title_from_path(&path),
                id: path,
                avatar: None,
            });
        }
        let link = youtube::parse_link(text)
            .ok_or("Paste a link to a YouTube channel, playlist or video, or a JioHotstar show.")?;
        Ok(match link {
            Link::Handle(handle) => {
                let (id, title, avatar) = self.channel(&format!("@{handle}"))?;
                Pin {
                    kind: PinKind::Channel,
                    id,
                    title,
                    avatar,
                }
            }
            Link::Channel(id) => {
                let (id, title, avatar) = self.channel(&format!("channel/{id}"))?;
                Pin {
                    kind: PinKind::Channel,
                    id,
                    title,
                    avatar,
                }
            }
            link @ Link::Playlist(_) => {
                let title = self.feed(&link)?.title;
                let Link::Playlist(id) = link else {
                    unreachable!()
                };
                Pin {
                    kind: PinKind::Playlist,
                    id,
                    title,
                    avatar: None,
                }
            }
            Link::Video(id) => {
                // YouTube's oEmbed answer names a video and its channel without a page load.
                let url = format!(
                    "https://www.youtube.com/oembed?format=json&url=https://www.youtube.com/watch?v={id}"
                );
                let reply: serde_json::Value = serde_json::from_slice(&self.get(&url, 64 * 1024)?)
                    .map_err(|_| "YouTube's answer could not be read")?;
                let title = reply["title"]
                    .as_str()
                    .ok_or("YouTube did not find that video.")?
                    .to_owned();
                Pin {
                    kind: PinKind::Video,
                    id,
                    title,
                    avatar: None,
                }
            }
        })
    }

    /// A picture fetched once, scaled to cover `size` and kept as JPEG; None if it cannot be.
    fn picture(&self, name: &str, urls: &[String], size: (u32, u32)) -> Option<String> {
        let path = self.cache.join(name);
        if path.is_file() {
            return path.to_str().map(str::to_owned);
        }
        let bytes = urls
            .iter()
            .find_map(|url| self.get(url, 4 * 1024 * 1024).ok())?;
        let mut reader = ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .ok()?
            .resize_to_fill(size.0, size.1, FilterType::Lanczos3)
            .to_rgb8();
        let partial = path.with_extension("partial");
        let file = fs::File::create(&partial).ok()?;
        let mut out = std::io::BufWriter::new(file);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
            .encode_image(&image)
            .ok()?;
        drop(out);
        fs::rename(&partial, &path).ok()?;
        path.to_str().map(str::to_owned)
    }

    fn thumbnail(&self, video: &str) -> Option<String> {
        self.picture(
            &format!("{video}.jpg"),
            &[
                youtube::thumbnail(video, true),
                youtube::thumbnail(video, false),
            ],
            CARD,
        )
    }

    fn row(&self, pin: &Pin) -> Result<(Vec<Card>, Option<String>), String> {
        let cards = match pin.kind {
            // Its own card: the show, opened on its page.
            PinKind::Show => vec![Card {
                video: pin.id.clone(),
                title: pin.title.clone(),
                published: String::new(),
                image: None,
            }],
            PinKind::Video => vec![Card {
                video: pin.id.clone(),
                title: pin.title.clone(),
                published: String::new(),
                image: self.thumbnail(&pin.id),
            }],
            PinKind::Channel | PinKind::Playlist => {
                let link = if pin.kind == PinKind::Channel {
                    Link::Channel(pin.id.clone())
                } else {
                    Link::Playlist(pin.id.clone())
                };
                self.feed(&link)?
                    .videos
                    .into_iter()
                    .take(PER_PIN)
                    .map(|video| Card {
                        image: self.thumbnail(&video.id),
                        video: video.id,
                        title: video.title,
                        published: video.published,
                    })
                    .collect()
            }
        };
        let avatar = pin.avatar.as_ref().and_then(|url| {
            self.picture(
                &format!("avatar-{}.jpg", pin.id),
                std::slice::from_ref(url),
                (AVATAR, AVATAR),
            )
        });
        Ok((cards, avatar))
    }
}

/// The folder the pictures are kept in, beside the preferences.
pub fn cache_folder(storage: &Path) -> PathBuf {
    storage.join("youtube")
}

pub struct Service {
    commands: Option<mpsc::Sender<Command>>,
    pub receiver: mpsc::Receiver<Response>,
}

impl Service {
    pub fn new(storage: &Path) -> Self {
        let (commands, jobs) = mpsc::channel::<Command>();
        let (sender, receiver) = mpsc::channel();
        let cache = cache_folder(storage);
        thread::spawn(move || {
            let _ = fs::create_dir_all(&cache);
            let client = Client {
                agent: ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(15)))
                    .max_redirects(3)
                    .build()
                    .new_agent(),
                cache,
            };
            while let Ok(mut job) = jobs.recv() {
                // Refreshes asked for meanwhile collapse into the newest.
                let mut resolves = Vec::new();
                while let Ok(next) = jobs.try_recv() {
                    match next {
                        Command::Resolve(link) => resolves.push(Command::Resolve(link)),
                        refresh @ Command::Refresh(_) => job = refresh,
                    }
                }
                for job in std::iter::once(job).chain(resolves) {
                    let sent = match job {
                        Command::Resolve(link) => crate::wake::sent(
                            sender.send(Response::Resolved(client.resolve(&link))),
                        ),
                        Command::Refresh(pins) => {
                            let mut kept = HashSet::new();
                            let mut sent = Ok(());
                            let mut failed = false;
                            for pin in &pins {
                                let Ok((cards, avatar)) = client.row(pin) else {
                                    // Unreachable now: the last good row stays on the shelf,
                                    // pictures and all.
                                    failed = true;
                                    continue;
                                };
                                kept.extend(cards.iter().map(|card| format!("{}.jpg", card.video)));
                                kept.insert(format!("avatar-{}.jpg", pin.id));
                                sent = crate::wake::sent(sender.send(Response::Shelf {
                                    pin: pin.key(),
                                    cards,
                                    avatar,
                                }));
                                if sent.is_err() {
                                    break;
                                }
                            }
                            // Pictures no pin shows any more go, so the cache never grows.
                            if sent.is_ok()
                                && !failed
                                && let Ok(entries) = fs::read_dir(&client.cache)
                            {
                                for entry in entries.flatten() {
                                    let name = entry.file_name();
                                    if !kept.contains(name.to_string_lossy().as_ref()) {
                                        let _ = fs::remove_file(entry.path());
                                    }
                                }
                            }
                            sent
                        }
                    };
                    if sent.is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            commands: Some(commands),
            receiver,
        }
    }

    pub fn send(&self, command: Command) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against YouTube itself: `cargo test -- --ignored pins_live`.
    #[test]
    #[ignore]
    fn pins_live_a_handle_resolves_and_its_row_comes_with_pictures() {
        let folder = std::env::temp_dir().join(format!("lounge-pins-{}", std::process::id()));
        let service = Service::new(&folder);
        service.send(Command::Resolve(
            "https://www.youtube.com/@etvandhrapradesh".into(),
        ));
        let Response::Resolved(pin) = service.receiver.recv().unwrap() else {
            panic!("expected the pin")
        };
        let pin = pin.unwrap();
        assert_eq!(pin.kind, PinKind::Channel);
        assert_eq!(pin.id, "UCJi8M0hRKjz8SLPvJKEVTOg");
        assert!(pin.avatar.is_some());
        service.send(Command::Refresh(vec![pin.clone()]));
        let Response::Shelf {
            pin: key,
            cards,
            avatar,
        } = service.receiver.recv().unwrap()
        else {
            panic!("expected the row")
        };
        assert_eq!(key, pin.key());
        assert!(!cards.is_empty() && cards.len() <= PER_PIN);
        for card in &cards {
            let image = card.image.as_deref().expect("each card has its picture");
            let decoded = image::open(image).unwrap();
            assert_eq!((decoded.width(), decoded.height()), CARD);
        }
        assert!(avatar.is_some_and(|path| Path::new(&path).is_file()));
        let _ = fs::remove_dir_all(folder);
    }
}
