//! Home: the Top Shelf dock and the app grid.
use super::{
    Assets, DOCK_RADIUS, FACE_HEIGHT, GRID_VEIL, Scene,
    glass::{home_backdrop, shelf_art, shelf_dissolve},
    shared::{c, row_style, text},
    tile::tile,
};
use crate::{
    i18n,
    renderer::{Color, Draw, GlassBlend, WHITE},
    state::{
        APPS_COLUMNS, DOCK_GAP, DOCK_TOP, GRID_GAP, GRID_LEFT, GRID_SCROLL, GRID_STEP,
        HOME_TILE_WIDTH, ROW_PITCH, Rect, SHELF_HINT, State, Target,
    },
};

pub(super) fn library(out: &mut Scene, state: &State, assets: &Assets) {
    // tvOS home: the dock platter (1820x274, r72) holds
    // the first row; the grid follows. Dock icons carry no labels; grid icons
    // show their name only when focused.
    let s = state.scroll;
    let apps = state.apps();
    if apps.is_empty() {
        let title = if state.loading {
            i18n::t("findingApps")
        } else {
            i18n::t("roomForNew")
        };
        let hint = state
            .error
            .clone()
            .unwrap_or_else(|| i18n::t("noCategoryApps").into());
        empty(
            out,
            state,
            assets,
            EmptyState {
                rect: Rect::new(84., 600., 1752., 304.),
                icon: "grid",
                title,
                hint: &hint,
                action: state.error.as_ref().map(|_| Target::Retry),
            },
        );
        return;
    }
    let dock_y = DOCK_TOP - s;
    // Under the fold the first grid row sits 230 below the dock; as the grid
    // rises it eases out to 260, following the drawn (gliding) scroll.
    let spread = (state.shown_scroll / GRID_SCROLL).clamp(0., 1.);
    let gap = DOCK_GAP + (GRID_GAP - DOCK_GAP) * spread;
    // Liquid Glass platter: exactly what is behind it, blurred and seen
    // through the glass's lens, as the context menu's and the time pill's
    // glass is. The blur dissolves with Top Shelf.
    let platter = Rect::new(50., dock_y - 40., 1820., FACE_HEIGHT + 80.);
    let backdrop = home_backdrop(state, assets);
    // tvOS 26 dock: clear glass, no veil, lit by the appearance's own light
    // (the lens tint rising toward the top). The light is the glass's tint,
    // so it costs no layer of its own: a quarter of the screen less to
    // paint on every scrolling frame.
    let (top, bottom) = out.theme.lens();
    let glass = |out: &mut Scene, path: String, shade: [f32; 3], alpha: f32, blend| {
        let lit = |light: Color| {
            [
                light[0] * shade[0],
                light[1] * shade[1],
                light[2] * shade[2],
                alpha,
            ]
        };
        out.draw.push(Draw::Glass(
            path,
            platter,
            DOCK_RADIUS,
            lit(top),
            lit(bottom),
            blend,
        ));
    };
    // Over the grid the platter blurs the grid's backdrop, blending in with
    // the drawn scroll exactly as the screen behind it does: one pane mixing
    // both backdrops, not the grid's glass laid over the dock's.
    let grid = (spread > 0.).then(|| GlassBlend {
        to: assets.wallpaper(state.wallpaper(), "-glass.png"),
        mix: spread,
        shade: [GRID_VEIL, GRID_VEIL, GRID_VEIL + 0.02],
    });
    match shelf_dissolve(state) {
        // Top Shelf art dissolving while the grid rises (rare): layered.
        Some((previous, t)) if spread < 1. => {
            let (_, previous) = shelf_art(state, assets, previous);
            glass(out, previous.clone(), [1.; 3], 1., None);
            if previous != backdrop {
                glass(out, backdrop, [1.; 3], t, None);
            }
            if let Some(grid) = grid {
                glass(out, grid.to, grid.shade, spread, None);
            }
        }
        _ => glass(out, backdrop, [1.; 3], 1., grid),
    }
    // One thin specular rim (no flat outline).
    out.rim(platter, DOCK_RADIUS, 0.5, 0.06);
    let slot = |index: usize| {
        let (row, column) = (index / APPS_COLUMNS, index % APPS_COLUMNS);
        let y = if row == 0 {
            dock_y
        } else {
            dock_y + gap + (row - 1) as f32 * ROW_PITCH
        };
        (GRID_LEFT + column as f32 * GRID_STEP, y)
    };
    // Move mode: a reorder glides every displaced tile (dock and grid alike)
    // from its old slot to its new one, easing out, as tvOS rearranges.
    let glide = state.glide_offsets();
    let arrived = state
        .rearranged
        .as_ref()
        .map_or(1., |(at, _)| State::glide_eased(*at));
    if let Some((id, index, at)) = &state.leaving
        && let Some(app) = state.inventory.apps.iter().find(|app| &app.id == id)
    {
        let t = (crate::clock::since(at).as_secs_f32() * 1000. / crate::state::LEAVE_MS).min(1.);
        let gone = 1. - (1. - t).powi(3);
        let (x, y) = slot(*index);
        let start = out.draw.len();
        let controls = out.controls.len();
        tile(
            out,
            state,
            assets,
            app,
            Rect::new(x, y, HOME_TILE_WIDTH, FACE_HEIGHT),
            None,
            false,
        );
        out.controls.truncate(controls);
        out.scale_about(
            start,
            x + HOME_TILE_WIDTH / 2.,
            y + FACE_HEIGHT / 2.,
            1. - 0.2 * gone,
        );
        out.fade(start, 1. - gone, 1.);
    }
    // The lifted tile (the app in hand during Move, else the focused one) is drawn last, so it
    // passes over its neighbours as it glides and grows instead of under them.
    let lifted = state.moving.clone().or_else(|| match &state.focus {
        Target::App(id) => Some(id.clone()),
        _ => None,
    });
    let order = apps
        .iter()
        .enumerate()
        .filter(|(_, app)| Some(&app.id) != lifted.as_ref())
        .chain(
            apps.iter()
                .enumerate()
                .filter(|(_, app)| Some(&app.id) == lifted.as_ref()),
        );
    for (index, app) in order {
        let row = index / APPS_COLUMNS;
        let (mut x, mut y) = slot(index);
        if let Some((dx, dy)) = glide.get(&app.id) {
            x += dx;
            y += dy;
        }
        let entering = state.entering.contains(&app.id);
        let start = out.draw.len();
        let focused = state.focus == Target::App(app.id.clone());
        tile(
            out,
            state,
            assets,
            app,
            Rect::new(x, y, HOME_TILE_WIDTH, FACE_HEIGHT),
            (row > 0 && (state.preferences.show_names || focused)).then_some(WHITE),
            false,
        );
        if entering {
            out.fade(start, arrived, 1.);
        }
    }
    shelf_chevron(out, state, assets, dock_y);
}

