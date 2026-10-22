//! Biome measurements on the route (docs/design/world-layout.md Phase G,
//! decided in #844): the biased temperature at an anchorage, the biome that
//! stands for an island, and cold/warm classification.
//!
//! Choices made here:
//! - Cold is `T < cold_below` and warm is `T > warm_above`, from
//!   `data/route.toml`.
//! - An island's land columns are those of `island_columns` on the
//!   undecorated `window(level, t)` region of the layout.
//! - An island is cold (warm) when more than half of its land columns are.
//! - The island biome is the most common biome among its land columns, with
//!   the biased climate; ties go to the lowest biome id. An island with no
//!   land has no biome and is neither cold nor warm.

use terrain::{biome_for, Biome, Climate};

use crate::validation::{island_columns, window};
use crate::{HumidityTerms, Layout, Level, RouteParams, TemperatureTerms, Thresholds};

/// `T < cold_below`.
pub fn is_cold(t: f64) -> bool {
    t < RouteParams::shipped().cold_below
}

/// `T > warm_above`.
pub fn is_warm(t: f64) -> bool {
    t > RouteParams::shipped().warm_above
}

/// Biased `(T, H)` at each column of `cols`, in order.
fn climates(layout: &Layout, cols: &[(i32, i32)]) -> Vec<(f64, f64)> {
    let climate = Climate::new(layout.seed);
    let (tt, ht) = (
        TemperatureTerms(&layout.climate),
        HumidityTerms(&layout.climate),
    );
    cols.iter()
        .map(|&(x, z)| climate.at(x, z, &[&tt], &[&ht]))
        .collect()
}

/// Biased temperature at the level's anchorage.
pub fn anchorage_temperature(layout: &Layout, level: &Level) -> f64 {
    let [x, z] = level.anchorage;
    climates(layout, &[(x, z)])[0].0
}

/// Biased `(T, H)` of every land column of the level's island.
fn island_climates(layout: &Layout, level: &Level) -> Vec<(f64, f64)> {
    let (x0, z0, w, d) = window(level, &Thresholds::shipped());
    let region = layout.generate_region(x0, z0, w, d, false);
    climates(layout, &island_columns(&region, level.island_center))
}

/// The most common biome among the island's land columns; `None` if the
/// island has no land.
pub fn island_biome(layout: &Layout, level: &Level) -> Option<Biome> {
    let mut counts = [0usize; Biome::COUNT];
    for (t, h) in island_climates(layout, level) {
        counts[biome_for(t, h) as usize] += 1;
    }
    let best = counts.iter().copied().max().filter(|&n| n > 0)?;
    counts
        .iter()
        .position(|&n| n == best)
        .map(|i| Biome::from_u8(i as u8))
}

fn mostly(layout: &Layout, level: &Level, test: impl Fn(f64) -> bool) -> bool {
    let cols = island_climates(layout, level);
    cols.iter().filter(|c| test(c.0)).count() * 2 > cols.len()
}

/// Most of the island's land columns are cold.
pub fn island_is_cold(layout: &Layout, level: &Level) -> bool {
    mostly(layout, level, is_cold)
}

/// Most of the island's land columns are warm.
pub fn island_is_warm(layout: &Layout, level: &Level) -> bool {
    mostly(layout, level, is_warm)
}
