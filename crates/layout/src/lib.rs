//! World layout: bias terms that steer the terrain generator.
//! See docs/design/world-layout.md (Phase A).

use terrain::{BaseHeightBias, DensityClamp, HumidityBias, ProtectedMask, TemperatureBias};

/// Hermite smoothstep from `e0` to `e1`; `e0 > e1` gives a falling edge.
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1.0 inside `r_inner`, 0.0 beyond `r_outer`, smooth in between.
pub fn falloff(d: f64, r_inner: f64, r_outer: f64) -> f64 {
    smoothstep(r_outer, r_inner, d)
}

/// A term that shapes the terrain.
#[derive(Clone, Debug, PartialEq)]
pub enum DensityTerm {
    /// Raises (or lowers) the base height by up to `delta` blocks around
    /// `center` (world x, z): full effect inside `r_inner`, none beyond
    /// `r_outer`.
    RadialBaseHeight {
        center: (f64, f64),
        r_inner: f64,
        r_outer: f64,
        delta: f64,
    },
    /// A navigable channel along the polyline `points` (world x, z). With
    /// `d` the distance to the polyline and
    /// `w = falloff(d, half_width, half_width + falloff)`: the base height
    /// moves toward `SEA_LEVEL - depth_below_sea` by `w`, and for
    /// `SEA_LEVEL - depth_below_sea <= y < SEA_LEVEL + clear_above` the
    /// density is clamped toward -1 by `w` (no overhangs, seabed carved).
    Channel {
        points: Vec<(f64, f64)>,
        half_width: f64,
        falloff: f64,
        depth_below_sea: f64,
        clear_above: i32,
    },
}

impl BaseHeightBias for DensityTerm {
    fn bias(&self, x: i32, z: i32, base: f64) -> f64 {
        match self {
            DensityTerm::RadialBaseHeight {
                center,
                r_inner,
                r_outer,
                delta,
            } => {
                let (center, delta) = (*center, *delta);
                let d = (x as f64 - center.0).hypot(z as f64 - center.1);
                delta * falloff(d, *r_inner, *r_outer)
            }
            DensityTerm::Channel { .. } => {
                ChannelEval::new(self).map_or(0.0, |c| c.bias(x, z, base))
            }
        }
    }
}

impl DensityClamp for DensityTerm {
    fn clamp(&self, x: i32, y: i32, z: i32, density: f64) -> f64 {
        match ChannelEval::new(self) {
            Some(c) => c.clamp(x, y, z, density),
            None => density,
        }
    }
}

/// A term that shifts temperature and humidity (`Climate::at` clamps the
/// sum to [0, 1]).
#[derive(Clone, Debug, PartialEq)]
pub enum ClimateBias {
    /// Adds `d_temp` / `d_humid` scaled by `falloff(d, r_inner, r_outer)`.
    Radial {
        center: (f64, f64),
        r_inner: f64,
        r_outer: f64,
        d_temp: f64,
        d_humid: f64,
    },
    /// Deltas interpolated by arc length (start to end of the polyline) at
    /// the nearest point on it, scaled by
    /// `falloff(d, half_width, half_width + falloff)`.
    AlongPath {
        points: Vec<(f64, f64)>,
        d_temp_start: f64,
        d_temp_end: f64,
        d_humid_start: f64,
        d_humid_end: f64,
        half_width: f64,
        falloff: f64,
    },
}

fn seg_len(a: (f64, f64), b: (f64, f64)) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

