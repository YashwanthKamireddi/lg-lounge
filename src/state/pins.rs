//! YouTube pins in the state: each pin's row of latest uploads for the shelf, when they are
//! due a refresh, and what a resolved link does. The fetching is `pins_service`'s.
use super::*;
use crate::pins_service::{Card, Command, Response};

/// How often the uploads are checked while Home is on screen: news channels post through the
/// day, and an hour-old "latest" would not be.
pub const PINS_REFRESH: Duration = Duration::from_secs(15 * 60);

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
    /// The shelf, in pin order: each pin with an upload fetched shows its newest.
    pub fn shelf_cards(&self) -> Vec<ShelfCard<'_>> {
        self.preferences
            .pins
            .iter()
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

    /// The shelf shows whenever YouTube is focused in the dock, as Apple TV's Top Shelf shows
    /// an app's content the moment it is focused: nobody has to know to press Up for it. Up
    /// then moves into its cards.
    pub fn shelf_visible(&self) -> bool {
        self.page == Page::Apps
            && !self.expanded
            && (self.shelf_open
                || (self.focus == Target::App(YOUTUBE.into())
                    && self
                        .apps()
                        .iter()
                        .take(APPS_COLUMNS)
                        .any(|app| app.id == YOUTUBE)))
            && !self.shelf_cards().is_empty()
    }

    /// Notes the shelf appearing or leaving, for the crossfade with the clock (once a frame).
    pub(super) fn track_shelf(&mut self) {
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

    /// Up from YouTube in the dock: the shelf, on its first card.
    pub(super) fn open_shelf(&mut self) -> bool {
        if self.shelf_cards().is_empty() {
            return false;
        }
        self.shelf_open = true;
        self.focus = Target::Card(0);
        self.dirty = true;
        true
    }

    /// Down or Back from the shelf: YouTube in the dock again.
    pub(super) fn close_shelf(&mut self) {
        self.shelf_open = false;
        self.focus = Target::App(YOUTUBE.into());
        self.dirty = true;
    }

    /// A refresh of every pin, when one is due: at once with Home on screen and none yet,
    /// then every `PINS_REFRESH` while it stays on screen. Behind an app nothing is fetched.
    pub fn pins_due(&mut self, visible: bool) -> Option<Effect> {
        if !visible || self.preferences.pins.is_empty() {
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
