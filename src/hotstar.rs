//! JioHotstar pins: a show, opened in the TV's JioHotstar app on its own page, where "Watch
//! Latest Episode" (or "Watch Next", where the profile left off) is already selected.
//!
//! The TV app (livingroom.hotstar.com) reads its launch `contentTarget` as a URL and follows it
//! only on its own host; the path after it is the website's (`/in/shows/<name>/<id>`). It asks
//! "Who's watching?" first on every launch. No feed says which episode is newest without signing
//! in, so a pin opens the show rather than guessing an episode.
use serde_json::{Value, json};

/// The TV's JioHotstar app.
pub const APP: &str = "hotstar";

/// A show's path, `in/shows/<name>/<id>`, from a link the website or the phone app shares.
pub fn parse_show(text: &str) -> Option<String> {
    let text = text.trim();
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    let (host, path) = rest.split_once('/')?;
    let host = host.trim_start_matches("www.");
    if !matches!(
        host,
        "hotstar.com" | "jiohotstar.com" | "livingroom.hotstar.com"
    ) {
        return None;
    }
    let path = path.split(['?', '#']).next()?;
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let [country, "shows", name, id, ..] = segments.as_slice() else {
        return None;
    };
    let plain = |text: &str| {
        !text.is_empty()
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    (country.len() == 2
        && plain(country)
        && plain(name)
        && name.len() <= 80
        && (1..=12).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_digit()))
    .then(|| format!("{country}/shows/{name}/{id}"))
}

/// A name to show for a show from its path ("bigg-boss" → "Bigg Boss"), until one is given.
pub fn title_from_path(path: &str) -> String {
    path.split('/')
        .nth(2)
        .unwrap_or("")
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The launch parameters that open the show's page in the TV app.
pub fn launch_params(path: &str) -> Value {
    json!({ "contentTarget": format!("https://livingroom.hotstar.com/{path}") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_links_as_jiohotstar_shares_them() {
        let telugu = Some("in/shows/bigg-boss/14714".to_owned());
        assert_eq!(
            parse_show("https://www.hotstar.com/in/shows/bigg-boss/14714"),
            telugu
        );
        assert_eq!(
            parse_show("https://www.jiohotstar.com/in/shows/bigg-boss/14714/watch?x=1"),
            telugu
        );
        assert_eq!(
            parse_show("https://www.hotstar.com/in/shows/bigg-boss/14714/day-74/1271702200/watch"),
            telugu
        );
        for text in [
            "https://www.hotstar.com/in/movies/x/123",
            "https://example.com/in/shows/bigg-boss/14714",
            "https://www.hotstar.com/in/shows/Bigg Boss/14714",
            "https://www.hotstar.com/in/shows/bigg-boss/abc",
            "bigg boss",
        ] {
            assert_eq!(parse_show(text), None, "{text}");
        }
    }

    #[test]
    fn a_show_opens_on_the_tv_app_s_own_host() {
        assert_eq!(
            launch_params("in/shows/bigg-boss/14714"),
            json!({ "contentTarget": "https://livingroom.hotstar.com/in/shows/bigg-boss/14714" })
        );
        assert_eq!(title_from_path("in/shows/bigg-boss/14714"), "Bigg Boss");
    }
}