impl ClimateBias {
    /// `(d_temp, d_humid)` at world column (x, z).
    pub fn deltas(&self, x: i32, z: i32) -> (f64, f64) {
        let p = (x as f64, z as f64);
        match self {
            ClimateBias::Radial {
                center,
                r_inner,
                r_outer,
                d_temp,
                d_humid,
            } => {
                let d = (p.0 - center.0).hypot(p.1 - center.1);
                let w = falloff(d, *r_inner, *r_outer);
                (d_temp * w, d_humid * w)
            }
            ClimateBias::AlongPath {
                points,
                d_temp_start,
                d_temp_end,
                d_humid_start,
                d_humid_end,
                half_width,
                falloff: fall,
            } => {
                let reach = half_width + fall;
                // Nearest segment (first wins ties); segments whose bounding
                // box, grown by `reach`, misses the column are skipped.
                let mut best: Option<(usize, f64, f64)> = None;
                let mut consider = |i: usize, a: (f64, f64), b: (f64, f64)| {
                    if p.0 < a.0.min(b.0) - reach
                        || p.0 > a.0.max(b.0) + reach
                        || p.1 < a.1.min(b.1) - reach
                        || p.1 > a.1.max(b.1) + reach
                    {
                        return;
                    }
                    let (d2, t) = nearest_on_segment(p, a, b);
                    if best.is_none_or(|(_, _, bd2)| d2 < bd2) {
                        best = Some((i, t, d2));
                    }
                };
                match points.as_slice() {
                    [] => {}
                    [a] => consider(0, *a, *a),
                    pts => pts
                        .windows(2)
                        .enumerate()
                        .for_each(|(i, w)| consider(i, w[0], w[1])),
                }
                let Some((i, t, d2)) = best else {
                    return (0.0, 0.0);
                };
                let w = falloff(d2.sqrt(), *half_width, reach);
                if w == 0.0 {
                    return (0.0, 0.0);
                }
                let total: f64 = points.windows(2).map(|w| seg_len(w[0], w[1])).sum();
                let s = if total == 0.0 {
                    0.0
                } else {
                    let before: f64 = points[..=i].windows(2).map(|w| seg_len(w[0], w[1])).sum();
                    let cur = points.get(i + 1).map_or(0.0, |&nb| seg_len(points[i], nb));
                    ((before + t * cur) / total).clamp(0.0, 1.0)
                };
                (
                    (d_temp_start + (d_temp_end - d_temp_start) * s) * w,
                    (d_humid_start + (d_humid_end - d_humid_start) * s) * w,
                )
            }
        }
    }
}

/// Temperature bias: the sum of `d_temp` over a list of terms.
#[derive(Clone, Copy, Debug)]
pub struct TemperatureTerms<'a>(pub &'a [ClimateBias]);

/// Humidity bias: the sum of `d_humid` over a list of terms.
#[derive(Clone, Copy, Debug)]
pub struct HumidityTerms<'a>(pub &'a [ClimateBias]);

impl TemperatureBias for TemperatureTerms<'_> {
    fn bias(&self, x: i32, z: i32) -> f64 {
        self.0.iter().map(|b| b.deltas(x, z).0).sum()
    }
}

impl HumidityBias for HumidityTerms<'_> {
    fn bias(&self, x: i32, z: i32) -> f64 {
        self.0.iter().map(|b| b.deltas(x, z).1).sum()
    }
}

/// A zone where decoration (trees, plants, snow) must not modify any cell.
/// Terrain is unaffected. Boundaries are inclusive: a column exactly
/// `radius` (or `half_width`) away is protected. Protection covers every y.
#[derive(Clone, Debug, PartialEq)]
pub enum Exclusion {
    Circle {
        center: (f64, f64),
        radius: f64,
    },
    /// Every column within `half_width` of the polyline through `points`.
    Corridor {
        points: Vec<(f64, f64)>,
        half_width: f64,
    },
}

/// `(squared distance, t in [0, 1])` of the point on segment `a`-`b` nearest `p`.
fn nearest_on_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dz * dz;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / len2).clamp(0.0, 1.0)
    };
    let (cx, cz) = (a.0 + t * dx, a.1 + t * dz);
    ((p.0 - cx).powi(2) + (p.1 - cz).powi(2), t)
}

/// Squared distance from `p` to the segment `a`-`b`.
fn dist2_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    nearest_on_segment(p, a, b).0
}

impl ProtectedMask for Exclusion {
    fn protected(&self, x: i32, _y: i32, z: i32) -> bool {
        let p = (x as f64, z as f64);
        match self {
            Exclusion::Circle { center, radius } => {
                (p.0 - center.0).powi(2) + (p.1 - center.1).powi(2) <= radius * radius
            }
            Exclusion::Corridor { points, half_width } => {
                let h = *half_width;
                let hit = |a: (f64, f64), b: (f64, f64)| {
                    // Per-segment bounding box (grown by the half-width) first.
                    p.0 >= a.0.min(b.0) - h
                        && p.0 <= a.0.max(b.0) + h
                        && p.1 >= a.1.min(b.1) - h
                        && p.1 <= a.1.max(b.1) + h
                        && dist2_to_segment(p, a, b) <= h * h
                };
                match points.as_slice() {
                    [] => false,
                    [a] => hit(*a, *a),
                    pts => pts.windows(2).any(|w| hit(w[0], w[1])),
                }
            }
        }
    }
}

/// A set of exclusions; protects a cell if any member does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exclusions(pub Vec<Exclusion>);