/// tvOS "Swipe up for fullscreen": a compact chevron centred above the dock
/// while a dock app with Top Shelf art has focus (only those go full screen). It bobs a few times when
/// Top Shelf mode begins, then rests (an idle home renders no frames).
fn shelf_chevron(out: &mut Scene, state: &State, assets: &Assets, dock_y: f32) {
    let Target::App(id) = &state.focus else {
        return;
    };
    let in_dock = state
        .apps()
        .iter()
        .take(APPS_COLUMNS)
        .any(|app| &app.id == id);
    if !in_dock || !state.has_art(id) || state.expanded {
        return;
    }
    // With pins, Up moves into the shelf instead: the chevron leaves as the shelf comes.
    let alpha = (1. - state.shown_scroll.abs() / 60.).clamp(0., 1.) * (1. - state.shelf_shown());
    if alpha <= 0. {
        return;
    }
    let lift = state
        .shelf_since
        .filter(|_| !state.preferences.reduced_motion)
        .map_or(0., |since| {
            let t = crate::clock::since(since).as_secs_f32();
            if t >= SHELF_HINT {
                0.
            } else {
                // Rise and settle: 0 → -8 → 0 every 0.8 s.
                -8. * (t / 0.8 * std::f32::consts::PI).sin().abs()
            }
        });
    out.icon(
        assets,
        "chevron-up",
        Rect::new(928., dock_y - 104. + lift, 64., 64.),
        c(0xffffff, 0.8 * alpha),
    );
}

struct EmptyState<'a> {
    rect: Rect,
    icon: &'a str,
    title: &'a str,
    hint: &'a str,
    action: Option<Target>,
}
fn empty(out: &mut Scene, state: &State, assets: &Assets, content: EmptyState<'_>) {
    let EmptyState {
        rect,
        icon,
        title,
        hint,
        action,
    } = content;
    out.shape(rect, 17.6, c(0xffffff, 0.02));
    out.border(rect, 17.6, 1., c(0xffffff, 0.05));
    let center = rect.x + rect.w / 2.;
    let top = rect.y
        + if action.is_some() {
            33.
        } else {
            (rect.h - 123.2) / 2.
        };
    out.icon(
        assets,
        icon,
        Rect::new(center - 17.6, top, 35.2, 35.2),
        c(0xd8d8d8, 0.60),
    );
    out.text(
        title,
        Rect::new(rect.x + 32., top + 52., rect.w - 64., 31.),
        text(24., 550, -0.035),
        WHITE,
        true,
    );
    out.text(
        hint,
        Rect::new(center - 352., top + 97.6, 704., 64.),
        text(16., 400, 0.).leading(1.6),
        c(0xf5f5f5, 0.72),
        true,
    );
    if let Some(target) = action {
        let label = i18n::t(if target == Target::Retry {
            "retry"
        } else {
            "exploreApps"
        });
        let width = assets.width(label, row_style()) + 80.;
        let button = Rect::new(center - width / 2., top + 140., width, 56.);
        out.shape(button, 11.2, c(0xededed, 1.));
        if state.focus == target {
            out.border(button.inset(-7.), 18.2, 2., WHITE);
        }
        out.text(
            label,
            Rect::new(button.x + 20.8, button.y + 12., button.w - 80., 32.),
            row_style(),
            c(0x242424, 1.),
            false,
        );
        out.icon(
            assets,
            "arrow",
            Rect::new(button.x + button.w - 38.4, button.y + 14.6, 17.6, 17.6),
            c(0x242424, 1.),
        );
        out.control(target, button, true);
    }
}
