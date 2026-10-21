//! Route builder (docs/design/world-layout.md Phase E): an ordered list of
//! anchorages. Every draw is a hash of `(seed, stream)`, so the route is a
//! pure function of the seed. Defaults come from `data/route.toml`.
//!
//! Choices made here:
//! - Points are `f64` world (x, z), unrounded, so the spacing bounds hold
//!   exactly.
//! - The first step goes along the hashed initial heading; each later step
//!   turns by a uniform draw in `[-max_turn, max_turn]` first.

use terrain::noise::mix64;

/// Defaults from `data/route.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteParams {
    pub spacing: f64,
    pub spacing_spread: f64,
    pub max_turn_degrees: f64,
    pub channel_half_width: f64,
    pub channel_falloff: f64,
    pub channel_depth_below_sea: f64,
    pub channel_clear_above: i32,
}

include!(concat!(env!("OUT_DIR"), "/route.rs"));

impl RouteParams {
    /// The shipped `data/route.toml`, baked in at build time.
    pub fn shipped() -> Self {
        SHIPPED_ROUTE.clone()
    }
}

/// Anchorages in order; the first is the world origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<[f64; 2]>,
}

/// Unit float in [0, 1) from the hash of `seed` and a stream number.
pub(crate) fn unit(seed: u64, stream: u64) -> f64 {
    let mix = |z: u64| mix64(z.wrapping_add(0x9E37_79B9_7F4A_7C15));
    (mix(seed ^ mix(stream)) >> 11) as f64 / (1u64 << 53) as f64
}

/// A route of `k` further anchorages (`k + 1` points) with the shipped
/// defaults.
pub fn build_route(seed: u64, k: usize) -> Route {
    build_route_with(seed, k, &RouteParams::shipped())
}

/// As `build_route`, with explicit parameters.
pub fn build_route_with(seed: u64, k: usize, params: &RouteParams) -> Route {
    let mut heading = unit(seed, 0) * std::f64::consts::TAU;
    let max_turn = params.max_turn_degrees.to_radians();
    let mut points = vec![[0.0, 0.0]];
    for step in 1..=k as u64 {
        if step > 1 {
            heading += (unit(seed, 2 * step) * 2.0 - 1.0) * max_turn;
        }
        let dist = params.spacing + (unit(seed, 2 * step + 1) * 2.0 - 1.0) * params.spacing_spread;
        let [x, z] = points[step as usize - 1];
        points.push([x + dist * heading.cos(), z + dist * heading.sin()]);
    }
    Route { points }
}