impl ProtectedMask for Exclusions {
    fn protected(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.iter().any(|e| e.protected(x, y, z))
    }
}

mod builder;
pub use builder::{build_layout, default_v0_level};
mod cache;
pub use cache::{CacheChange, RegionCache};
mod channel;
pub use channel::ChannelEval;
mod corridor;
pub use corridor::Corridor;
mod derived;
pub use derived::{DerivedMaps, NO_BOTTOM, SHORE_DISTANCE_CAP, SHORE_PAD};
mod fish;
pub use fish::{Fish, FishColumn, Species};
#[cfg(feature = "toml-io")]
mod fish_rules;
mod habitat;
mod habitat_rule;
#[cfg(feature = "toml-io")]
mod habitat_rules;
pub use habitat::{classify, HabitatId, HabitatInputs, HabitatRule, Habitats};
mod habitat_map;
pub use habitat_map::{HabitatMap, NO_HABITAT};
mod level;
pub use level::{Level, LevelFile, LevelFileError, Route as LevelRoute, WorldConfig};
#[cfg(feature = "toml-io")]
mod seed_output;
#[cfg(feature = "toml-io")]
pub use seed_output::{level_file, render, Args, USAGE};
mod navigability;
pub use navigability::NavMap;
mod pathfind;
pub use pathfind::{erode, find_path};
mod prefilter;
pub use prefilter::{prefilter, prefilter_layout, prefilter_regions, PrefilterReject};
mod region;
#[cfg(feature = "toml-io")]
mod rule_check;
pub use region::{generate_region, Layout, LayoutRegion, StructurePlacement};
mod region_coord;
pub use region_coord::{RegionCoord, REGION_SIZE};
mod retry;
pub use retry::{find_level, find_v0_level, LevelFound, RetryError};
mod route;
pub use route::{build_route, build_route_with, Route, RouteParams};
mod route_validation;
pub use route_validation::{
    anchorage_paths, validate_route, CHECK_ANCHORAGE_REACHABLE, CHECK_CHANNEL_CLEAR,
};
mod route_layout;
pub use route_layout::build_route_layout;
mod route_retry;
pub use route_retry::{find_route, RouteFound};
mod scoring;
mod scoring_island;
pub use scoring::{path_directness, spacing_regularity, ScoringConfig};
pub use scoring_island::{coastline_interest, island_quality, score_island};
mod search;
pub use search::{search_seeds, Scored, SearchParams, SearchProgress, TermScores};
mod scoring_extra;
pub use scoring_extra::{extra_islands, ExtraIslands};
mod structure;
mod structure_error;
#[cfg(feature = "toml-io")]
mod structure_rules;
pub use structure::{Structure, StructureError};
mod validation;
mod validation_decorated;
pub use validation::{
    raft_footprint, validate_flat_grass, validate_island, validate_level, validate_raft, window,
    CheckResult, Thresholds, ValidationReport, CHECK_CLEAR_FLAT, CHECK_FLAT_GRASS,
    CHECK_ISLAND_AREA, CHECK_RAFT_CLEAR, CHECK_RAFT_WATER, CHECK_SPAWN_CLEAR, CHECK_TREES,
};
pub use validation_decorated::validate_decorated;

#[cfg(test)]
mod builder_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod channel_tests;
#[cfg(test)]
mod climate_tests;
#[cfg(test)]
mod corridor_tests;
#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod exclusion_tests;
#[cfg(all(test, feature = "toml-io"))]
mod fish_tests;
#[cfg(test)]
mod habitat_map_tests;
#[cfg(test)]
mod habitat_tests;
#[cfg(all(test, feature = "toml-io"))]
mod level_tests;
#[cfg(test)]
mod navigability_tests;
#[cfg(test)]
mod pathfind_tests;
#[cfg(test)]
mod prefilter_tests;
#[cfg(test)]
mod raft_tests;
#[cfg(test)]
mod region_coord_tests;
#[cfg(test)]
mod region_tests;
#[cfg(test)]
mod retry_tests;
#[cfg(test)]
mod route_layout_tests;
#[cfg(test)]
mod route_retry_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod route_validation_tests;
#[cfg(test)]
mod scoring_extra_tests;
#[cfg(test)]
mod scoring_island_tests;
#[cfg(test)]
mod scoring_tests;
#[cfg(test)]
mod search_tests;
#[cfg(all(test, feature = "toml-io"))]
mod seed_output_tests;
#[cfg(test)]
mod structure_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_decorated_tests;
#[cfg(test)]
mod validation_tests;
