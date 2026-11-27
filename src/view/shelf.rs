//! The YouTube shelf (Up from YouTube in the dock): one card per pin with its newest upload,
//! the focused one's title and channel above the row. It takes the big clock's place over the
//! wallpaper and crossfades with it.
use super::{Assets, Scene, TILE_RADIUS, shared::c, shared::text};
use crate::{
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

/// A title as the shelf shows it: no emoji (the TV's font has none) and no live marker, which
/// becomes the card's LIVE badge; and whether it was live.
fn clean(title: &str) -> (String, bool) {
    let live = title.contains("LIVE") || title.contains('🔴');
    let text: String = title
        .chars()
        .filter(|c| (*c as u32) < 0x2190 || (0x3000..0xD800).contains(&(*c as u32)))
        .collect();
    let mut text = text.replace(":LIVE", "").replace("| LIVE", "");
    if let Some(stripped) = text.trim_end().strip_suffix("LIVE") {
        text = stripped.to_owned();
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let text = words.join(" ");
    let text = text
        .trim_end_matches([':', '|', '-', '·'])
        .trim()
        .to_owned();
    (text, live)
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
    // The row moves only when focus would leave it: the focused card stays where the eye is.
    let first = focused.saturating_sub(VISIBLE - 1);
    let start = out.draw.len();
    for (index, shelf_card) in cards.iter().enumerate() {
        let x = LEFT + (index as f32 - first as f32) * (CARD_W + GAP);
        if x + CARD_W < -CARD_W || x > 1920. + CARD_W {
            continue;
        }
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
        let show = shelf_card.pin.kind == crate::model::PinKind::Show;
        match &shelf_card.card.image {
            Some(image) => out.image(image.clone(), rect, TILE_RADIUS, false),
            // A show has no thumbnail feed: its card is JioHotstar's own blue to violet, with
            // the show's name across it.
            None if show => {
                out.draw.push(Draw::RoundedGradient(
                    rect,
                    TILE_RADIUS,
                    c(0x1c4fe0, 1.),
                    c(0x6a2bd9, 1.),
                ));
                out.text(
                    shelf_card.pin.title.as_str(),
                    Rect::new(
                        rect.x + 24. * scale,
                        rect.y + rect.h / 2. - 34. * scale,
                        rect.w - 48. * scale,
                        68. * scale,
                    ),
                    text(40., 650, -0.01).scaled(scale),
                    c(0xffffff, 1.),
                    true,
                );
                out.text(
                    "JioHotstar",
                    Rect::new(rect.x + 22. * scale, rect.y + 18. * scale, 300., 30.),
                    text(20., 600, 0.02).scaled(scale),
                    c(0xffffff, 0.8),
                    false,
                );
            }
            // Its picture is still on its way: the card's place, quietly.
            None => out.shape(rect, TILE_RADIUS, c(0x2c2c2e, 0.9)),
        }
        if lift > 0. {
            out.rim(rect, TILE_RADIUS, 0.3 * lift, 0.);
        }
        let (_, live) = clean(&shelf_card.card.title);
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
    }
    // The focused card's video and channel, above the row.
    let shelf_card = &cards[focused];
    let (title, meta) = if shelf_card.pin.kind == crate::model::PinKind::Show {
        (
            shelf_card.pin.title.clone(),
            crate::i18n::t("showLatest").to_owned(),
        )
    } else {
        let (title, _) = clean(&shelf_card.card.title);
        let mut meta = shelf_card.pin.title.clone();
        if let Some(ago) = ago(&shelf_card.card.published, now) {
            meta = format!("{meta} · {ago}");
        }
        (title, meta)
    };
    lettering(
        out,
        &meta,
        Rect::new(LEFT, ROW_Y - 196., 1500., 40.),
        text(30., 600, 0.),
        0.78,
    );
    let title: String = title.chars().take(110).collect();
    lettering(
        out,
        &title,
        Rect::new(LEFT, ROW_Y - 150., 1500., 112.),
        text(44., 650, -0.01).leading(52. / 44.),
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
            clean("దిల్లీకి సీఎం చంద్రబాబు.. చర్చలు? :🔴LIVE"),
            ("దిల్లీకి సీఎం చంద్రబాబు.. చర్చలు?".into(), true)
        );
        assert_eq!(
            clean("Weekend news roundup"),
            ("Weekend news roundup".into(), false)
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
