//! YouTube pins in the state: each pin's row of latest uploads for the shelf, when they are
//! due a refresh, and what a resolved link does. The fetching is `pins_service`'s.
use super::*;
use crate::pins_service::{Card, Command, Response};

/// How often the uploads are checked while Home is on screen: news channels post through the
/// day, and an hour-old "latest" would not be.
pub const PINS_REFRESH: Duration = Duration::from_secs(15 * 60);
/// After a refresh that could not reach YouTube: soon again. Waking, the TV takes a few seconds
/// to join the network, and the first refresh came too early; the next was 15 minutes away.
pub const PINS_RETRY: Duration = Duration::from_secs(30);
/// With nobody pressing anything for this long (asleep under the screen saver, say), the pins
/// are not refreshed: no one sees them. The next press refreshes them at once.
pub const PINS_IDLE: Duration = Duration::from_secs(30 * 60);

/// One pin's row: its latest uploads, newest first, and its channel's picture on the TV.
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfRow {
    pub cards: Vec<Card>,
    pub avatar: Option<String>,
    /// When YouTube last answered for it: offline, the row stays, but not its LIVE claims.
    pub fetched: std::time::Instant,
}

/// How long a row's LIVE badge holds without a fresh answer. Refreshes come every quarter hour
/// while Home is used; past this, offline, the stream has likely ended and the badge would lie.
const LIVE_HOLDS: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// What a pin's alert offers. The first pin is already at the front, so it has no Move to Front:
/// a button that does nothing reads as broken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnpinChoice {
    Front,
    Remove,
    Cancel,
}

/// A card on the shelf: a pin's newest upload (a video pin: itself).
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfCard<'a> {
    pub pin: &'a crate::model::Pin,
    pub card: &'a Card,
    pub avatar: Option<&'a str>,
    /// On air as far as a recent answer says (`LIVE_HOLDS`).
    pub live: bool,
    /// Uploaded in the last `NEW_FOR` and not opened from the shelf yet.
    pub new: bool,
}

/// How long an upload nobody has opened reads NEW.
const NEW_FOR: i64 = 36 * 3600;
/// A habit fades by half every this many days: what the family watches now outweighs what it
/// watched last season.
const HALF_LIFE_DAYS: f64 = 21.;
/// How far from the hour it was watched at a habit still counts (minutes, one deviation).
const HABIT_SPREAD: f64 = 75.;

/// The pins' keys, what is usually opened around `minute` (local minute of the day) first: each
/// open counts by how near its time of day was (a bell over the day, wrapping at midnight) and
/// how recent it was, plus a little for any time of day. Pins never opened keep their own order
/// after those that were, so a new family sees the shelf just as pinned.
pub fn rank_pins(
    pins: &[crate::model::Pin],
    watched: &[crate::model::Watch],
    now: i64,
    minute: u16,
) -> Vec<String> {
    let score = |key: &str| -> f64 {
        watched
            .iter()
            .filter(|watch| watch.pin == key)
            .map(|watch| {
                let days = (now - watch.at).max(0) as f64 / 86_400.;
                let recent = 0.5_f64.powf(days / HALF_LIFE_DAYS);
                let apart = (i32::from(watch.minute) - i32::from(minute)).rem_euclid(1440);
                let apart = f64::from(apart.min(1440 - apart)) / HABIT_SPREAD;
                recent * (0.15 + (-apart * apart / 2.).exp())
            })
            .sum()
    };
    let mut ranked: Vec<(String, f64)> = pins
        .iter()
        .map(|pin| {
            let key = pin.key();
            let score = score(&key);
            (key, score)
        })
        .collect();
    // Stable: equal scores (none, mostly) keep the pins' order.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked.into_iter().map(|(key, _)| key).collect()
}

impl State {
    /// The dock app the shelf is about: the one it was opened from, or the focused one.
    pub fn shelf_app(&self) -> Option<&str> {
        // Every frame asks: with nothing pinned (most TVs) it costs nothing.
        if self.preferences.pins.is_empty() {
            return None;
        }
        // In its cards, by arrows or by the Magic Remote's pointer, the shelf stays its app's.
        if self.shelf_open || matches!(self.focus, Target::Card(_)) {
            return self.shelf_owner.as_deref();
        }
        let Target::App(id) = &self.focus else {
            return None;
        };
        self.apps()
            .iter()
            .take(APPS_COLUMNS)
            .any(|app| &app.id == id)
            .then_some(id.as_str())
    }

