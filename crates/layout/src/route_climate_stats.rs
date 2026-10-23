//! Counting for the route climate statistics example (docs/design/
//! world-layout.md Phase G "Done when"; decided in #844 and #845).
//!
//! Choices made here:
//! - A world counts as cold-first/warm-last when `island_is_cold` holds for
//!   the first level and `island_is_warm` for the last.
//! - A consecutive pair of anchorage islands differs when both have a biome
//!   and the biomes are not equal. There is no exemption for short routes
//!   (#845), so every consecutive pair counts.

use terrain::Biome;

use crate::{
    build_route_layout, island_biome, island_is_cold, island_is_warm, Scored, WorldConfig,
};

/// `(hits, total)` as a share in 0..=1; 0 when `total` is 0.
pub fn share(hits: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        hits as f64 / total as f64
    }
}

/// `(pairs with different biomes, consecutive pairs)` over `biomes`.
pub fn different_biome_pairs(biomes: &[Option<Biome>]) -> (usize, usize) {
    let pairs = biomes.windows(2);
    let total = pairs.len();
    let different = pairs
        .filter(|w| matches!((w[0], w[1]), (Some(a), Some(b)) if a != b))
        .count();
    (different, total)
}

/// Whether the world's first island is cold and its last warm.
pub fn is_cold_first_warm_last(s: &Scored) -> bool {
    let (layout, levels) = build_route_layout(&WorldConfig { seed: s.seed }, &s.route);
    let ends = match (levels.first(), levels.last()) {
        (Some(first), Some(last)) => Some((
            island_is_cold(&layout, first),
            island_is_warm(&layout, last),
        )),
        _ => None,
    };
    cold_then_warm(ends)
}

/// Whether `(first is cold, last is warm)` holds; no islands never counts.
pub fn cold_then_warm(ends: Option<(bool, bool)>) -> bool {
    matches!(ends, Some((true, true)))
}

/// Anchorage island biomes of the world, in route order.
pub fn anchorage_biomes(s: &Scored) -> Vec<Option<Biome>> {
    let (layout, levels) = build_route_layout(&WorldConfig { seed: s.seed }, &s.route);
    levels.iter().map(|l| island_biome(&layout, l)).collect()
}

/// Worlds that are cold-first/warm-last, out of `worlds.len()`.
pub fn cold_first_warm_last(worlds: &[Scored]) -> (usize, usize) {
    let hits = worlds.iter().filter(|s| is_cold_first_warm_last(s)).count();
    (hits, worlds.len())
}

/// Pairs with different biomes and all pairs, summed over `worlds`.
pub fn biome_pair_totals(worlds: &[Scored]) -> (usize, usize) {
    worlds
        .iter()
        .map(|s| different_biome_pairs(&anchorage_biomes(s)))
        .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
}
