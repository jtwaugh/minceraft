use super::*;
use crate::{build_route, build_route_layout, ClimateBias, WorldConfig};

const SEED: u64 = 12345;

fn shipped() -> (Layout, Vec<Level>) {
    build_route_layout(&WorldConfig { seed: SEED }, &build_route(SEED, 4))
}

#[test]
fn thresholds_classify_cold_warm_and_neither() {
    assert!(is_cold(0.49) && !is_warm(0.49));
    assert!(!is_cold(0.5) && !is_warm(0.5));
    assert!(!is_cold(0.9) && !is_warm(0.9));
    assert!(is_warm(1.0) && !is_cold(1.0));
    assert!(!is_warm(62.0 / 63.0));
}

#[test]
fn first_island_cold_last_warm_with_the_shipped_bias() {
    let (layout, levels) = shipped();
    let (first, last) = (&levels[0], &levels[4]);
    assert!(is_cold(anchorage_temperature(&layout, first)));
    assert!(is_warm(anchorage_temperature(&layout, last)));
    assert!(island_is_cold(&layout, first) && !island_is_warm(&layout, first));
    assert!(island_is_warm(&layout, last) && !island_is_cold(&layout, last));
}

#[test]
fn without_the_bias_the_result_does_not_shift() {
    let (mut layout, levels) = shipped();
    layout.climate.clear();
    let first = anchorage_temperature(&layout, &levels[0]);
    let biased = anchorage_temperature(&shipped().0, &levels[0]);
    assert!(first > biased);
    let last = anchorage_temperature(&layout, &levels[4]);
    assert!(last < anchorage_temperature(&shipped().0, &levels[4]));
    assert!(!is_warm(last));
}

#[test]
fn neither_cold_nor_warm_in_the_middle() {
    let (mut layout, levels) = shipped();
    layout.climate.clear();
    let level = &levels[2];
    let raw = anchorage_temperature(&layout, level);
    let c = level.anchorage;
    layout.climate.push(ClimateBias::Radial {
        center: (f64::from(c[0]), f64::from(c[1])),
        r_inner: 500.0,
        r_outer: 600.0,
        d_temp: 0.75 - raw,
        d_humid: 0.0,
    });
    let t = anchorage_temperature(&layout, level);
    assert!(!is_cold(t) && !is_warm(t), "T {t}");
    assert!(!island_is_cold(&layout, level) && !island_is_warm(&layout, level));
}

#[test]
fn island_biome_is_deterministic_and_matches_the_columns() {
    let (layout, levels) = shipped();
    let level = &levels[1];
    let a = island_biome(&layout, level);
    assert!(a.is_some());
    assert_eq!(a, island_biome(&layout, level));
    // A larger window around the same island sees the same land.
    let wider = Level {
        spawn_water_radius: level.spawn_water_radius + 8.0,
        ..level.clone()
    };
    assert_eq!(a, island_biome(&layout, &wider));
}
