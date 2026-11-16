//! The big clock over Home's wallpaper (Settings › Home › Clock). Digital is the Lock Screen's:
//! the day and date small above the time in large figures. Analog is a glass face with every
//! hour numbered, so it reads at a glance from across the room. Both stand on the wallpaper
//! and leave as the grid rises, with the status pill.
use super::{
    Assets, Scene,
    glass::home_backdrop,
    shared::{PLATTER, c, text},
};
use crate::{
    model::ClockStyle,
    renderer::{Draw, TextStyle},
    state::{Rect, State},
};
use std::f32::consts::TAU;
use time::OffsetDateTime;

/// The centre line the clock stands on, and its top: under the status pill, over the dock.
const CENTRE_X: f32 = 960.;
const TOP: f32 = 150.;

pub(super) fn big_clock(out: &mut Scene, state: &State, assets: &Assets, now: OffsetDateTime) {
    if !state.clock_set {
        // The TV has not set its clock yet (no network since power-on): no wrong time.
        return;
    }
    match state.preferences.clock {
        ClockStyle::Digital => digital(out, state, assets, now),
        ClockStyle::Analog => analog(out, state, assets, now),
        ClockStyle::Off => {}
    }
}

/// "Mon 16 Nov": the Lock Screen's short date over its figures.
fn short_day_and_date(now: OffsetDateTime) -> String {
    let short = |name: String| name.chars().take(3).collect::<String>();
    format!(
        "{} {} {}",
        short(now.weekday().to_string()),
        now.day(),
        short(now.month().to_string())
    )
}

/// Ink that stays readable on any wallpaper: white, over a soft dark copy just below it.
fn lettering(
    out: &mut Scene,
    words: &str,
    rect: Rect,
    style: TextStyle,
    alpha: f32,
    centered: bool,
) {
    let drop = (style.size * 0.012).max(1.5);
    let shade = Rect::new(rect.x, rect.y + drop, rect.w, rect.h);
    out.text(words, shade, style, c(0x000000, 0.2 * alpha), centered);
    out.text(words, rect, style, c(0xffffff, alpha), centered);
}

fn digital(out: &mut Scene, state: &State, assets: &Assets, now: OffsetDateTime) {
    // As the Lock Screen: no AM/PM beside the large figures; the hour alone says enough.
    let hour = if state.hour_24 {
        format!("{:02}", now.hour())
    } else {
        match now.hour() % 12 {
            0 => "12".to_owned(),
            hour => hour.to_string(),
        }
    };
    let time = format!("{hour}:{:02}", now.minute());
    lettering(
        out,
        &short_day_and_date(now),
        Rect::new(CENTRE_X - 700., TOP - 34., 1400., 60.),
        text(48., 650, 0.),
        0.94,
        true,
    );
    // The Lock Screen's figures: tall and narrow (the clock's own condensed cut), spaced so
    // each stands clear, made of Liquid Glass: the picture behind them, lit and frosted.
    let figures = text(460., 620, 0.025).clock();
    out.draw.push(Draw::GlassText(
        time,
        Rect::new(CENTRE_X - 700., TOP - 28., 1400., 560.),
        figures,
        [1.4, 1.4, 1.44, 1.],
        home_backdrop(state, assets),
    ));
}

