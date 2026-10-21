//! Route validation (docs/design/world-layout.md 6.1 row "Anchorage reachable
//! from previous anchorage", Phase E).
//!
//! Choices made here:
//! - Level `i` is paired with `route.points[i]`; only
//!   `min(levels.len(), route.points.len())` of them are used.
//! - Each segment gets its own **decorated** window (navigability is measured
//!   on decorated terrain), without the layout's structures so the stamped
//!   raft does not block its own anchorage. The window is the box of the two
//!   anchorages and the segment's two route points, grown by
//!   `window_margin + raft_half_width` plus the channel's half-width.
//! - `anchorage_reachable` is one check per consecutive pair: A* over the
//!   eroded navigable set from one anchorage to the next. Its `name` is the
//!   shared constant; the pair is named in `required`.
//! - `channel_clear` takes `half_width` and `clear_above` from the first
//!   `DensityTerm::Channel` in the layout. A channel cell is a window column
//!   whose centre is within `half_width` of a route segment. It is blocked
//!   when any block from `SEA_LEVEL` up to (not including)
//!   `SEA_LEVEL + clear_above` is neither air nor water. With no `Channel`
//!   term in the layout the check fails.

use std::collections::BTreeSet;

use terrain::{Block, HEIGHT, SEA_LEVEL};

use crate::validation::{CheckResult, ValidationReport};
use crate::{
    erode, find_path, nearest_on_segment, validate_level, DensityTerm, Layout, Level, NavMap,
    Route, Thresholds,
};

pub const CHECK_ANCHORAGE_REACHABLE: &str = "anchorage_reachable";
pub const CHECK_CHANNEL_CLEAR: &str = "channel_clear";

/// Window `(x0, z0, w, d)` for one anchorage pair: the box of the two
/// anchorages and the segment's two route points, grown by `margin`.
fn pair_window(
    from: &Level,
    to: &Level,
    pa: [f64; 2],
    pb: [f64; 2],
    margin: i32,
) -> (i32, i32, u32, u32) {
    let xs = [
        from.anchorage[0],
        to.anchorage[0],
        pa[0].floor() as i32,
        pb[0].floor() as i32,
    ];
    let zs = [
        from.anchorage[1],
        to.anchorage[1],
        pa[1].floor() as i32,
        pb[1].floor() as i32,
    ];
    let x0 = xs.into_iter().min().unwrap() - margin;
    let z0 = zs.into_iter().min().unwrap() - margin;
    (
        x0,
        z0,
        (xs.into_iter().max().unwrap() + margin - x0 + 1) as u32,
        (zs.into_iter().max().unwrap() + margin - z0 + 1) as u32,
    )
}

/// The A* path for each consecutive anchorage pair that has one, using the
/// same window and eroded navigable set as `validate_route`. Coordinates are
/// window-local.
pub fn anchorage_paths(
    layout: &Layout,
    levels: &[Level],
    route: &Route,
    t: &Thresholds,
) -> Vec<Vec<[i32; 2]>> {
    let bare = Layout {
        structures: Vec::new(),
        ..layout.clone()
    };
    let half_width = layout.density.iter().find_map(|d| match d {
        DensityTerm::Channel { half_width, .. } => Some(*half_width),
        _ => None,
    });
    let margin =
        t.window_margin + t.raft_half_width as i32 + half_width.unwrap_or(0.0).ceil() as i32;
    let pairs = levels.len().min(route.points.len()).saturating_sub(1);
    (0..pairs)
        .filter_map(|i| {
            let (from, to) = (&levels[i], &levels[i + 1]);
            let (x0, z0, w, d) =
                pair_window(from, to, route.points[i], route.points[i + 1], margin);
            let region = bare.generate_region(x0, z0, w, d, true);
            let nav = erode(&NavMap::from_region(&region, t), t.raft_half_width);
            let [a, b] = [from.anchorage, to.anchorage].map(|a| [a[0] - x0, a[1] - z0]);
            find_path(&nav, a, b)
        })
        .collect()
}

/// Validate every level, every consecutive anchorage pair and the channel
/// along `route`.
pub fn validate_route(
    layout: &Layout,
    levels: &[Level],
    route: &Route,
    t: &Thresholds,
) -> ValidationReport {
    let mut checks = Vec::new();
    for level in levels {
        checks.extend(validate_level(layout, level, t).checks);
    }
    let channel = layout.density.iter().find_map(|d| match d {
        DensityTerm::Channel {
            half_width,
            clear_above,
            ..
        } => Some((*half_width, *clear_above)),
        _ => None,
    });
    let (half_width, clear_above) = channel.unwrap_or((0.0, 0));
    let bare = Layout {
        structures: Vec::new(),
        ..layout.clone()
    };
    let margin = t.window_margin + t.raft_half_width as i32 + half_width.ceil() as i32;
    let mut blocked = BTreeSet::new();
    let pairs = levels.len().min(route.points.len()).saturating_sub(1);
    for i in 0..pairs {
        let (from, to) = (&levels[i], &levels[i + 1]);
        let [(ax, az), (bx, bz)] = [from.anchorage, to.anchorage].map(|a| (a[0], a[1]));
        let (pa, pb) = (route.points[i], route.points[i + 1]);
        let (x0, z0, w, d) = pair_window(from, to, pa, pb, margin);
        let region = bare.generate_region(x0, z0, w, d, true);
        let nav = erode(&NavMap::from_region(&region, t), t.raft_half_width);
        let path = find_path(&nav, [ax - x0, az - z0], [bx - x0, bz - z0]);
        checks.push(CheckResult {
            name: CHECK_ANCHORAGE_REACHABLE,
            measured: path.as_ref().map_or(0.0, |p| p.len() as f64),
            required: format!("path from {} to {}", from.id, to.id),
            passed: path.is_some(),
        });

        let (a, b) = ((pa[0], pa[1]), (pb[0], pb[1]));
        let lo = SEA_LEVEL as usize;
        let hi = (lo + clear_above.max(0) as usize).min(HEIGHT);
        for lz in 0..d {
            for lx in 0..w {
                let (x, z) = (x0 + lx as i32, z0 + lz as i32);
                if nearest_on_segment((x as f64, z as f64), a, b).0 > half_width * half_width {
                    continue;
                }
                let start = region.index(lx, 0, lz);
                let col = &region.blocks[start..start + HEIGHT];
                if col[lo..hi]
                    .iter()
                    .any(|&b| b != Block::Air as u8 && b != Block::Water as u8)
                {
                    blocked.insert((x, z));
                }
            }
        }
    }
    checks.push(CheckResult {
        name: CHECK_CHANNEL_CLEAR,
        measured: blocked.len() as f64,
        required: format!(
            "== 0 channel columns with solid blocks in SEA_LEVEL..SEA_LEVEL+{clear_above}, \
             and a Channel term in the layout"
        ),
        passed: channel.is_some() && blocked.is_empty(),
    });
    ValidationReport { checks }
}
