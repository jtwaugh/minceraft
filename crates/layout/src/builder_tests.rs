use super::*;
use terrain::{column_summary, Block, HEIGHT, SEA_LEVEL};

fn built(seed: u64) -> (Level, Layout) {
    let level = default_v0_level(seed);
    let layout = build_layout(&WorldConfig { seed }, std::slice::from_ref(&level));
    (level, layout)
}

#[test]
fn same_inputs_equal_layout() {
    for seed in [1u64, 99, 8812734419] {
        assert_eq!(built(seed), built(seed));
    }
}

#[test]
fn placement_is_a_pure_function_of_the_seed() {
    let mut centres = std::collections::BTreeSet::new();
    for seed in 1..=100u64 {
        let l = default_v0_level(seed);
        assert_eq!(l, default_v0_level(seed));
        assert_eq!(l.anchorage, [0, 0]);
        let [x, z] = l.island_center;
        let d = (x as f64).hypot(z as f64);
        // Rounding to whole blocks can move the distance by up to 0.71.
        assert!((23.2..=32.8).contains(&d), "seed {seed}: distance {d}");
        centres.insert((x, z));
    }
    assert!(centres.len() > 90, "directions should vary with the seed");
}

#[test]
fn emits_terms_exclusion_and_raft_for_each_level() {
    let (level, l) = built(5);
    assert_eq!(l.density.len(), 2);
    assert_eq!(l.exclusions.len(), 1);
    assert_eq!(
        l.structures,
        vec![StructurePlacement {
            structure: "raft_start".into(),
            origin: [0, SEA_LEVEL, 0],
            rotation: 0,
        }]
    );
    let Exclusion::Circle { center, radius } = &l.exclusions[0] else {
        panic!("expected a circle");
    };
    assert_eq!(*center, (0.0, 0.0));
    // Farthest raft cell is a corner at (2, 2) from the origin, plus 2.
    assert!(*radius >= 8f64.sqrt() + 2.0);
    assert_eq!(level.spawn_water_radius, 8.0);
}

#[test]
fn island_centre_is_land_for_95_percent_of_100_seeds() {
    let mut tops = Vec::new();
    let mut land = 0;
    for seed in 1..=100u64 {
        let (level, l) = built(seed);
        let [x, z] = level.island_center;
        let r = l.generate_region(x, z, 1, 1, false);
        let (top, ty, _, _) = column_summary(&r.blocks[..HEIGHT]);
        if top != Block::Water as u8 && top != Block::Ice as u8 && ty as i32 >= SEA_LEVEL {
            land += 1;
        }
        tops.push(ty as i32);
    }
    tops.sort_unstable();
    assert!(land >= 95, "only {land}/100 island centres are land");
    let median = tops[50];
    assert!(
        (median - (SEA_LEVEL + 6)).abs() <= 6,
        "median centre top {median}"
    );
}

#[test]
fn anchorage_is_deep_water_for_most_seeds() {
    let mut deep = 0;
    for seed in 1..=100u64 {
        let (_, l) = built(seed);
        let r = l.generate_region(-2, -2, 5, 5, false);
        let ok = (0..5).all(|z| {
            (0..5).all(|x| {
                let i = r.index(x, 0, z);
                let mut col = r.blocks[i..i + HEIGHT].to_vec();
                // Drop the stamped deck and what stands on it.
                col[SEA_LEVEL as usize..].fill(Block::Air as u8);
                let (top, ty, floor, _) = column_summary(&col);
                top == Block::Water as u8
                    && ty as i32 == SEA_LEVEL - 1
                    && SEA_LEVEL - 1 - floor as i32 >= 3
            })
        });
        deep += ok as u32;
    }
    assert!(deep >= 75, "only {deep}/100 anchorages are 3 deep");
}

#[test]
fn raft_footprint_plus_margin_is_undecorated() {
    let mut decorated_elsewhere = false;
    for seed in [3u64, 17, 12345, 777, 2024] {
        let (_, l) = built(seed);
        let (x0, z0, n) = (-40, -40, 80u32);
        let plain = l.generate_region(x0, z0, n, n, false);
        let deco = l.generate_region(x0, z0, n, n, true);
        let Exclusion::Circle { radius, .. } = &l.exclusions[0] else {
            unreachable!()
        };
        for z in 0..n {
            for x in 0..n {
                let (wx, wz) = ((x0 + x as i32) as f64, (z0 + z as i32) as f64);
                let i = plain.index(x, 0, z);
                let same = plain.blocks[i..i + HEIGHT] == deco.blocks[i..i + HEIGHT];
                if wx.hypot(wz) <= *radius {
                    assert!(same, "seed {seed}: decoration at ({wx}, {wz})");
                } else if !same {
                    decorated_elsewhere = true;
                }
            }
        }
    }
    assert!(
        decorated_elsewhere,
        "decoration should occur outside the circle"
    );
}

#[test]
fn overlapping_regions_agree_on_a_built_layout() {
    let (level, l) = built(4242);
    let [ix, iz] = level.island_center;
    for decorate in [false, true] {
        let big = l.generate_region(-60, -60, 120, 120, decorate);
        let small = l.generate_region(ix.min(0) - 5, iz.min(0) - 3, 50, 45, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let i = small.index(x, 0, z);
                let j = big.index(
                    (small.x0 + x as i32 + 60) as u32,
                    0,
                    (small.z0 + z as i32 + 60) as u32,
                );
                assert_eq!(
                    small.blocks[i..i + HEIGHT],
                    big.blocks[j..j + HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}