fn analog(out: &mut Scene, state: &State, assets: &Assets, now: OffsetDateTime) {
    // A watch's face in the screen's language: a near-square rounded rectangle (a squircle, as
    // Apple Watch's), so all twelve hours sit evenly around it as on any real clock.
    let (half_w, half_h) = (250., 236.);
    let radius = half_h;
    let centre = (CENTRE_X, 150. + half_h);
    let face = Rect::new(
        centre.0 - half_w,
        centre.1 - half_h,
        half_w * 2.,
        half_h * 2.,
    );
    let corner = 150.;
    out.shadow(face, corner, 70., 14., 0.32);
    out.draw.push(Draw::Soft(Some(0.8)));
    out.glass_over(home_backdrop(state, assets), face, corner, c(PLATTER, 0.52));
    out.draw.push(Draw::Soft(None));
    out.rim(face, corner, 0.7, 0.1);
    // How far from the centre the outline `inset` inside the face lies, along `angle`.
    let reach = |angle: f32, inset: f32| {
        let (a, b) = (half_w - inset, half_h - inset);
        let (x, y) = (angle.sin(), angle.cos());
        const N: f32 = 4.;
        ((x / a).abs().powf(N) + (y / b).abs().powf(N)).powf(-1. / N)
    };
    let at = |angle: f32, distance: f32| {
        (
            centre.0 + distance * angle.sin(),
            centre.1 - distance * angle.cos(),
        )
    };
    // A fine minute track along the edge, the hours bolder; inside it every hour, numbered.
    for mark in 0..60 {
        let hour = mark % 5 == 0;
        let (length, width, alpha) = if hour { (18., 4.5, 1.) } else { (9., 2., 0.4) };
        let angle = mark as f32 / 60. * TAU;
        let outer = reach(angle, 16.);
        out.draw.push(Draw::Rotated(
            Rect::new(centre.0 - width / 2., centre.1 - outer, width, length),
            width / 2.,
            angle,
            centre,
            c(0xffffff, alpha),
        ));
    }
    let numeral = text(46., 600, -0.02);
    for hour in 1..=12 {
        let angle = hour as f32 / 12. * TAU;
        let (x, y) = at(angle, reach(angle, 66.));
        out.text(
            hour.to_string(),
            Rect::new(x - 44., y - 29., 88., 58.),
            numeral,
            c(0xffffff, 0.96),
            true,
        );
    }
    // The date on the dial, as Apple Watch prints it: the day in the second hand's orange, the
    // date in white, small, between the centre and the 6. Clock and date are one object.
    let day = text(30., 650, 0.06);
    let weekday: String = now
        .weekday()
        .to_string()
        .chars()
        .take(3)
        .collect::<String>()
        .to_uppercase();
    let date = now.day().to_string();
    let (day_w, date_w) = (assets.width(&weekday, day), assets.width(&date, day));
    let gap = 10.;
    let left = centre.0 - (day_w + gap + date_w) / 2.;
    let y = centre.1 + half_h * 0.36;
    out.text(
        weekday,
        Rect::new(left, y, day_w + 4., 38.),
        day,
        c(0xff9f0a, 1.),
        false,
    );
    out.text(
        date,
        Rect::new(left + day_w + gap, y, date_w + 4., 38.),
        day,
        c(0xffffff, 0.96),
        false,
    );
    // Hands move continuously, as a watch's do: the second hand sweeps.
    let seconds = now.second() as f32 + now.nanosecond() as f32 / 1e9;
    let minutes = now.minute() as f32 + seconds / 60.;
    let hours = (now.hour() % 12) as f32 + minutes / 60.;
    // A watch hand: a slim stem out of the centre, then a broad rounded blade, over its shade.
    let hand = |out: &mut Scene, angle: f32, length: f32, blade: f32| {
        let stem = radius * 0.13;
        for (colour, dy) in [(c(0x000000, 0.28), 5.), (c(0xffffff, 1.), 0.)] {
            let pivot = (centre.0, centre.1 + dy);
            out.draw.push(Draw::Rotated(
                Rect::new(pivot.0 - 2.5, pivot.1 - stem - 2., 5., stem + 4.),
                2.5,
                angle,
                pivot,
                colour,
            ));
            out.draw.push(Draw::Rotated(
                Rect::new(pivot.0 - blade / 2., pivot.1 - length, blade, length - stem),
                blade / 2.,
                angle,
                pivot,
                colour,
            ));
        }
    };
    hand(out, hours / 12. * TAU, radius * 0.56, 13.);
    hand(out, minutes / 60. * TAU, radius * 0.86, 10.);
    // The disc the hands turn on, then Apple's orange second hand with its counterweight.
    let disc = |out: &mut Scene, size: f32, colour| {
        out.shape(
            Rect::new(centre.0 - size / 2., centre.1 - size / 2., size, size),
            size / 2.,
            colour,
        );
    };
    disc(out, 18., c(0xffffff, 1.));
    let orange = c(0xff9f0a, 1.);
    let sweep = seconds / 60. * TAU;
    out.draw.push(Draw::Rotated(
        Rect::new(
            centre.0 - 1.75,
            centre.1 - radius * 0.92,
            3.5,
            radius * 1.14,
        ),
        1.75,
        sweep,
        centre,
        orange,
    ));
    disc(out, 11., orange);
    disc(out, 4., c(0x1c1c1e, 1.));
}
