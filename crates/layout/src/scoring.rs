//! Seed scoring (docs/design/world-layout.md 6.3, decided in #742): weights
//! and caps from `data/scoring.toml`, and the two terms that need only the
//! route.
//!
//! Choices made here:
//! - With no gaps (fewer than two anchorages) or no paths, a term is 0.0, so
//!   an empty route never outscores a real one.
//! - A path of zero length (both ends the same column) counts as ratio 1.0.
//! - A zero `spacing_spread` scores 1.0 for a gap exactly on `spacing` and
//!   0.0 otherwise.

use crate::RouteParams;

/// Weights and caps from `data/scoring.toml`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "toml-io", derive(serde::Deserialize))]
#[cfg_attr(feature = "toml-io", serde(deny_unknown_fields))]
pub struct ScoringConfig {
    pub weight_spacing_regularity: f64,
    pub weight_path_directness: f64,
    pub weight_island_quality: f64,
    pub weight_coastline_interest: f64,
    pub weight_extra_islands: f64,
    /// Island quality: area reaches 1.0 at this multiple of the minimum.
    pub cap_area_factor: f64,
    /// Island quality: flat tiles reach 1.0 at this multiple of the minimum.
    pub cap_flat_factor: f64,
    /// Coastline interest reaches 1.0 at this perimeter / sqrt(area).
    pub cap_ratio: f64,
    /// Extra islands: the count that reaches 1.0.
    pub cap_extra_islands: f64,
    /// Extra islands: height above `SEA_LEVEL` of the viewpoint on the route.
    pub eye_height: f64,
}

include!(concat!(env!("OUT_DIR"), "/scoring.rs"));

impl ScoringConfig {
    /// Parses a config; a missing or unknown key is an error naming it.
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The shipped `data/scoring.toml`, baked in at build time.
    pub fn shipped() -> Self {
        SHIPPED_SCORING.clone()
    }
}

/// Mean over consecutive anchorage gaps of
/// `1 - min(|gap - spacing| / spacing_spread, 1)`, in 0..1.
pub fn spacing_regularity(points: &[[f64; 2]], params: &RouteParams) -> f64 {
    let gaps: Vec<f64> = points
        .windows(2)
        .map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]))
        .collect();
    if gaps.is_empty() {
        return 0.0;
    }
    let score = |gap: f64| {
        let off = (gap - params.spacing).abs();
        if params.spacing_spread > 0.0 {
            1.0 - (off / params.spacing_spread).min(1.0)
        } else if off == 0.0 {
            1.0
        } else {
            0.0
        }
    };
    let mean = gaps.iter().map(|&g| score(g)).sum::<f64>() / gaps.len() as f64;
    mean.clamp(0.0, 1.0)
}

/// Mean over `paths` (one A* path per consecutive anchorage pair, from
/// `find_path`) of straight-line distance between the ends over path
/// length, clamped to 0..1.
pub fn path_directness(paths: &[Vec<[i32; 2]>]) -> f64 {
    if paths.is_empty() {
        return 0.0;
    }
    let dist = |a: [i32; 2], b: [i32; 2]| f64::from(b[0] - a[0]).hypot(f64::from(b[1] - a[1]));
    let ratio = |path: &Vec<[i32; 2]>| {
        let (Some(&first), Some(&last)) = (path.first(), path.last()) else {
            return 0.0;
        };
        let length: f64 = path.windows(2).map(|p| dist(p[0], p[1])).sum();
        if length == 0.0 {
            1.0
        } else {
            (dist(first, last) / length).clamp(0.0, 1.0)
        }
    };
    paths.iter().map(ratio).sum::<f64>() / paths.len() as f64
}