    /// The shelf, in pin order: the shelf app's pins, each with its newest upload (a show: its
    /// own card).
    pub fn shelf_cards(&self) -> Vec<ShelfCard<'_>> {
        let Some(app) = self.shelf_app() else {
            return vec![];
        };
        let mut pins: Vec<&crate::model::Pin> = self
            .preferences
            .pins
            .iter()
            .filter(|pin| pin.app() == app)
            .collect();
        if self.preferences.personal_order {
            let place = |pin: &crate::model::Pin| {
                let key = pin.key();
                self.shelf_order
                    .iter()
                    .position(|ranked| *ranked == key)
                    .unwrap_or(usize::MAX)
            };
            pins.sort_by_key(|pin| place(pin));
        }
        let now = self.local_now.map(time::OffsetDateTime::unix_timestamp);
        pins.into_iter()
            .filter_map(|pin| {
                let row = self.shelf.get(&pin.key())?;
                let card = row.cards.first()?;
                let live = card.live && crate::clock::since(row.fetched) < LIVE_HOLDS;
                let fresh = now
                    .zip(
                        time::OffsetDateTime::parse(
                            &card.published,
                            &time::format_description::well_known::Rfc3339,
                        )
                        .ok(),
                    )
                    .is_some_and(|(now, at)| (0..NEW_FOR).contains(&(now - at.unix_timestamp())));
                let opened = self
                    .preferences
                    .watched
                    .iter()
                    .any(|watch| watch.video == card.video);
                Some(ShelfCard {
                    pin,
                    card,
                    avatar: row.avatar.as_deref(),
                    live,
                    new: !live && fresh && !opened,
                })
            })
            .collect()
    }

    /// The local time, once a minute (and at once when the clock or the time zone changes). The
    /// shelf is ordered again for the new time of day, the next time it is out of sight.
    pub fn set_local_now(&mut self, now: time::OffsetDateTime) {
        // Before the network sets it, a TV's clock reads 1970: no habit is reckoned from that.
        if now.year() < 2024 {
            return;
        }
        let hour_turned = self.local_now.is_none_or(|was| was.hour() != now.hour());
        self.local_now = Some(now);
        if hour_turned {
            self.shelf_order_due = true;
        }
    }

    /// Orders the shelf again when due, while nobody is looking at it (or before it has ever
    /// been ordered).
    fn order_shelf(&mut self) {
        if !self.shelf_order_due || !self.preferences.personal_order {
            return;
        }
        let ordered_before = !self.shelf_order.is_empty();
        if ordered_before && (self.shelf_open || self.shelf_visible()) {
            return;
        }
        let Some(now) = self.local_now else {
            return;
        };
        let minute = u16::from(now.hour()) * 60 + u16::from(now.minute());
        self.shelf_order = rank_pins(
            &self.preferences.pins,
            &self.preferences.watched,
            now.unix_timestamp(),
            minute,
        );
        self.shelf_order_due = false;
        self.dirty = true;
    }

    /// The shelf shows whenever a dock app with pins is focused, as Apple TV's Top Shelf shows
    /// an app's content the moment it is focused: nobody has to know to press Up for it. Up
    /// then moves into its cards.
    pub fn shelf_visible(&self) -> bool {
        self.page == Page::Apps
            && !self.expanded
            && !self.shelf.is_empty()
            && !self.shelf_cards().is_empty()
    }

    /// Notes the shelf appearing or leaving, for the crossfade with the clock (once a frame).
    pub(super) fn track_shelf(&mut self) {
        self.order_shelf();
        // The dock app a shelf shows for is remembered, so a pointer moving onto a card (focus
        // then on the card, not the app) keeps the shelf it saw.
        if !self.preferences.pins.is_empty()
            && let Target::App(id) = &self.focus
            && !self.shelf_open
            && self.shelf_owner.as_ref() != Some(id)
            && self
                .apps()
                .iter()
                .take(APPS_COLUMNS)
                .any(|app| &app.id == id)
        {
            self.shelf_owner = Some(id.clone());
        }
        let visible = self.shelf_visible();
        if visible != self.shelf_was_visible {
            self.shelf_was_visible = visible;
            self.shelf_changed = (!self.preferences.reduced_motion).then(crate::clock::now);
            self.dirty = true;
        }
    }

    /// How far the shelf has come in (0 gone, 1 shown): it and the clock crossfade.
    pub fn shelf_shown(&self) -> f32 {
        let t = self.shelf_changed.map_or(1., |at| {
            (crate::clock::since(at).as_secs_f32() * 1000. / SHELF_FADE_MS).min(1.)
        });
        let eased = t * t * (3. - 2. * t);
        if self.shelf_was_visible {
            eased
        } else {
            1. - eased
        }
    }

    /// Up from a dock app with pins: its shelf, on the first card.
    pub(super) fn open_shelf(&mut self) -> bool {
        if self.shelf_cards().is_empty() {
            return false;
        }
        self.shelf_owner = self.shelf_app().map(str::to_owned);
        self.shelf_open = true;
        self.focus = Target::Card(0);
        self.dirty = true;
        true
    }

    /// Down or Back from the shelf: its app in the dock again.
    pub(super) fn close_shelf(&mut self) {
        self.shelf_open = false;
        self.focus = Target::App(self.shelf_owner.clone().unwrap_or_else(|| YOUTUBE.into()));
        self.dirty = true;
    }

    /// A refresh of every pin, when one is due: at once with Home on screen and none yet,
    /// then every `PINS_REFRESH` while it stays on screen. Behind an app nothing is fetched.
    pub fn pins_due(&mut self, visible: bool) -> Option<Effect> {
        if !visible || self.preferences.pins.is_empty() {
            return None;
        }
        if self
            .last_input
            .is_some_and(|at| crate::clock::since(at) >= PINS_IDLE)
        {
            return None;
        }
        if self
            .pins_refreshed
            .is_some_and(|at| crate::clock::since(at) < PINS_REFRESH)
        {
            return None;
        }
        self.pins_refreshed = Some(crate::clock::now());
        Some(Effect::Pins(Command::Refresh(
            self.preferences.pins.clone(),
        )))
    }

    /// What the pins' worker answered.
    pub fn pins_response(&mut self, response: Response) -> Vec<Effect> {
        match response {
            // Due again after `PINS_RETRY` instead of `PINS_REFRESH`.
            Response::Unreachable => {
                self.pins_refreshed = crate::clock::now().checked_sub(PINS_REFRESH - PINS_RETRY);
                vec![]
            }
            Response::Shelf { pin, cards, avatar } => {
                // A row for a pin removed meanwhile is dropped.
                if self.preferences.pins.iter().any(|kept| kept.key() == pin) {
                    self.shelf.insert(
                        pin,
                        ShelfRow {
                            cards,
                            avatar,
                            fetched: crate::clock::now(),
                        },
                    );
                    self.dirty = true;
                }
                vec![]
            }
            Response::Resolved(Err(error)) => {
                self.notify(error);
                vec![]
            }
            Response::Resolved(Ok(pin)) => {
                let title = pin.title.clone();
                if self
                    .preferences
                    .pins
                    .iter()
                    .any(|kept| kept.key() == pin.key())
                {
                    self.notify(i18n::format("alreadyPinned", &[("title", &title)]));
                    return vec![];
                }
                if self.preferences.pins.len() >= crate::model::MAX_PINS {
                    self.notify(i18n::t("pinsFull"));
                    return vec![];
                }
                self.preferences.pins.push(pin);
                self.notify(i18n::format("pinned", &[("title", &title)]));
                self.pins_refreshed = Some(crate::clock::now());
                vec![
                    Effect::Save,
                    Effect::Pins(Command::Refresh(self.preferences.pins.clone())),
                ]
            }
        }
    }

    /// What OK on a shelf card opens: its video, in the pin's app. The open is remembered, for
    /// the shelf's order (`rank_pins`) and its NEW badges.
    pub(super) fn open_card(&mut self, index: usize) -> Vec<Effect> {
        let Some(card) = self.shelf_cards().get(index).cloned() else {
            return vec![];
        };
        let params = crate::youtube::launch_params(&card.card.video);
        let open = Effect::Open(card.pin.app().into(), params);
        let Some(now) = self.local_now else {
            return vec![open];
        };
        let watch = crate::model::Watch {
            pin: card.pin.key(),
            video: card.card.video.clone(),
            at: now.unix_timestamp(),
            minute: u16::from(now.hour()) * 60 + u16::from(now.minute()),
        };
        let watched = &mut self.preferences.watched;
        watched.push(watch);
        let over = watched.len().saturating_sub(crate::model::MAX_WATCHES);
        watched.drain(..over);
        self.shelf_order_due = true;
        vec![open, Effect::Save]
    }

    /// The remote or the pointer was used.
    pub fn touched(&mut self) {
        self.last_input = Some(crate::clock::now());
    }

    /// Takes a pin off, its row with it.
    /// The pin alert's buttons, top to bottom, for the pin it is open on.
    pub fn unpin_choices(&self) -> &'static [UnpinChoice] {
        let Some(Overlay::Unpin { key, .. }) = &self.overlay else {
            return &[];
        };
        let first = self
            .preferences
            .pins
            .first()
            .is_some_and(|pin| &pin.key() == key);
        // Ordered for the family, the shelf does not follow the pins' order: Move to Front
        // would change nothing anyone sees.
        if first || self.preferences.personal_order {
            &[UnpinChoice::Remove, UnpinChoice::Cancel]
        } else {
            &[UnpinChoice::Front, UnpinChoice::Remove, UnpinChoice::Cancel]
        }
    }

    /// What the pin alert's button at `index` does, while that alert is open.
    pub fn unpin_choice(&self, index: usize) -> Option<UnpinChoice> {
        self.unpin_choices().get(index).copied()
    }

    pub fn unpin(&mut self, key: &str) -> Vec<Effect> {
        let before = self.preferences.pins.len();
        self.preferences.pins.retain(|pin| pin.key() != key);
        if self.preferences.pins.len() == before {
            return vec![];
        }
        self.shelf.remove(key);
        self.preferences.watched.retain(|watch| watch.pin != key);
        self.dirty = true;
        vec![Effect::Save]
    }
}

