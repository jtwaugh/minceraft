//! Layout for a multi-level route (docs/design/world-layout.md Phase E,
//! 4.1): a `Channel` per route segment plus the Phase B level terms per
//! anchorage.
//!
//! Choices made here:
//! - Every island sits on the left of the heading (decided in #631), where
//!   left is `(dz, -dx)` for heading `(dx, dz)` (north when heading east,
//!   with z growing south). The heading at an anchorage is the mean of the
//!   incoming and outgoing segment directions (the one segment at the ends;
//!   east for a single-point route).
//! - The island centre is 24 to 32 blocks (the v0 range) from the anchorage,
//!   a hash of `(seed, week)`. That is beyond `half_width + falloff` of the
//!   channel for the shipped parameters, even on the inside of a 30 degree
//!   bend.

use crate::builder::{V0_ISLAND_DIST_MAX, V0_ISLAND_DIST_MIN, V0_SPAWN_WATER_RADIUS};
use crate::route::unit;
use crate::{build_layout, DensityTerm, Layout, Level, Route, RouteParams, WorldConfig};

/// Hash stream for island distances, clear of the route builder's streams.
const ISLAND_STREAM: u64 = 1 << 32;

fn direction(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
    if len == 0.0 {
        [0.0, 0.0]
    } else {
        [(b[0] - a[0]) / len, (b[1] - a[1]) / len]
    }
}

/// Unit heading at point `i` of `points`.
fn heading(points: &[[f64; 2]], i: usize) -> [f64; 2] {
    let before = (i > 0).then(|| direction(points[i - 1], points[i]));
    let after = points.get(i + 1).map(|&b| direction(points[i], b));
    let [dx, dz] = match (before, after) {
        (Some(a), Some(b)) => [a[0] + b[0], a[1] + b[1]],
        (Some(d), None) | (None, Some(d)) => d,
        (None, None) => [1.0, 0.0],
    };
    let len = dx.hypot(dz);
    if len == 0.0 {
        [1.0, 0.0]
    } else {
        [dx / len, dz / len]
    }
}

/// The layout and levels for `route` under `world`: level `i` has week
/// `i + 1` and its anchorage at route point `i`.
pub fn build_route_layout(world: &WorldConfig, route: &Route) -> (Layout, Vec<Level>) {
    let params = RouteParams::shipped();
    let points = &route.points;
    let levels: Vec<Level> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let [hx, hz] = heading(points, i);
            let dist = V0_ISLAND_DIST_MIN
                + unit(world.seed, ISLAND_STREAM + i as u64)
                    * (V0_ISLAND_DIST_MAX - V0_ISLAND_DIST_MIN);
            Level {
                id: format!("week{}", i + 1),
                week: i as u32 + 1,
                anchorage: [p[0].round() as i32, p[1].round() as i32],
                island_center: [
                    (p[0] + dist * hz).round() as i32,
                    (p[1] - dist * hx).round() as i32,
                ],
                spawn_water_radius: V0_SPAWN_WATER_RADIUS,
            }
        })
        .collect();
    let mut layout = build_layout(world, &levels);
    for w in points.windows(2) {
        layout.density.push(DensityTerm::Channel {
            points: w.iter().map(|p| (p[0], p[1])).collect(),
            half_width: params.channel_half_width,
            falloff: params.channel_falloff,
            depth_below_sea: params.channel_depth_below_sea,
            clear_above: params.channel_clear_above,
        });
    }
    (layout, levels)
}
