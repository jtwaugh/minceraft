//! Island terms of seed scoring (docs/design/world-layout.md 6.3, caps
//! decided in #742): island quality and coastline interest for one level.
//!
//! Choices made here:
//! - Perimeter is the number of island-column edges that touch a column
//!   outside the island (4-neighbourhood), so a holed island counts its
//!   inner coast too.
//! - An empty island scores 0.0 on both terms.
//! - Island quality averages two surplus ratios, each 0.0 at the validation
//!   minimum and 1.0 at `cap_factor` times it, linear between.

use std::collections::HashSet;

use crate::validation::{flat_grass_tiles, island_columns};
use terrain::Region;

use crate::{Level, ScoringConfig, Thresholds};

/// `(value - min) / (factor * min - min)` clamped to 0..1: 0 at the minimum,
/// 1 at `factor` times it, linear between.
fn surplus(value: f64, min: f64, factor: f64) -> f64 {
    let span = factor * min - min;
    if span <= 0.0 {
        return if value > min { 1.0 } else { 0.0 };
    }
    ((value - min) / span).clamp(0.0, 1.0)
}

/// Mean of the area surplus and the flat-tile surplus over the validation
/// minimums, in 0..1.
pub fn island_quality(area: usize, flat_tiles: usize, t: &Thresholds, c: &ScoringConfig) -> f64 {
    let a = surplus(
        area as f64,
        f64::from(t.min_island_columns),
        c.cap_area_factor,
    );
    let f = surplus(
        flat_tiles as f64,
        f64::from(t.min_flat_grass_tiles),
        c.cap_flat_factor,
    );
    (a + f) / 2.0
}

/// `min(perimeter / sqrt(area) / cap_ratio, 1)` for the island `columns`.
pub fn coastline_interest(columns: &[(i32, i32)], c: &ScoringConfig) -> f64 {
    if columns.is_empty() || c.cap_ratio <= 0.0 {
        return 0.0;
    }
    let set: HashSet<(i32, i32)> = columns.iter().copied().collect();
    let perimeter = set
        .iter()
        .map(|&(x, z)| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .filter(|&&(dx, dz)| !set.contains(&(x + dx, z + dz)))
                .count()
        })
        .sum::<usize>();
    (perimeter as f64 / (set.len() as f64).sqrt() / c.cap_ratio).clamp(0.0, 1.0)
}

/// `(island_quality, coastline_interest)` for `level`'s island on an
/// undecorated `region` covering [`crate::window`].
pub fn score_island(
    region: &Region,
    level: &Level,
    t: &Thresholds,
    c: &ScoringConfig,
) -> (f64, f64) {
    let island = island_columns(region, level.island_center);
    let flat = flat_grass_tiles(region, &island);
    (
        island_quality(island.len(), flat, t, c),
        coastline_interest(&island, c),
    )
}
