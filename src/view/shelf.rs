//! The YouTube shelf (Up from YouTube in the dock): one card per pin with its newest upload,
//! the focused one's title and channel above the row. It takes the big clock's place over the
//! wallpaper and crossfades with it.
use super::{Assets, Scene, TILE_RADIUS, shared::c, shared::text};
use crate::{
    i18n,
    renderer::{Draw, TextStyle},
    state::{Rect, State, Target},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const CARD_W: f32 = 400.;
const CARD_H: f32 = 225.;
const GAP: f32 = 40.;
const LEFT: f32 = 84.;
/// The row's top: its cards end clear of the dock's glass (740).
const ROW_Y: f32 = 470.;
/// Cards that fit across the screen at once.
const VISIBLE: usize = 4;

/// A title as the shelf shows it: no emoji (the TV's font has none), no live marker (the card's
/// badge says it, when YouTube does), and nothing that repeats the line above it: hashtags, and
/// a `|`-separated part that is just the channel's name
/// (`… | Full Bulletin | ETV Andhra Pradesh#etvannadata`).
fn clean_for(title: &str, channel: &str) -> String {
    let text: String = title
        .chars()
        .filter(|c| (*c as u32) < 0x2190 || (0x3000..0xD800).contains(&(*c as u32)))
        .collect();
    let mut text = text.replace(":LIVE", "").replace("| LIVE", "");
    if let Some(stripped) = text.trim_end().strip_suffix("LIVE") {
        text = stripped.to_owned();
    }
    // Hashtags, also glued to a word ("Pradesh#etvannadata").
    let mut plain = String::with_capacity(text.len());
    let mut tag = false;
    for c in text.chars() {
        if c == '#' {
            tag = true;
        } else if c.is_whitespace() || c == '|' {
            tag = false;
        }
        if !tag {
            plain.push(c);
        }
    }
    let channel = channel.trim().to_lowercase();
    let parts: Vec<String> = plain
        .split('|')
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|part| !part.is_empty() && (channel.is_empty() || part.to_lowercase() != channel))
        .collect();
    let text = parts.join(" | ");
    text.trim_end_matches([':', '|', '-', '·'])
        .trim()
        .to_owned()
}

/// "Just now", "25 min ago", "3 hours ago", "Yesterday", or the date.
fn ago(published: &str, now: OffsetDateTime) -> Option<String> {
    let at = OffsetDateTime::parse(published, &Rfc3339).ok()?;
    let minutes = (now - at).whole_minutes();
    Some(match minutes {
        ..=1 => "Just now".into(),
        2..=59 => format!("{minutes} min ago"),
        60..=119 => "1 hour ago".into(),
        120..=1439 => format!("{} hours ago", minutes / 60),
        1440..=2879 => "Yesterday".into(),
        _ => {
            let local = at.to_offset(now.offset());
            format!("{} {}", local.day(), local.month())
        }
    })
}

