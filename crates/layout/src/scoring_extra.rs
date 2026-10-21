//! Extra-islands scoring term (docs/design/world-layout.md 6.3, visibility
//! decided in #743, cap in #742).
//!
//! Choices made here:
//! - Land is as in validation: topmost non-air block not water or ice, at
//!   `y >= SEA_LEVEL`. Land regions are 4-connected components of the region.
//! - An island is an anchorage island if it contains a level's
//!   `island_center`; those are not counted.
//! - Viewpoints are the route polyline sampled every `VIEW_STEP` blocks (ends
//!   included), at `SEA_LEVEL + eye_height`. Viewpoints outside the region
//!   are skipped.
//! - Line of sight runs from a viewpoint to the top of the island's highest
//!   column (the first in x, z order on ties). It is blocked when a column
//!   along the way, other than the island's own, has its top surface above the
//!   line. Columns outside the region never block.

use terrain::{column_summary, Block, Region, HEIGHT, SEA_LEVEL};

use crate::validation::{is_land, island_columns};
use crate::ScoringConfig;

/// Distance between viewpoints along the route, in blocks.
const VIEW_STEP: f64 = 8.0;

/// Result of the extra-islands term.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtraIslands {
    /// Non-anchorage islands visible from the route.
    pub count: usize,
    /// `min(count / cap_extra_islands, 1)`, in 0..1.
    pub term: f64,
}

fn top_y(region: &Region, x: i32, z: i32) -> Option<i32> {
    let (lx, lz) = (x as i64 - region.x0 as i64, z as i64 - region.z0 as i64);
    if lx < 0 || lz < 0 || lx >= region.width as i64 || lz >= region.depth as i64 {
        return None;
    }
    let start = region.index(lx as u32, 0, lz as u32);
    let (top, y, _, _) = column_summary(&region.blocks[start..start + HEIGHT]);
    (top != Block::Air as u8).then_some(y as i32)
}

fn mark(region: &Region, claimed: &mut [bool], cols: &[(i32, i32)]) {
    for &(x, z) in cols {
        claimed[(z - region.z0) as usize * region.width as usize + (x - region.x0) as usize] = true;
    }
}

fn viewpoints(route: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = route.first().copied().into_iter().collect();
    for seg in route.windows(2) {
        let len = (seg[1][0] - seg[0][0]).hypot(seg[1][1] - seg[0][1]);
        let n = (len / VIEW_STEP).ceil().max(1.0) as usize;
        for i in 1..=n {
            let t = i as f64 / n as f64;
            out.push([
                seg[0][0] + (seg[1][0] - seg[0][0]) * t,
                seg[0][1] + (seg[1][1] - seg[0][1]) * t,
            ]);
        }
    }
    out
}

fn sees(
    region: &Region,
    eye: [f64; 2],
    eye_y: f64,
    target: (i32, i32, i32),
    own: &[(i32, i32)],
) -> bool {
    let (tx, tz, ty) = (
        target.0 as f64 + 0.5,
        target.1 as f64 + 0.5,
        target.2 as f64 + 1.0,
    );
    let len = (tx - eye[0]).hypot(tz - eye[1]);
    let steps = len.ceil() as usize;
    (1..steps).all(|i| {
        let t = i as f64 / steps as f64;
        let (x, z) = (
            (eye[0] + (tx - eye[0]) * t).floor() as i32,
            (eye[1] + (tz - eye[1]) * t).floor() as i32,
        );
        let line_y = eye_y + (ty - eye_y) * t;
        match top_y(region, x, z) {
            Some(y) if own.binary_search(&(x, z)).is_err() => (y as f64 + 1.0) <= line_y,
            _ => true,
        }
    })
}

/// Counts the non-anchorage islands of `region` visible from `route` and
/// normalizes the count. `anchorage_centres` are the levels'
/// `island_center`s. A region with no route points scores 0.
pub fn extra_islands(
    region: &Region,
    anchorage_centres: &[[i32; 2]],
    route: &[[f64; 2]],
    cfg: &ScoringConfig,
) -> ExtraIslands {
    let mut claimed = vec![false; region.width as usize * region.depth as usize];
    for &c in anchorage_centres {
        mark(region, &mut claimed, &island_columns(region, c));
    }
    let eyes = viewpoints(route);
    let eye_y = SEA_LEVEL as f64 + cfg.eye_height;
    let mut count = 0;
    for z in 0..region.depth {
        for x in 0..region.width {
            let i = z as usize * region.width as usize + x as usize;
            if claimed[i] || !is_land(region, x, z) {
                continue;
            }
            let island = island_columns(region, [x as i32 + region.x0, z as i32 + region.z0]);
            mark(region, &mut claimed, &island);
            let peak = island
                .iter()
                .filter_map(|&(cx, cz)| top_y(region, cx, cz).map(|y| (cx, cz, y)))
                .max_by_key(|&(cx, cz, y)| (y, std::cmp::Reverse((cx, cz))));
            let Some(peak) = peak else { continue };
            if eyes.iter().any(|&e| sees(region, e, eye_y, peak, &island)) {
                count += 1;
            }
        }
    }
    let term = if cfg.cap_extra_islands > 0.0 {
        (count as f64 / cfg.cap_extra_islands).min(1.0)
    } else {
        f64::from(count > 0)
    };
    ExtraIslands { count, term }
}
