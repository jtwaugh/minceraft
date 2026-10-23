use super::*;
use crate::route_climate_stats::cold_then_warm;
use crate::{build_route, Level, TermScores};
use terrain::Biome;

fn world(seed: u64) -> Scored {
    let route = build_route(seed, 2);
    let (_, levels): (_, Vec<Level>) = build_route_layout(&WorldConfig { seed }, &route);
    Scored {
        seed,
        route,
        levels,
        terms: TermScores {
            spacing_regularity: 0.0,
            island_quality: 0.0,
            coastline_interest: 0.0,
            extra_islands: 0.0,
            path_directness: 0.0,
            climate_gradient: 0.0,
            biome_variety: 0.0,
        },
        score: 0.0,
    }
}

#[test]
fn share_handles_empty_input() {
    assert_eq!(share(0, 0), 0.0);
    assert_eq!(share(3, 4), 0.75);
}

#[test]
fn pairs_count_only_known_different_biomes() {
    let (a, b) = (Some(Biome::Tundra), Some(Biome::Desert));
    assert_eq!(different_biome_pairs(&[]), (0, 0));
    assert_eq!(different_biome_pairs(&[a]), (0, 0));
    assert_eq!(different_biome_pairs(&[a, b, b, a]), (2, 3));
    assert_eq!(different_biome_pairs(&[a, None, a]), (0, 2));
}

#[test]
fn totals_sum_over_worlds_and_match_the_per_world_counts() {
    let worlds = [world(12345), world(7)];
    let (diff, total) = biome_pair_totals(&worlds);
    assert_eq!(total, 4);
    let each: usize = worlds
        .iter()
        .map(|w| different_biome_pairs(&anchorage_biomes(w)).0)
        .sum();
    assert_eq!(diff, each);
    let (hits, n) = cold_first_warm_last(&worlds);
    assert_eq!(n, 2);
    assert_eq!(
        hits,
        worlds.iter().filter(|w| is_cold_first_warm_last(w)).count()
    );
}

#[test]
fn no_worlds_count_nothing() {
    assert_eq!(cold_first_warm_last(&[]), (0, 0));
    assert_eq!(biome_pair_totals(&[]), (0, 0));
}

#[test]
fn cold_then_warm_counts_known_inputs() {
    assert!(cold_then_warm(Some((true, true))));
    assert!(!cold_then_warm(Some((true, false))));
    assert!(!cold_then_warm(Some((false, true))));
    assert!(!cold_then_warm(Some((false, false))));
    assert!(!cold_then_warm(None));
}
