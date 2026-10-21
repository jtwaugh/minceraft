use super::*;
use crate::{build_route, RouteParams};

fn route(points: &[[f64; 2]]) -> Route {
    Route {
        points: points.to_vec(),
    }
}

#[test]
fn margin_zero_straight_segment() {
    let c = Corridor::new(&route(&[[10.0, 10.0], [400.0, 10.0]]), 0);
    let want: Vec<_> = (0..=3).map(|rx| RegionCoord::new(rx, 0)).collect();
    assert_eq!(c.regions(), want);
}

#[test]
fn margin_expands_chebyshev() {
    let c = Corridor::new(&route(&[[10.0, 10.0], [20.0, 20.0]]), 2);
    assert_eq!(c.regions().len(), 25);
    assert!(c.contains(RegionCoord::new(2, -2)));
    assert!(!c.contains(RegionCoord::new(3, 0)));
    assert!(!c.contains(RegionCoord::new(0, -3)));
}

#[test]
fn diagonal_segment_skips_far_corners() {
    // From region (0,0) to (3,3) along the diagonal: the off-diagonal corner
    // regions are not passed through.
    let c = Corridor::new(&route(&[[1.0, 1.0], [383.0, 383.0]]), 0);
    assert!(c.contains(RegionCoord::new(1, 1)));
    assert!(c.contains(RegionCoord::new(2, 2)));
    assert!(!c.contains(RegionCoord::new(3, 0)));
    assert!(!c.contains(RegionCoord::new(0, 3)));
}

#[test]
fn shallow_diagonal_passes_through_every_crossed_region() {
    let c = Corridor::new(&route(&[[0.5, 0.5], [300.0, 200.0]]), 0);
    for (rx, rz) in [(0, 0), (1, 0), (1, 1), (2, 1)] {
        assert!(c.contains(RegionCoord::new(rx, rz)), "{rx},{rz}");
    }
    assert!(!c.contains(RegionCoord::new(0, 1)));
    assert!(!c.contains(RegionCoord::new(2, 0)));
}

#[test]
fn crosses_negative_coordinates() {
    let c = Corridor::new(&route(&[[-200.0, -10.0], [200.0, 10.0]]), 0);
    for (rx, rz) in [(-2, -1), (-1, -1), (-1, 0), (0, 0), (1, 0)] {
        assert!(c.contains(RegionCoord::new(rx, rz)), "{rx},{rz}");
    }
    assert!(!c.contains(RegionCoord::new(-3, -1)));
    assert!(!c.contains(RegionCoord::new(2, 0)));
}

#[test]
fn single_point_route() {
    let c = Corridor::new(&route(&[[-1.0, 0.0]]), 0);
    assert_eq!(c.regions(), vec![RegionCoord::new(-1, 0)]);
    assert!(Corridor::new(&route(&[]), 2).regions().is_empty());
}

#[test]
fn regions_sorted_and_independent_of_route_direction() {
    let pts = [[-300.0, 50.0], [20.0, -250.0], [260.0, 140.0]];
    let mut rev = pts;
    rev.reverse();
    let a = Corridor::new(&route(&pts), 1);
    let b = Corridor::new(&route(&rev), 1);
    assert_eq!(a, b);
    let r = a.regions();
    assert!(r.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn built_route_points_are_covered() {
    let margin = RouteParams::shipped().corridor_margin;
    let r = build_route(42, 8);
    let c = Corridor::new(&r, margin);
    for p in &r.points {
        assert!(c.contains(RegionCoord::containing(
            p[0].floor() as i32,
            p[1].floor() as i32
        )));
    }
}
