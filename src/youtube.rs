//! YouTube pins: the links Lounge understands, the public feeds it reads a channel's or a
//! playlist's latest uploads from (no account, no key), and how the TV's YouTube app opens a
//! video. Pure text in, values out: the network is `pins_service`'s.
use serde_json::{Value, json};

/// What a pasted link points at.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    /// A channel by its id (UC…).
    Channel(String),
    /// A channel by its handle (@name), resolved to its id from the channel's page.
    Handle(String),
    Video(String),
    Playlist(String),
}

/// One upload in a feed.
#[derive(Clone, Debug, PartialEq)]
pub struct Video {
    pub id: String,
    pub title: String,
    /// RFC 3339, as the feed gives it.
    pub published: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Feed {
    pub title: String,
    /// Newest first, as YouTube lists them.
    pub videos: Vec<Video>,
}

fn is_id(text: &str, length: std::ops::RangeInclusive<usize>) -> bool {
    length.contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub fn is_video_id(text: &str) -> bool {
    is_id(text, 11..=11)
}

pub fn is_channel_id(text: &str) -> bool {
    text.starts_with("UC") && is_id(text, 24..=24)
}

fn is_playlist_id(text: &str) -> bool {
    is_id(text, 10..=64)
}

fn is_handle(text: &str) -> bool {
    (3..=30).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// The value of `key` in a URL's query.
fn query<'a>(url: &'a str, key: &str) -> Option<&'a str> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
}

/// A link as YouTube's apps share it: youtube.com (www., m., music.) and youtu.be; watch, live,
/// shorts, playlist, channel and @handle pages. A bare @handle or channel id works too.
pub fn parse_link(text: &str) -> Option<Link> {
    let text = text.trim();
    if let Some(handle) = text.strip_prefix('@') {
        return is_handle(handle).then(|| Link::Handle(handle.to_owned()));
    }
    if is_channel_id(text) {
        return Some(Link::Channel(text.to_owned()));
    }
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host
        .trim_start_matches("www.")
        .trim_start_matches("m.")
        .trim_start_matches("music.");
    let path_only = path.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path_only.split('/').filter(|s| !s.is_empty()).collect();
    match host {
        "youtu.be" => segments
            .first()
            .filter(|id| is_video_id(id))
            .map(|id| Link::Video((*id).to_owned())),
        "youtube.com" => {
            match segments.as_slice() {
                ["watch", ..] => {
                    // A video is what was shared, even when it plays in a playlist.
                    query(path, "v")
                        .filter(|id| is_video_id(id))
                        .map(|id| Link::Video(id.to_owned()))
                }
                ["live" | "shorts" | "embed", id, ..] if is_video_id(id) => {
                    Some(Link::Video((*id).to_owned()))
                }
                ["playlist", ..] => query(path, "list")
                    .filter(|id| is_playlist_id(id))
                    .map(|id| Link::Playlist(id.to_owned())),
                ["channel", id, ..] if is_channel_id(id) => Some(Link::Channel((*id).to_owned())),
                [first, ..] => first
                    .strip_prefix('@')
                    .filter(|handle| is_handle(handle))
                    .map(|handle| Link::Handle(handle.to_owned())),
                [] => None,
            }
        }
        _ => None,
    }
}

/// The public Atom feed of a channel's or a playlist's latest uploads.
pub fn feed_url(link: &Link) -> Option<String> {
    match link {
        Link::Channel(id) => Some(format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={id}"
        )),
        Link::Playlist(id) => Some(format!(
            "https://www.youtube.com/feeds/videos.xml?playlist_id={id}"
        )),
        Link::Handle(_) | Link::Video(_) => None,
    }
}

/// A video's 16:9 thumbnail: 1280x720, or the 320x180 one every video has.
pub fn thumbnail(id: &str, large: bool) -> String {
    let name = if large { "hq720" } else { "mqdefault" };
    format!("https://i.ytimg.com/vi/{id}/{name}.jpg")
}

/// The launch parameters with which the TV's YouTube app (and YouTube AdFree) plays `id` at
/// once: its manifest's own `contentTarget`.
pub fn launch_params(id: &str) -> Value {
    json!({ "contentTarget": format!("v={id}") })
}

/// The text inside the first `<tag>…</tag>` from `from`, and where it ends.
fn element<'a>(text: &'a str, tag: &str, from: usize) -> Option<(&'a str, usize)> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text[from..].find(&open)? + from + open.len();
    let end = text[start..].find(&close)? + start;
    Some((&text[start..end], end + close.len()))
}

/// XML's escapes, as YouTube writes them.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|n| n.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A channel's or playlist's feed: its title and its latest uploads, newest first.
pub fn parse_feed(xml: &str) -> Result<Feed, String> {
    let first_entry = xml.find("<entry>").unwrap_or(xml.len());
    let title = element(&xml[..first_entry], "title", 0)
        .map(|(title, _)| unescape(title.trim()))
        .ok_or("The feed has no title")?;
    let mut videos = Vec::new();
    let mut at = first_entry;
    while let Some((entry, end)) = element(xml, "entry", at) {
        at = end;
        let field = |tag| element(entry, tag, 0).map(|(value, _)| value.trim());
        let Some(id) = field("yt:videoId").filter(|id| is_video_id(id)) else {
            continue;
        };
        videos.push(Video {
            id: id.to_owned(),
            title: field("title").map(unescape).unwrap_or_default(),
            published: field("published").unwrap_or_default().to_owned(),
        });
    }
    Ok(Feed { title, videos })
}

/// Whether a title says it is live: worth asking its watch page.
pub fn claims_live(title: &str) -> bool {
    title.contains("LIVE") || title.contains("Live") || title.contains('🔴')
}

