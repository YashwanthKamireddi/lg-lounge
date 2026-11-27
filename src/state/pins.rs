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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShelfRow {
    pub cards: Vec<Card>,
    pub avatar: Option<String>,
}

/// A card on the shelf: a pin's newest upload (a video pin: itself).
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfCard<'a> {
    pub pin: &'a crate::model::Pin,
    pub card: &'a Card,
    pub avatar: Option<&'a str>,
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
        self.preferences
            .pins
            .iter()
            .filter(|pin| pin.app() == app)
            .filter_map(|pin| {
                let row = self.shelf.get(&pin.key())?;
                Some(ShelfCard {
                    pin,
                    card: row.cards.first()?,
                    avatar: row.avatar.as_deref(),
                })
            })
            .collect()
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
                    self.shelf.insert(pin, ShelfRow { cards, avatar });
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

    /// What OK on a shelf card opens: its video, in the pin's app.
    pub(super) fn open_card(&self, index: usize) -> Vec<Effect> {
        let Some(card) = self.shelf_cards().get(index).cloned() else {
            return vec![];
        };
        let params = crate::youtube::launch_params(&card.card.video);
        vec![Effect::Open(card.pin.app().into(), params)]
    }

    /// The remote or the pointer was used.
    pub fn touched(&mut self) {
        self.last_input = Some(crate::clock::now());
    }

    /// Takes a pin off, its row with it.
    pub fn unpin(&mut self, key: &str) -> Vec<Effect> {
        let before = self.preferences.pins.len();
        self.preferences.pins.retain(|pin| pin.key() != key);
        if self.preferences.pins.len() == before {
            return vec![];
        }
        self.shelf.remove(key);
        self.dirty = true;
        vec![Effect::Save]
    }
}