#[cfg(test)]
mod tests {
    use super::rank_pins;
    use crate::model::{Pin, PinKind, Watch};

    fn pin(id: &str) -> Pin {
        Pin {
            kind: PinKind::Channel,
            id: id.into(),
            title: id.into(),
            avatar: None,
        }
    }
    fn watch(id: &str, days_ago: i64, minute: u16) -> Watch {
        Watch {
            pin: format!("channel:{id}"),
            video: "v".into(),
            at: 100 * 86_400 - days_ago * 86_400,
            minute,
        }
    }

    #[test]
    fn the_shelf_leads_with_what_is_watched_at_this_hour() {
        let pins = [
            pin("news"),
            pin("cartoons"),
            pin("devotional"),
            pin("unwatched"),
        ];
        // News most evenings at 21:00, cartoons after school at 16:30, devotional at 6:30.
        let mut watched = vec![];
        for day in 0..10 {
            watched.push(watch("news", day, 21 * 60));
            watched.push(watch("cartoons", day, 16 * 60 + 30));
            watched.push(watch("devotional", day, 6 * 60 + 30));
        }
        let at = |minute| rank_pins(&pins, &watched, 100 * 86_400, minute);
        assert_eq!(at(21 * 60 + 15)[0], "channel:news");
        assert_eq!(at(16 * 60)[0], "channel:cartoons");
        // Wrapping at midnight: 23:50 is nearer 21:00 than 6:30 is.
        assert_eq!(at(23 * 60 + 50)[0], "channel:news");
        assert_eq!(at(6 * 60)[0], "channel:devotional");
        // Never opened: last, in its own order.
        assert_eq!(at(12 * 60)[3], "channel:unwatched");
        // Nothing watched yet: exactly as pinned.
        assert_eq!(
            rank_pins(&pins, &[], 0, 0),
            [
                "channel:news",
                "channel:cartoons",
                "channel:devotional",
                "channel:unwatched"
            ]
        );
    }
}
