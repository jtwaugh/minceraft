use crate::scoring_climate::*;
use crate::ScoringConfig;
use terrain::Biome;

fn cfg() -> ScoringConfig {
    ScoringConfig::shipped()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-12, "{a} vs {b}");
}

#[test]
fn defaults_weigh_both_terms_fully() {
    let c = cfg();
    assert_eq!(
        (c.weight_climate_gradient, c.weight_biome_variety),
        (1.0, 1.0)
    );
    assert_eq!((c.cap_temp_change, c.cap_biomes), (0.5, 4.0));
}

#[test]
fn gradient_rising_series_scores_one() {
    close(climate_gradient(&[0.1, 0.4, 0.7, 1.0], &cfg()), 1.0);
}

#[test]
fn gradient_partial_rise_and_a_dip() {
    // Rise 0.25 of cap 0.5 -> 0.5; steps: up, down, up -> 2/3.
    close(
        climate_gradient(&[0.25, 0.5, 0.4, 0.5], &cfg()),
        (0.5 + 2.0 / 3.0) / 2.0,
    );
}

#[test]
fn gradient_flat_series_has_no_change_but_is_monotone() {
    close(climate_gradient(&[0.5, 0.5, 0.5], &cfg()), 0.5);
}

#[test]
fn gradient_falling_series_scores_zero() {
    close(climate_gradient(&[1.0, 0.7, 0.2], &cfg()), 0.0);
}

#[test]
fn gradient_one_or_no_anchorage_scores_zero() {
    assert_eq!(climate_gradient(&[0.7], &cfg()), 0.0);
    assert_eq!(climate_gradient(&[], &cfg()), 0.0);
}

#[test]
fn gradient_zero_cap_is_defined() {
    let c = ScoringConfig {
        cap_temp_change: 0.0,
        ..cfg()
    };
    close(climate_gradient(&[0.1, 0.2], &c), 1.0);
    close(climate_gradient(&[0.2, 0.1], &c), 0.0);
}

#[test]
fn variety_all_same_and_all_different() {
    let same = [Some(Biome::Plains); 5];
    close(biome_variety(&same, &cfg()), 0.25);
    let all = [
        Some(Biome::Tundra),
        Some(Biome::Forest),
        Some(Biome::Plains),
        Some(Biome::Desert),
        Some(Biome::Swampland),
    ];
    close(biome_variety(&all, &cfg()), 1.0);
}

#[test]
fn variety_counts_distinct_and_skips_none() {
    let b = [
        Some(Biome::Tundra),
        None,
        Some(Biome::Forest),
        Some(Biome::Tundra),
    ];
    close(biome_variety(&b, &cfg()), 0.5);
    assert_eq!(biome_variety(&[], &cfg()), 0.0);
}

#[test]
fn from_toml_rejects_missing_new_keys() {
    let text = include_str!("../../../data/scoring.toml");
    for key in [
        "weight_climate_gradient = 1.0",
        "weight_biome_variety = 1.0",
        "cap_temp_change = 0.5",
        "cap_biomes = 4.0",
    ] {
        let err = ScoringConfig::from_toml(&text.replace(key, "")).unwrap_err();
        assert!(err.contains(key.split(' ').next().unwrap()), "{err}");
    }
}