/// A watch page's own word on whether it is streaming now.
pub fn page_live_now(page: &str) -> bool {
    page.contains("\"isLiveNow\":true")
}

/// A channel page's id, name and avatar (its `og:` tags and `externalId`), for a pin made from
/// a handle.
pub fn parse_channel_page(html: &str) -> Option<(String, String, Option<String>)> {
    let meta = |property: &str| {
        let marker = format!("<meta property=\"{property}\" content=\"");
        let start = html.find(&marker)? + marker.len();
        let end = html[start..].find('"')? + start;
        Some(unescape(&html[start..end]))
    };
    let marker = "\"externalId\":\"";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"')? + start;
    let id = &html[start..end];
    if !is_channel_id(id) {
        return None;
    }
    let title = meta("og:title")?;
    let avatar = meta("og:image").filter(|url| url.starts_with("https://"));
    Some((id.to_owned(), title, avatar))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_as_youtube_shares_them() {
        let video = |id: &str| Some(Link::Video(id.into()));
        assert_eq!(
            parse_link("https://www.youtube.com/watch?v=kJqrDRnWA9s&t=4s"),
            video("kJqrDRnWA9s")
        );
        assert_eq!(
            parse_link("https://youtu.be/kJqrDRnWA9s?si=abc"),
            video("kJqrDRnWA9s")
        );
        assert_eq!(
            parse_link("https://m.youtube.com/live/kJqrDRnWA9s?feature=share"),
            video("kJqrDRnWA9s")
        );
        assert_eq!(
            parse_link("youtube.com/shorts/kJqrDRnWA9s"),
            video("kJqrDRnWA9s")
        );
        assert_eq!(
            parse_link("https://www.youtube.com/@etvandhrapradesh"),
            Some(Link::Handle("etvandhrapradesh".into()))
        );
        assert_eq!(
            parse_link("https://www.youtube.com/@etvandhrapradesh/videos"),
            Some(Link::Handle("etvandhrapradesh".into()))
        );
        assert_eq!(
            parse_link("@ETVTelangana"),
            Some(Link::Handle("ETVTelangana".into()))
        );
        assert_eq!(
            parse_link("https://www.youtube.com/channel/UCJi8M0hRKjz8SLPvJKEVTOg"),
            Some(Link::Channel("UCJi8M0hRKjz8SLPvJKEVTOg".into()))
        );
        assert_eq!(
            parse_link("https://www.youtube.com/playlist?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG"),
            Some(Link::Playlist("PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG".into()))
        );
        // Not YouTube, or not a link to anything.
        for text in [
            "https://example.com/watch?v=kJqrDRnWA9s",
            "https://www.youtube.com/",
            "https://www.youtube.com/watch?v=short",
            "hello",
            "",
        ] {
            assert_eq!(parse_link(text), None, "{text}");
        }
    }

    #[test]
    fn a_channel_feed_reads_newest_first_with_its_titles() {
        let feed = parse_feed(include_str!("../tests/fixtures/youtube-feed.xml")).unwrap();
        assert_eq!(feed.title, "ETV Andhra Pradesh");
        assert_eq!(feed.videos.len(), 3);
        assert_eq!(feed.videos[0].id, "kJqrDRnWA9s");
        assert!(feed.videos[0].title.contains("చంద్రబాబు"));
        assert_eq!(feed.videos[0].published, "2026-11-27T07:53:51+00:00");
        assert!(feed.videos[0].published > feed.videos[1].published);
    }

    #[test]
    fn escapes_in_titles_come_out_as_text() {
        assert_eq!(
            unescape("Tom &amp; Jerry &#39;live&#39; &lt;3 &#x1F534;"),
            "Tom & Jerry 'live' <3 🔴"
        );
        assert_eq!(unescape("a & b &bogus;"), "a & b &bogus;");
    }

    #[test]
    fn a_handle_page_gives_the_channel_s_id_name_and_avatar() {
        let page = r#"<meta property="og:title" content="ETV Andhra Pradesh"><meta property="og:image" content="https://yt3.googleusercontent.com/x=s900"> ... "externalId":"UCJi8M0hRKjz8SLPvJKEVTOg" ..."#;
        assert_eq!(
            parse_channel_page(page),
            Some((
                "UCJi8M0hRKjz8SLPvJKEVTOg".into(),
                "ETV Andhra Pradesh".into(),
                Some("https://yt3.googleusercontent.com/x=s900".into())
            ))
        );
        assert_eq!(parse_channel_page("<html>nothing</html>"), None);
    }

    #[test]
    fn only_the_watch_page_says_a_stream_is_live_now() {
        assert!(claims_live("దిల్లీకి సీఎం చంద్రబాబు :🔴LIVE"));
        assert!(!claims_live("Weekend news roundup"));
        // An ended stream keeps LIVE in its title; its page says otherwise.
        assert!(!page_live_now(
            r#"..."isLiveContent":true,..."isLiveNow":false..."#
        ));
        assert!(page_live_now(
            r#"..."isLiveContent":true,..."isLiveNow":true..."#
        ));
    }

    #[test]
    fn the_youtube_app_opens_a_video_from_its_content_target() {
        assert_eq!(
            launch_params("kJqrDRnWA9s"),
            json!({ "contentTarget": "v=kJqrDRnWA9s" })
        );
        assert_eq!(
            feed_url(&Link::Channel("UCJi8M0hRKjz8SLPvJKEVTOg".into())).unwrap(),
            "https://www.youtube.com/feeds/videos.xml?channel_id=UCJi8M0hRKjz8SLPvJKEVTOg"
        );
        assert!(thumbnail("kJqrDRnWA9s", true).ends_with("/vi/kJqrDRnWA9s/hq720.jpg"));
    }
}
