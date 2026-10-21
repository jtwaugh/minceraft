use super::*;
use crate::{build_route, find_path, NavMap};

fn params() -> RouteParams {
    RouteParams::shipped()
}

#[test]
fn shipped_matches_file_text() {
    let text = include_str!("../../../data/scoring.toml");
    assert_eq!(
        ScoringConfig::from_toml(text).unwrap(),
        ScoringConfig::shipped()
    );
    let c = ScoringConfig::shipped();
    assert_eq!(c.weight_spacing_regularity, 1.0);
    assert_eq!(c.weight_coastline_interest, 0.5);
}

#[test]
fn from_toml_names_missing_and_unknown_keys() {
    let text = include_str!("../../../data/scoring.toml");
    let missing = text.replace("cap_ratio = 6.0", "");
    let err = ScoringConfig::from_toml(&missing).unwrap_err();
    assert!(err.contains("cap_ratio"), "{err}");
    let err = ScoringConfig::from_toml(&format!("{text}\nbogus_key = 1\n")).unwrap_err();
    assert!(err.contains("bogus_key"), "{err}");
}

#[test]
fn spacing_exact_is_one_and_deviation_lowers() {
    let p = params();
    let exact = [[0.0, 0.0], [p.spacing, 0.0], [p.spacing, p.spacing]];
    assert_eq!(spacing_regularity(&exact, &p), 1.0);
    let half = [[0.0, 0.0], [p.spacing + p.spacing_spread / 2.0, 0.0]];
    assert!((spacing_regularity(&half, &p) - 0.5).abs() < 1e-9);
    let far = [[0.0, 0.0], [p.spacing * 10.0, 0.0]];
    assert_eq!(spacing_regularity(&far, &p), 0.0);
    let near = [[0.0, 0.0], [1.0, 0.0]];
    assert_eq!(spacing_regularity(&near, &p), 0.0);
    assert_eq!(spacing_regularity(&[[0.0, 0.0]], &p), 0.0);
}

#[test]
fn spacing_with_zero_spread() {
    let p = RouteParams {
        spacing_spread: 0.0,
        ..params()
    };
    let on = [[0.0, 0.0], [p.spacing, 0.0]];
    let off = [[0.0, 0.0], [p.spacing + 1.0, 0.0]];
    assert_eq!(spacing_regularity(&on, &p), 1.0);
    assert_eq!(spacing_regularity(&off, &p), 0.0);
}

#[test]
fn directness_straight_is_one_and_detour_is_lower() {
    let open = NavMap {
        width: 10,
        depth: 10,
        navigable: vec![true; 100],
    };
    let straight = find_path(&open, [0, 0], [9, 0]).unwrap();
    assert_eq!(path_directness(&[straight]), 1.0);
    let diagonal = find_path(&open, [0, 0], [9, 9]).unwrap();
    assert!((path_directness(&[diagonal]) - 1.0).abs() < 1e-9);

    // A wall with a gap at the far end forces a detour.
    let mut wall = open.clone();
    for z in 0..9 {
        wall.navigable[z * 10 + 5] = false;
    }
    let detour = find_path(&wall, [0, 0], [9, 0]).unwrap();
    let d = path_directness(&[detour]);
    assert!(d > 0.0 && d < 1.0, "{d}");
}

#[test]
fn directness_stays_in_range_on_odd_input() {
    assert_eq!(path_directness(&[]), 0.0);
    assert_eq!(path_directness(&[vec![]]), 0.0);
    assert_eq!(path_directness(&[vec![[3, 3]]]), 1.0);
    let back_and_forth = vec![[0, 0], [5, 0], [0, 0]];
    assert_eq!(path_directness(&[back_and_forth]), 0.0);
}

#[test]
fn scores_are_deterministic_for_a_seed() {
    let p = params();
    let score = |seed| spacing_regularity(&build_route(seed, 5).points, &p);
    assert_eq!(score(12345), score(12345));
    let s = score(12345);
    assert!((0.0..=1.0).contains(&s));
}