/// `text` in at most `lines` lines of `width`, ending at a word with "…" where it is longer, as
/// Apple's titles do (cut by the line box, it ended mid-word with nothing to say so). Measured
/// with a margin: for scripts without metrics the measure is an estimate, and the renderer
/// must not wrap a line more.
fn fit_lines(assets: &Assets, text: &str, style: TextStyle, width: f32, lines: usize) -> String {
    let width = width * 0.92;
    if assets.lines(text, style, width) <= lines {
        return text.to_owned();
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let fits = |count: usize| {
        let candidate = format!("{}…", words[..count].join(" "));
        (assets.lines(&candidate, style, width) <= lines).then_some(candidate)
    };
    // The most words that fit, found by halving (titles are a few dozen words at most).
    let (mut low, mut high) = (0, words.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(middle).is_some() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    fits(low.max(1)).unwrap_or_else(|| format!("{}…", words.first().copied().unwrap_or("")))
}

/// Lettering over any wallpaper: white over a faint shade just below.
fn lettering(out: &mut Scene, words: &str, rect: Rect, style: TextStyle, alpha: f32) {
    let shade = Rect::new(rect.x, rect.y + 2., rect.w, rect.h);
    out.text(words, shade, style, c(0x000000, 0.22 * alpha), false);
    out.text(words, rect, style, c(0xffffff, alpha), false);
}

pub(super) fn shelf(out: &mut Scene, state: &State, assets: &Assets, now: OffsetDateTime) {
    let cards = state.shelf_cards();
    if cards.is_empty() {
        return;
    }
    let focused = match state.focus {
        Target::Card(index) => index.min(cards.len() - 1),
        _ => 0,
    };
    // The row moves only when focus would leave it, and then glides there with the focus:
    // the focused card stays where the eye is, and the row never jumps.
    let first_for = |index: usize| index.saturating_sub(VISIBLE - 1) as f32;
    let first = match state.focus_move() {
        Some((Target::Card(from), progress)) => {
            let from = first_for((*from).min(cards.len() - 1));
            from + (first_for(focused) - from) * progress
        }
        _ => first_for(focused),
    };
    // A soft shade under the shelf, as tvOS lays one under content over artwork: white
    // lettering stays legible over a bright wallpaper or Top Shelf picture. Clear at the top,
    // deepest behind the cards.
    // Two bands: rising to its depth above the lettering, then holding behind the lettering
    // and the cards (one ramp left the small line on top nearly bare).
    let text_top = ROW_Y - 210.;
    out.draw.push(Draw::RoundedGradient(
        Rect::new(0., text_top - 200., 1920., 200.),
        0.,
        c(0x000000, 0.),
        c(0x000000, 0.42),
    ));
    out.draw.push(Draw::RoundedGradient(
        Rect::new(0., text_top, 1920., ROW_Y + CARD_H + 70. - text_top),
        0.,
        c(0x000000, 0.42),
        c(0x000000, 0.52),
    ));
    let start = out.draw.len();
    for (index, shelf_card) in cards.iter().enumerate() {
        let x = LEFT + (index as f32 - first) * (CARD_W + GAP);
        if x + CARD_W < 0. || x > 1920. {
            continue;
        }
        let card_start = out.draw.len();
        let lift = state.focus_amount(&Target::Card(index));
        let base = Rect::new(x, ROW_Y, CARD_W, CARD_H);
        let scale = 1. + 0.08 * lift;
        let (cx, cy) = base.center();
        let rect = Rect::new(
            cx - CARD_W * scale / 2.,
            cy - CARD_H * scale / 2.,
            CARD_W * scale,
            CARD_H * scale,
        );
        if lift > 0. {
            out.shadow(rect, TILE_RADIUS, 48., 30. * lift, 0.5 * lift);
        }
        match &shelf_card.card.image {
            Some(image) => out.image(image.clone(), rect, TILE_RADIUS, false),
            // Its picture is still on its way: the card's place, quietly.
            None => out.shape(rect, TILE_RADIUS, c(0x2c2c2e, 0.9)),
        }
        if lift > 0. {
            out.rim(rect, TILE_RADIUS, 0.3 * lift, 0.);
        }
        let live = shelf_card.live;
        // The channel's picture, bottom left on the card, as on YouTube's own TV app.
        let size = 52. * scale;
        let pad = 14. * scale;
        if let Some(avatar) = shelf_card.avatar {
            let circle = Rect::new(rect.x + pad, rect.y + rect.h - pad - size, size, size);
            out.shape(circle.inset(-2.), (size + 4.) / 2., c(0xffffff, 0.9));
            out.draw.push(Draw::TintedImage(
                avatar.to_owned(),
                circle,
                size / 2.,
                false,
                [1., 1., 1., 1.],
            ));
        }
        if live {
            let style = text(20. * scale, 650, 0.06);
            let word = "LIVE";
            let width = assets.width(word, style) + 24. * scale;
            let badge = Rect::new(rect.x + pad, rect.y + pad, width, 32. * scale);
            out.shape(badge, badge.h / 2., c(0xe5241b, 1.));
            out.text(
                word,
                Rect::new(badge.x, badge.y + 3. * scale, badge.w, badge.h),
                style,
                c(0xffffff, 1.),
                true,
            );
        }
        out.control(Target::Card(index), base, false);
        // A card reaching past either edge of the screen fades with how far it reaches, as a
        // row runs off the edge on tvOS, instead of being cut.
        let inside = ((1920. - LEFT / 2. - x) / CARD_W).min((x + CARD_W - LEFT / 2.) / CARD_W);
        let shown = inside.clamp(0., 1.);
        if shown < 1. {
            out.fade(card_start, shown * shown * (3. - 2. * shown), 1.);
        }
    }
    // The focused card's video and channel, above the row.
    let shelf_card = &cards[focused];
    let title = clean_for(&shelf_card.card.title, &shelf_card.pin.title);
    let mut meta = shelf_card.pin.title.clone();
    // A stream on air is live now, not as old as its upload: "4 hours ago" beside a LIVE badge
    // read as a stream that had ended.
    let when = if shelf_card.live {
        Some(i18n::t("liveNow").to_owned())
    } else {
        ago(&shelf_card.card.published, now)
    };
    if let Some(when) = when {
        meta = format!("{meta} · {when}");
    }
    lettering(
        out,
        &meta,
        Rect::new(LEFT, ROW_Y - 196., 1500., 40.),
        text(30., 600, 0.),
        0.78,
    );
    let style = text(44., 650, -0.01).leading(52. / 44.);
    let title = fit_lines(assets, &title, style, 1500., 2);
    lettering(
        out,
        &title,
        Rect::new(LEFT, ROW_Y - 150., 1500., 112.),
        style,
        1.,
    );
    let _ = start;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_lose_their_emoji_and_live_markers() {
        assert_eq!(
            clean_for("దిల్లీకి సీఎం చంద్రబాబు.. చర్చలు? :🔴LIVE", ""),
            "దిల్లీకి సీఎం చంద్రబాబు.. చర్చలు?"
        );
        assert_eq!(
            clean_for("Weekend news roundup", ""),
            "Weekend news roundup"
        );
    }

    #[test]
    fn titles_lose_hashtags_and_the_channel_s_own_name() {
        assert_eq!(
            clean_for(
                "అన్నదాత | 27th November 2026 | Annadata | Full Bulletin | ETV Andhra Pradesh#etvannadata",
                "ETV Andhra Pradesh"
            ),
            "అన్నదాత | 27th November 2026 | Annadata | Full Bulletin"
        );
        assert_eq!(
            clean_for(
                "Kishan Reddy LIVE: Public Meeting | NTV Telugu #news #live",
                "NTV Telugu"
            ),
            "Kishan Reddy LIVE: Public Meeting"
        );
        // A channel name inside a sentence stays: only a part that is just the name goes.
        assert_eq!(
            clean_for("Why ETV Telugu changed its logo", "ETV Telugu"),
            "Why ETV Telugu changed its logo"
        );
    }

    #[test]
    fn a_long_title_ends_at_a_word_with_an_ellipsis_in_two_lines() {
        let assets = crate::tests::test_assets();
        let style = text(44., 650, -0.01).leading(52. / 44.);
        let long = "Kishan Reddy addresses a public meeting on the new railway line and the \
                    irrigation projects promised to the farmers of the district this year";
        let fitted = fit_lines(&assets, long, style, 1500., 2);
        assert!(fitted.ends_with('…'));
        assert!(long.starts_with(fitted.trim_end_matches('…')));
        assert!(assets.lines(&fitted, style, 1500. * 0.92) <= 2);
        // A title that fits is left whole.
        assert_eq!(
            fit_lines(&assets, "Morning news", style, 1500., 2),
            "Morning news"
        );
    }

    #[test]
    fn uploads_say_how_long_ago() {
        let now = OffsetDateTime::parse("2026-11-27T10:00:00+05:30", &Rfc3339).unwrap();
        assert_eq!(ago("2026-11-27T04:20:00+00:00", now).unwrap(), "10 min ago");
        assert_eq!(
            ago("2026-11-27T01:30:00+00:00", now).unwrap(),
            "3 hours ago"
        );
        assert_eq!(ago("2026-11-26T01:30:00+00:00", now).unwrap(), "Yesterday");
        assert_eq!(
            ago("2026-11-20T01:30:00+00:00", now).unwrap(),
            "20 November"
        );
    }
}
