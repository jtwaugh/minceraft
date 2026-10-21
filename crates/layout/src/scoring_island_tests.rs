use super::*;

fn t() -> Thresholds {
    Thresholds::shipped()
}

fn c() -> ScoringConfig {
    ScoringConfig::shipped()
}

fn disc(r: i32) -> Vec<(i32, i32)> {
    (-r..=r)
        .flat_map(|x| (-r..=r).map(move |z| (x, z)))
        .filter(|&(x, z)| x * x + z * z <= r * r)
        .collect()
}

/// A square core with 1-wide teeth along its top edge; same area as a plain
/// block of that many columns.
fn ragged(area: usize) -> Vec<(i32, i32)> {
    let mut v: Vec<(i32, i32)> = (0..10).flat_map(|x| (0..10).map(move |z| (x, z))).collect();
    let mut x = 0;
    while v.len() < area {
        for z in 10..13 {
            if v.len() < area {
                v.push((x, z));
            }
        }
        x += 2;
    }
    v
}

#[test]
fn quality_is_linear_from_zero_and_caps_at_one() {
    let (t, c) = (t(), c());
    let (a, f) = (
        t.min_island_columns as usize,
        t.min_flat_grass_tiles as usize,
    );
    assert_eq!(island_quality(0, 0, &t, &c), 0.0);
    let at_min = (1.0 / c.cap_area_factor + 1.0 / c.cap_flat_factor) / 2.0;
    assert!((island_quality(a, f, &t, &c) - at_min).abs() < 1e-12);
    let (ca, cf) = (
        (c.cap_area_factor * a as f64) as usize,
        (c.cap_flat_factor * f as f64) as usize,
    );
    assert_eq!(island_quality(ca, cf, &t, &c), 1.0);
    assert_eq!(island_quality(ca * 5, cf * 5, &t, &c), 1.0);
}

#[test]
fn island_quality_is_monotonic_and_in_range() {
    let (t, c) = (t(), c());
    let mut prev = 0.0;
    for area in 100..500 {
        let q = island_quality(area, 8, &t, &c);
        assert!((0.0..=1.0).contains(&q) && q >= prev);
        prev = q;
    }
    let mut prev = 0.0;
    for flat in 0..30 {
        let q = island_quality(200, flat, &t, &c);
        assert!((0.0..=1.0).contains(&q) && q >= prev);
        prev = q;
    }
}

#[test]
fn ragged_coast_beats_round_island_of_equal_area() {
    let c = c();
    let round = disc(8);
    let rag = ragged(round.len());
    assert_eq!(rag.len(), round.len());
    let (r, g) = (coastline_interest(&round, &c), coastline_interest(&rag, &c));
    assert!(r < g, "{r} {g}");
    assert!((0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&g));
    assert_eq!(coastline_interest(&[], &c), 0.0);
}

#[test]
fn island_scores_are_deterministic_for_a_seed() {
    let layout = Layout::new(12345);
    let l = Level {
        id: "t".into(),
        week: 1,
        anchorage: [0, 0],
        island_center: [28, 6],
        spawn_water_radius: 8.0,
    };
    let (x0, z0, w, d) = window(&l, &t());
    let region = layout.generate_region(x0, z0, w, d, false);
    let a = score_island(&region, &l, &t(), &c());
    let b = score_island(&region, &l, &t(), &c());
    assert_eq!(a, b);
    assert!((0.0..=1.0).contains(&a.0) && (0.0..=1.0).contains(&a.1));
}
