use crate::validation::*;
use crate::{
    validate_route, DensityTerm, Layout, Level, Route, Thresholds, CHECK_ANCHORAGE_REACHABLE,
    CHECK_CHANNEL_CLEAR,
};

fn level(id: &str, anchorage: [i32; 2]) -> Level {
    Level {
        id: id.into(),
        week: 1,
        anchorage,
        island_center: [anchorage[0], anchorage[1] + 400],
        spawn_water_radius: 8.0,
    }
}

fn levels() -> Vec<Level> {
    vec![level("one", [0, 0]), level("two", [100, 0])]
}

fn route() -> Route {
    Route {
        points: vec![[0.0, 0.0], [100.0, 0.0]],
    }
}

fn radial(center: (f64, f64), r_inner: f64, delta: f64) -> DensityTerm {
    DensityTerm::RadialBaseHeight {
        center,
        r_inner,
        r_outer: r_inner + 10.0,
        delta,
    }
}

fn channel(z: f64) -> DensityTerm {
    DensityTerm::Channel {
        points: vec![(-20.0, z), (120.0, z)],
        half_width: 6.0,
        falloff: 6.0,
        depth_below_sea: 4.0,
        clear_above: 4,
    }
}

/// Open sea around the route, with a mountain across the middle of it when
/// `wall` is set.
fn layout(wall: bool, channel_z: Option<f64>) -> Layout {
    let mut density = vec![radial((50.0, 0.0), 300.0, -60.0)];
    if wall {
        // A ridge along x = 50, long enough to span the whole window.
        density.extend((-20..=20).map(|i| radial((50.0, i as f64 * 10.0), 12.0, 90.0)));
    }
    density.extend(channel_z.map(channel));
    Layout {
        seed: 3,
        density,
        ..Layout::default()
    }
}

fn check<'a>(r: &'a ValidationReport, name: &str) -> &'a CheckResult {
    r.check(name).unwrap_or_else(|| panic!("no {name}: {r:?}"))
}

#[test]
fn open_sea_with_clear_channel_passes() {
    let l = layout(false, Some(0.0));
    let r = validate_route(&l, &levels(), &route(), &Thresholds::shipped());
    assert!(check(&r, CHECK_ANCHORAGE_REACHABLE).passed, "{r:?}");
    assert!(check(&r, CHECK_CHANNEL_CLEAR).passed, "{r:?}");
}

#[test]
fn blocked_segment_fails_and_names_the_pair() {
    let l = layout(true, None);
    let r = validate_route(&l, &levels(), &route(), &Thresholds::shipped());
    let c = check(&r, CHECK_ANCHORAGE_REACHABLE);
    assert!(!c.passed);
    assert!(c.required.contains("one") && c.required.contains("two"));
}

#[test]
fn one_check_per_pair_and_level_checks_included() {
    let mut lv = levels();
    lv.push(level("three", [200, 0]));
    let route = Route {
        points: vec![[0.0, 0.0], [100.0, 0.0], [200.0, 0.0]],
    };
    let r = validate_route(
        &layout(false, Some(0.0)),
        &lv,
        &route,
        &Thresholds::shipped(),
    );
    let n = |name: &str| r.checks.iter().filter(|c| c.name == name).count();
    assert_eq!(n(CHECK_ANCHORAGE_REACHABLE), 2);
    assert_eq!(n(CHECK_ISLAND_AREA), 3);
    assert_eq!(n(CHECK_CHANNEL_CLEAR), 1);
}

#[test]
fn solid_block_in_clearance_band_fails_channel_clear() {
    // The channel is carved 60 blocks off the route, so the wall stands in
    // the route's channel cells.
    let l = layout(true, Some(60.0));
    let r = validate_route(&l, &levels(), &route(), &Thresholds::shipped());
    let c = check(&r, CHECK_CHANNEL_CLEAR);
    assert!(!c.passed && c.measured > 0.0, "{c}");
}

#[test]
fn route_without_channel_term_fails_channel_clear() {
    let l = layout(false, None);
    let r = validate_route(&l, &levels(), &route(), &Thresholds::shipped());
    assert!(!check(&r, CHECK_CHANNEL_CLEAR).passed);
}
