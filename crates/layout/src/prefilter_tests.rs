use crate::{
    build_route, build_route_layout, prefilter, prefilter_layout, prefilter_regions,
    validate_route, Corridor, DensityTerm, Layout, Level, PrefilterReject, Route, RouteParams,
    Thresholds, WorldConfig,
};

fn level(anchorage: [i32; 2], island_center: [i32; 2]) -> Level {
    Level {
        id: "w".into(),
        week: 1,
        anchorage,
        island_center,
        spawn_water_radius: 8.0,
    }
}

fn radial(center: [i32; 2], delta: f64) -> DensityTerm {
    DensityTerm::RadialBaseHeight {
        center: (center[0] as f64, center[1] as f64),
        r_inner: 12.0,
        r_outer: 20.0,
        delta,
    }
}

fn route() -> Route {
    Route {
        points: vec![[0.0, 0.0]],
    }
}

fn run(anchor_delta: f64, centre_delta: f64) -> Result<(), PrefilterReject> {
    let l = level([0, 0], [0, 28]);
    let layout = Layout {
        seed: 1,
        density: vec![
            radial(l.anchorage, anchor_delta),
            radial(l.island_center, centre_delta),
        ],
        ..Layout::default()
    };
    prefilter_layout(&layout, &[l], &route())
}

#[test]
fn land_in_the_anchorage_is_rejected() {
    assert_eq!(
        run(80.0, 80.0),
        Err(PrefilterReject::AnchorageSolid { level: 0 })
    );
}

#[test]
fn seabed_only_in_the_anchorage_is_accepted() {
    assert_eq!(run(-60.0, 80.0), Ok(()));
}

#[test]
fn drowned_island_centre_is_rejected() {
    assert_eq!(
        run(-60.0, -60.0),
        Err(PrefilterReject::IslandUnderwater { level: 0 })
    );
}

#[test]
fn only_corridor_regions_are_sampled() {
    let route = Route {
        points: vec![[0.0, 0.0], [700.0, 0.0], [1400.0, 300.0]],
    };
    let (_, levels) = build_route_layout(&WorldConfig { seed: 3 }, &route);
    let corridor = Corridor::new(&route, RouteParams::shipped().corridor_margin);
    let sampled = prefilter_regions(&levels, &corridor);
    assert!(sampled.len() >= 3);
    assert!(sampled.iter().all(|c| corridor.contains(*c)));
    assert!(sampled.len() < corridor.regions().len());
}

#[test]
fn rejected_seeds_never_pass_validation() {
    let t = Thresholds::shipped();
    let mut rejected = 0;
    for seed in 1..=50u64 {
        if prefilter(seed, 2).is_err() {
            rejected += 1;
            let route = build_route(seed, 2);
            let (layout, levels) = build_route_layout(&WorldConfig { seed }, &route);
            assert!(
                !validate_route(&layout, &levels, &route, &t).passed(),
                "seed {seed} was rejected but validates"
            );
        }
    }
    println!("prefilter rejected {rejected} of 50 seeds");
}
