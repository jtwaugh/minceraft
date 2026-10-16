use super::*;

fn level_layout(seed: u64) -> Layout {
    build_layout(&WorldConfig { seed }, &[default_v0_level(seed)])
}

fn warm() -> ClimateBias {
    ClimateBias::Radial {
        center: (0.0, 0.0),
        r_inner: 1.0e6,
        r_outer: 2.0e6,
        d_temp: 1.0,
        d_humid: 0.0,
    }
}

fn assert_exactly_one(l: &Layout, x0: i32, z0: i32) -> usize {
    let m = l.habitat_map(x0, z0, 96, 96);
    let rules = Habitats::shipped().rules.len();
    let mut water = 0;
    for i in 0..m.habitat.len() {
        if m.derived.water_depth[i] == 0 {
            assert_eq!(m.habitat[i], NO_HABITAT);
        } else {
            water += 1;
            assert!((m.habitat[i] as usize) < rules);
        }
    }
    water
}

fn channel_layout(seed: u64) -> Layout {
    let mut l = level_layout(seed);
    l.density.push(DensityTerm::Channel {
        points: vec![(-60.0, 40.0), (0.0, 50.0), (60.0, 40.0)],
        half_width: 6.0,
        falloff: 8.0,
        depth_below_sea: 12.0,
        clear_above: 8,
    });
    l
}

#[test]
fn water_has_one_habitat_land_none() {
    let mut water = 0;
    for seed in [1, 2, 12345, 99] {
        water += assert_exactly_one(
            &Layout {
                seed,
                ..Layout::default()
            },
            -40,
            -40,
        );
        water += assert_exactly_one(&level_layout(seed), -40, -40);
    }
    water += assert_exactly_one(&channel_layout(3), -40, -40);
    let mut biased = channel_layout(7);
    biased.climate.push(warm());
    water += assert_exactly_one(&biased, -40, -40);
    assert!(water > 0);
}

#[test]
fn bias_changes_temperature_and_yields_warm_shallows() {
    let id = Habitats::shipped().id_of("warm_shallows").unwrap().0 as u8;
    let sand = terrain::Block::Sand as u8;
    let mut flipped = false;
    for seed in 0..40u64 {
        let plain = Layout {
            seed,
            ..Layout::default()
        };
        let hot = Layout {
            climate: vec![warm()],
            ..plain.clone()
        };
        let (p, h) = (
            plain.habitat_map(-64, -64, 128, 128),
            hot.habitat_map(-64, -64, 128, 128),
        );
        assert!(h
            .temperature
            .iter()
            .zip(&p.temperature)
            .all(|(a, b)| a >= b));
        let m = &h.derived;
        for i in 0..h.habitat.len() {
            let shallow_sand = m.water_depth[i] != 0
                && m.water_depth[i] <= 6
                && m.bottom_block[i] == sand
                && !m.frozen[i];
            if shallow_sand {
                assert_eq!(h.habitat[i], id);
                flipped |= p.habitat[i] != id;
            }
        }
        if flipped {
            return;
        }
    }
    panic!("no column where the warm bias creates warm_shallows");
}

#[test]
fn overlapping_regions_agree() {
    let mut l = level_layout(5);
    l.climate.push(warm());
    let a = l.habitat_map(-30, -20, 90, 70);
    let b = l.habitat_map(10, 0, 40, 100);
    for z in 0..70i32 {
        for x in 0..90i32 {
            let (wx, wz) = (x - 30, z - 20);
            let (bx, bz) = (wx - 10, wz);
            if (0..40).contains(&bx) && (0..100).contains(&bz) {
                let ia = (z * 90 + x) as usize;
                let ib = (bz * 40 + bx) as usize;
                assert_eq!(a.habitat[ia], b.habitat[ib], "at {wx},{wz}");
                assert_eq!(a.temperature[ia], b.temperature[ib]);
                assert_eq!(a.derived.shore_distance[ia], b.derived.shore_distance[ib]);
            }
        }
    }
}
