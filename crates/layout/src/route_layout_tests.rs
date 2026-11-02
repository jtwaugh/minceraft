use super::*;
use crate::builder::V0_SPAWN_WATER_RADIUS;
use crate::{build_route, nearest_on_segment};

fn dist_to_route(route: &Route, p: (f64, f64)) -> f64 {
    route
        .points
        .windows(2)
        .map(|w| {
            nearest_on_segment(p, (w[0][0], w[0][1]), (w[1][0], w[1][1]))
                .0
                .sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

fn build(seed: u64, k: usize) -> (Route, Layout, Vec<Level>) {
    let route = build_route(seed, k);
    let (layout, levels) = build_route_layout(&WorldConfig { seed }, &route);
    (route, layout, levels)
}

#[test]
fn one_channel_per_segment_with_data_file_parameters() {
    let p = RouteParams::shipped();
    let (route, layout, levels) = build(9, 4);
    assert_eq!(levels.len(), 5);
    let channels: Vec<_> = layout
        .density
        .iter()
        .filter_map(|t| match t {
            DensityTerm::Channel {
                points,
                half_width,
                falloff,
                depth_below_sea,
                clear_above,
            } => Some((
                points,
                *half_width,
                *falloff,
                *depth_below_sea,
                *clear_above,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(channels.len(), 4);
    for (i, (pts, hw, fall, depth, clear)) in channels.into_iter().enumerate() {
        let (a, b) = (route.points[i], route.points[i + 1]);
        assert_eq!(pts, &vec![(a[0], a[1]), (b[0], b[1])]);
        assert_eq!(
            (hw, fall, depth, clear),
            (
                p.channel_half_width,
                p.channel_falloff,
                p.channel_depth_below_sea,
                p.channel_clear_above
            )
        );
    }
    assert_eq!(
        (p.channel_half_width, p.channel_falloff),
        (6.0, 6.0),
        "shipped defaults"
    );
    assert_eq!((p.channel_depth_below_sea, p.channel_clear_above), (4.0, 4));
}

#[test]
fn levels_follow_the_route_and_level_terms_are_reused() {
    let (route, layout, levels) = build(3, 3);
    for (i, l) in levels.iter().enumerate() {
        assert_eq!(l.week, i as u32 + 1);
        assert_eq!(l.anchorage[0], route.points[i][0].round() as i32);
        assert_eq!(l.spawn_water_radius, V0_SPAWN_WATER_RADIUS);
    }
    let base = build_layout(&WorldConfig { seed: 3 }, &levels);
    assert_eq!(layout.density[..base.density.len()], base.density[..]);
    assert_eq!(layout.exclusions, base.exclusions);
    assert_eq!(layout.structures, base.structures);
    assert_eq!(layout.structures.len(), 4);
}

#[test]
fn islands_sit_left_of_the_heading_and_clear_of_the_channel() {
    let p = RouteParams::shipped();
    let min = p.channel_half_width + p.channel_falloff;
    for seed in 0..200 {
        let (route, _, levels) = build(seed, 6);
        for (i, l) in levels.iter().enumerate() {
            let c = (l.island_center[0] as f64, l.island_center[1] as f64);
            let d = dist_to_route(&route, c);
            assert!(d > min, "seed {seed} level {i}: {d}");
            // Left: (dz, -dx) of the outgoing (or, last, incoming) segment.
            let (a, b) = if i + 1 < route.points.len() {
                (route.points[i], route.points[i + 1])
            } else {
                (route.points[i - 1], route.points[i])
            };
            let side = (b[0] - a[0]) * (c.1 - route.points[i][1])
                - (b[1] - a[1]) * (c.0 - route.points[i][0]);
            assert!(side < 0.0, "seed {seed} level {i} is on the right");
        }
    }
}

#[test]
fn pure_function_of_seed_and_route() {
    assert_eq!(build(5, 4).1, build(5, 4).1);
    assert_eq!(build(5, 4).2, build(5, 4).2);
    assert_ne!(build(5, 4).2, build(6, 4).2);
}

#[test]
fn single_point_route_has_no_channel() {
    let (_, layout, levels) = build(1, 0);
    assert_eq!(levels.len(), 1);
    assert!(layout
        .density
        .iter()
        .all(|t| !matches!(t, DensityTerm::Channel { .. })));
}

#[test]
fn overlapping_regions_agree_across_a_channel_and_an_anchorage() {
    let (route, layout, _) = build(11, 2);
    let mid = route.points[1];
    let (mx, mz) = (mid[0].round() as i32, mid[1].round() as i32);
    // Two windows around the second anchorage (where a channel ends and the
    // next begins), offset against each other.
    for decorate in [false, true] {
        let big = layout.generate_region(mx - 40, mz - 40, 80, 80, decorate);
        let small = layout.generate_region(mx - 10, mz - 25, 45, 60, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let i = small.index(x, 0, z);
                let j = big.index(
                    (small.x0 + x as i32 - big.x0) as u32,
                    0,
                    (small.z0 + z as i32 - big.z0) as u32,
                );
                assert_eq!(
                    small.blocks[i..i + terrain::HEIGHT],
                    big.blocks[j..j + terrain::HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}

fn coast_term(layout: &Layout) -> Option<&DensityTerm> {
    layout
        .density
        .iter()
        .find(|t| matches!(t, DensityTerm::Coast { .. }))
}

#[test]
fn one_coast_over_the_whole_route() {
    let p = RouteParams::shipped();
    let (route, layout, _) = build(9, 4);
    let coasts: Vec<_> = layout
        .density
        .iter()
        .filter(|t| matches!(t, DensityTerm::Coast { .. }))
        .collect();
    assert_eq!(coasts.len(), 1);
    let DensityTerm::Coast {
        points,
        seed,
        near,
        ramp,
        height,
        width,
        fade,
        wobble_length,
    } = coasts[0]
    else {
        unreachable!()
    };
    let expect: Vec<_> = route.points.iter().map(|p| (p[0], p[1])).collect();
    assert_eq!(points, &expect);
    assert_eq!(*seed, 9);
    assert_eq!(*near, (p.coast_near_min, p.coast_near_max));
    assert_eq!(*ramp, (p.coast_ramp_min, p.coast_ramp_max));
    assert_eq!(*height, (p.coast_height_min, p.coast_height_max));
    assert_eq!(
        (*width, *fade, *wobble_length),
        (p.coast_width, p.coast_fade, p.coast_wobble_length)
    );
}

#[test]
fn no_coast_for_a_single_point_route() {
    assert!(coast_term(&build(1, 0).1).is_none());
}

#[test]
fn land_on_the_right_and_not_the_left() {
    let p = RouteParams::shipped();
    let off = (p.coast_near_max + p.coast_ramp_max + 1.0) as i32;
    for seed in [1, 2, 3] {
        let (route, layout, _) = build(seed, 2);
        let (a, b) = (route.points[0], route.points[1]);
        let (mx, mz) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
        let (cx, cz) = (mx.round() as i32, mz.round() as i32);
        let reg = layout.generate_region(cx - 100, cz - 100, 200, 200, false);
        let top = |x: i32, z: i32| {
            let (ix, iz) = ((x - reg.x0) as u32, (z - reg.z0) as u32);
            let i = reg.index(ix, 0, iz);
            (0..terrain::HEIGHT)
                .rev()
                .find(|&y| reg.blocks[i + y] != 0)
                .map_or(0, |y| y as i32 + 1)
        };
        let side = |s: f64| {
            let (x, z) = (mx - s * d[1] * off as f64, mz + s * d[0] * off as f64);
            top(x.round() as i32, z.round() as i32)
        };
        // Right of the heading is (-dz, dx).
        assert!(side(1.0) > terrain::SEA_LEVEL, "seed {seed} right");
        assert!(side(-1.0) <= terrain::SEA_LEVEL, "seed {seed} left");
    }
}

#[test]
fn overlapping_regions_agree_across_the_coast() {
    let (route, layout, _) = build(11, 2);
    let a = route.points[0];
    let (ax, az) = (a[0].round() as i32, a[1].round() as i32);
    for decorate in [false, true] {
        let big = layout.generate_region(ax - 70, az - 70, 140, 140, decorate);
        let small = layout.generate_region(ax - 30, az - 15, 50, 70, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let i = small.index(x, 0, z);
                let j = big.index(
                    (small.x0 + x as i32 - big.x0) as u32,
                    0,
                    (small.z0 + z as i32 - big.z0) as u32,
                );
                assert_eq!(
                    small.blocks[i..i + terrain::HEIGHT],
                    big.blocks[j..j + terrain::HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}
