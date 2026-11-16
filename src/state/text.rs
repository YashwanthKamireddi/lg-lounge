//! Typing a place name for the weather, on the TV's own keyboard (on the desktop, the
//! computer's).
use super::*;

impl State {
    pub fn text(&mut self, text: &str) {
        if self.text_input_active() {
            let mut query = self.weather.query.clone();
            query.extend(
                text.chars()
                    .filter(|c| !c.is_control())
                    .take(120usize.saturating_sub(query.chars().count())),
            );
            self.weather.edit(query);
            self.dirty = true;
        }
    }

    pub fn delete(&mut self) {
        if self.text_input_active() {
            let mut query = self.weather.query.clone();
            query.pop();
            self.weather.edit(query);
            self.dirty = true;
        }
    }

    pub fn text_input_active(&self) -> bool {
        matches!(self.overlay, Some(Overlay::SystemKeyboard))
    }

    pub fn finish_text_input(&mut self) {
        if self.text_input_active() {
            self.overlay = None;
            self.panel_started = None;
            self.focus = Target::WeatherSearch;
            self.pointer = false;
            self.dirty = true;
        }
    }

    pub fn delete_surrounding_text(&mut self, offset: i32, length: i32) {
        if !self.text_input_active() || length <= 0 {
            return;
        }
        // SDL-webOS forwards the native text model's UTF-8 byte offsets,
        // relative to the insertion cursor (at the end of the text).
        let mut query = self.weather.query.clone();
        let mut start =
            (query.len() as i64 + i64::from(offset)).clamp(0, query.len() as i64) as usize;
        let mut end = start.saturating_add(length as usize).min(query.len());
        while !query.is_char_boundary(start) {
            start -= 1;
        }
        while !query.is_char_boundary(end) {
            end += 1;
        }
        query.replace_range(start..end, "");
        self.weather.edit(query);
        self.dirty = true;
    }

    pub fn editing_text(&self) -> &str {
        &self.weather.query
    }

    /// Opens the keyboard for the weather's place name.
    pub fn open_keyboard(&mut self) {
        self.overlay = Some(Overlay::SystemKeyboard);
        self.focus = Target::WeatherQuery;
        self.panel_started = None;
        self.dirty = true;
    }

    pub(super) fn open_location(&mut self) {
        self.weather.edit(String::new());
        // A fresh page: no answer or message left from the last visit.
        self.weather.cancel_search();
        self.subpage = Some(Subpage::WeatherLocation);
        self.focus = Target::WeatherQuery;
        self.scroll = 0.;
        if !self.preferences.reduced_motion {
            self.panel_started = Some(crate::clock::now());
        }
        self.dirty = true;
    }
}
