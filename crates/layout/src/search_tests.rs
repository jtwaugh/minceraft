use super::*;
use std::sync::atomic::Ordering::Relaxed;

fn params(workers: usize) -> SearchParams {
    SearchParams {
        start: 1,
        candidates: 12,
        top: 3,
        k: 1,
        workers: Some(workers),
    }
}

fn run(p: &SearchParams, t: &Thresholds) -> (Vec<Scored>, [usize; 3]) {
    let progress = SearchProgress::default();
    let found = search_seeds(p, t, &ScoringConfig::shipped(), &progress);
    let n = |a: &std::sync::atomic::AtomicUsize| a.load(Relaxed);
    let counts = [
        n(&progress.prefilter_rejected),
        n(&progress.failed_validation),
        n(&progress.scored),
    ];
    (found, counts)
}

#[test]
fn same_ranking_with_one_and_many_workers() {
    let t = Thresholds::shipped();
    let (one, c1) = run(&params(1), &t);
    let (many, c4) = run(&params(4), &t);
    assert_eq!(one, many);
    assert_eq!(c1, c4);
    assert_eq!(c1.iter().sum::<usize>(), 12);
    assert!(!one.is_empty(), "{c1:?}");
}

#[test]
fn results_are_valid_bounded_and_ordered() {
    let t = Thresholds::shipped();
    let p = SearchParams {
        top: 2,
        ..params(2)
    };
    let (found, counts) = run(&p, &t);
    assert!(found.len() <= 2);
    assert_eq!(found.len(), counts[2].min(2));
    for s in &found {
        assert!(s.seed >= p.start && s.seed < p.start + p.candidates);
        assert_eq!(s.route, build_route(s.seed, 1));
        let (layout, levels) = build_route_layout(&WorldConfig { seed: s.seed }, &s.route);
        assert_eq!(s.levels, levels);
        assert!(validate_route(&layout, &levels, &s.route, &t).passed());
        assert!((0.0..=1.0).contains(&s.terms.path_directness));
        let t = s.terms;
        assert!((0.0..=1.0).contains(&t.climate_gradient));
        assert!((0.0..=1.0).contains(&t.biome_variety));
        let c = ScoringConfig::shipped();
        let sum = t.spacing_regularity * c.weight_spacing_regularity
            + t.island_quality * c.weight_island_quality
            + t.coastline_interest * c.weight_coastline_interest
            + t.extra_islands * c.weight_extra_islands
            + t.path_directness * c.weight_path_directness
            + t.climate_gradient * c.weight_climate_gradient
            + t.biome_variety * c.weight_biome_variety;
        assert!((s.score - sum).abs() < 1e-12);
        assert!((0.0..=6.0).contains(&s.score));
    }
    assert!(found
        .windows(2)
        .all(|w| w[0].score > w[1].score || (w[0].score == w[1].score && w[0].seed < w[1].seed)));
}

#[test]
fn a_seed_failing_full_validation_is_never_returned() {
    let t = Thresholds {
        min_island_columns: u32::MAX,
        ..Thresholds::shipped()
    };
    let (found, counts) = run(&params(2), &t);
    assert!(found.is_empty());
    assert_eq!(counts[2], 0);
    assert_eq!(counts[0] + counts[1], 12);
}
