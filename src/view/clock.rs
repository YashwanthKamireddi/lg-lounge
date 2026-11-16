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

/// "Monday 16 November".
fn day_and_date(now: OffsetDateTime) -> String {
    format!("{} {} {}", now.weekday(), now.day(), now.month())
}

/// Ink that stays readable on any wallpaper: white, over a soft dark copy just below it.
fn lettering(out: &mut Scene, words: &str, rect: Rect, style: TextStyle, alpha: f32) {
    let drop = (style.size * 0.012).max(1.5);
    let shade = Rect::new(rect.x, rect.y + drop, rect.w, rect.h);
    out.text(words, shade, style, c(0x000000, 0.2 * alpha), true);
    out.text(words, rect, style, c(0xffffff, alpha), true);
}

fn digital(out: &mut Scene, state: &State, assets: &Assets, now: OffsetDateTime) {
    let _ = assets;
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
    // The Lock Screen's proportions: the date a line of semibold, the time in tall bold figures
    // set tight, the two as one block.
    lettering(
        out,
        &day_and_date(now),
        Rect::new(CENTRE_X - 700., TOP - 40., 1400., 56.),
        text(46., 600, 0.),
        0.9,
    );
    lettering(
        out,
        &time,
        Rect::new(CENTRE_X - 700., TOP - 6., 1400., 380.),
        text(320., 650, -0.035),
        0.94,
    );
}

fn analog(out: &mut Scene, state: &State, assets: &Assets, now: OffsetDateTime) {
    let radius = 214.;
    let centre = (CENTRE_X, TOP + radius - 18.);
    let face = Rect::new(
        centre.0 - radius,
        centre.1 - radius,
        radius * 2.,
        radius * 2.,
    );
    // The face: a deep pane of the dock's glass, lit at its rim.
    out.shadow(face, radius, 70., 14., 0.32);
    out.draw.push(Draw::Soft(Some(0.8)));
    out.glass_over(home_backdrop(state, assets), face, radius, c(PLATTER, 0.52));
    out.draw.push(Draw::Soft(None));
    out.rim(face, radius, 0.7, 0.1);
    let at = |angle: f32, distance: f32| {
        (
            centre.0 + distance * angle.sin(),
            centre.1 - distance * angle.cos(),
        )
    };
    // A fine minute track, the hours marked bolder within it.
    for mark in 0..60 {
        let hour = mark % 5 == 0;
        let (length, width, alpha) = if hour { (18., 4., 1.) } else { (9., 2., 0.42) };
        let outer = radius - 12.;
        out.draw.push(Draw::Rotated(
            Rect::new(centre.0 - width / 2., centre.1 - outer, width, length),
            width / 2.,
            mark as f32 / 60. * TAU,
            centre,
            c(0xffffff, alpha),
        ));
    }
    // Every hour numbered, upright and large: a glance from the sofa reads it.
    let numeral = text(42., 600, -0.02);
    for hour in 1..=12 {
        let (x, y) = at(hour as f32 / 12. * TAU, radius - 64.);
        out.text(
            hour.to_string(),
            Rect::new(x - 40., y - 26., 80., 52.),
            numeral,
            c(0xffffff, 0.95),
            true,
        );
    }
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
    lettering(
        out,
        &day_and_date(now),
        Rect::new(CENTRE_X - 600., face.y + face.h + 24., 1200., 50.),
        text(40., 600, 0.),
        0.92,
    );
}
