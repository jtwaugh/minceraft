use super::*;

fn headings(r: &Route) -> Vec<f64> {
    r.points
        .windows(2)
        .map(|w| (w[1][1] - w[0][1]).atan2(w[1][0] - w[0][0]))
        .collect()
}

#[test]
fn starts_at_origin_with_k_plus_one_points() {
    for k in [0, 1, 5] {
        let r = build_route(7, k);
        assert_eq!(r.points.len(), k + 1);
        assert_eq!(r.points[0], [0.0, 0.0]);
    }
}

#[test]
fn spacing_and_turn_bounds_over_many_seeds() {
    let p = RouteParams::shipped();
    for seed in 0..500 {
        let r = build_route(seed, 12);
        for w in r.points.windows(2) {
            let d = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
            assert!(
                (120.0 - 1e-9..=180.0 + 1e-9).contains(&d),
                "seed {seed}: {d}"
            );
        }
        for h in headings(&r).windows(2) {
            let turn = (h[1] - h[0]).sin().atan2((h[1] - h[0]).cos()).abs();
            assert!(
                turn <= p.max_turn_degrees.to_radians() + 1e-9,
                "seed {seed}"
            );
        }
    }
}

#[test]
fn deterministic_and_seed_dependent() {
    assert_eq!(build_route(42, 6), build_route(42, 6));
    assert_ne!(build_route(1, 6), build_route(2, 6));
    // A longer route extends a shorter one.
    assert_eq!(build_route(9, 3).points[..], build_route(9, 6).points[..4]);
}

#[test]
fn shipped_defaults() {
    let p = RouteParams::shipped();
    assert_eq!(
        (p.spacing, p.spacing_spread, p.max_turn_degrees),
        (150.0, 30.0, 30.0)
    );
}
