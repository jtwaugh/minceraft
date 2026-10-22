use super::*;
use std::collections::{HashSet, VecDeque};
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

const DEPTH: f64 = 4.0;
const CLEAR: i32 = 8;
const HALF: f64 = 6.0;
const FALL: f64 = 6.0;
/// Seabed error in blocks that 97% of columns inside `half_width` must meet.
const TOLERANCE: i32 = 2;
/// Hard bound on the seabed error (measured on seeds 1 to 12: 6).
const MAX_ERROR: i32 = 6;
const SEEDS: std::ops::RangeInclusive<u64> = 1..=12;

fn channel(points: Vec<(f64, f64)>) -> DensityTerm {
    DensityTerm::Channel {
        points,
        half_width: HALF,
        falloff: FALL,
        depth_below_sea: DEPTH,
        clear_above: CLEAR,
    }
}

/// An east-west channel along z = 0 from x = -60 to x = 60.
fn layout(seed: u64) -> Layout {
    Layout {
        seed,
        density: vec![channel(vec![(-60.0, 0.0), (60.0, 0.0)])],
        ..Layout::default()
    }
}

fn is_open(b: u8) -> bool {
    b == Block::Air as u8 || b == Block::Water as u8 || b == Block::Ice as u8
}

fn column(r: &Region, wx: i32, wz: i32) -> &[u8] {
    let i = r.index((wx - r.x0) as u32, 0, (wz - r.z0) as u32);
    &r.blocks[i..i + HEIGHT]
}

/// Highest block below sea level that is not air, water or ice.
fn seabed_top(col: &[u8]) -> i32 {
    (0..SEA_LEVEL as usize)
        .rev()
        .find(|&y| !is_open(col[y]))
        .unwrap() as i32
}

/// Whether the column has land (solid at or above sea level).
fn is_land(col: &[u8]) -> bool {
    (SEA_LEVEL as usize..HEIGHT).any(|y| !is_open(col[y]))
}

#[test]
fn seabed_depth_within_half_width() {
    let mut worst = 0;
    let mut hist = [0u32; 10];
    for seed in SEEDS {
        let r = layout(seed).generate_region(-70, -20, 140, 40, false);
        for wz in -(HALF as i32)..=(HALF as i32) {
            for wx in -50..=50 {
                // Depth of the seabed's top face below SEA_LEVEL.
                let depth = SEA_LEVEL - 1 - seabed_top(column(&r, wx, wz));
                let err = (depth - DEPTH as i32).abs();
                hist[err.min(9) as usize] += 1;
                worst = worst.max(err);
            }
        }
    }
    eprintln!("worst seabed error: {worst} blocks, histogram {hist:?}");
    // Natural air pockets under the carved floor can leave the seabed deeper
    // than the target in a few columns, so the bound is statistical.
    let total: u32 = hist.iter().sum();
    let near: u32 = hist[..=TOLERANCE as usize].iter().sum();
    assert!(
        near * 100 >= total * 97,
        "{near}/{total} within {TOLERANCE}"
    );
    assert!(worst <= MAX_ERROR, "worst seabed error {worst}");
}

#[test]
fn no_overhangs_inside_channel() {
    for seed in SEEDS {
        // Terrain only: tree leaves from neighbouring land may overhang.
        {
            let r = layout(seed).generate_region(-70, -20, 140, 40, false);
            for wz in -(HALF as i32)..=(HALF as i32) {
                for wx in -60..=60 {
                    let col = column(&r, wx, wz);
                    for y in (SEA_LEVEL - 1)..(SEA_LEVEL + CLEAR) {
                        assert!(
                            is_open(col[y as usize]),
                            "seed {seed} ({wx},{y},{wz}) = {}",
                            col[y as usize]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn terrain_unchanged_beyond_reach() {
    for seed in SEEDS {
        let a = layout(seed).generate_region(-70, -60, 140, 120, false);
        let b = Layout::new(seed).generate_region(-70, -60, 140, 120, false);
        // Terrain depends on the 4-block density lattice and its
        // interpolation: allow two extra lattice cells around the reach.
        let reach = (HALF + FALL) as i32 + 8;
        for wz in -60i32..60 {
            for wx in -70..70 {
                if wz.abs() > reach || wx < -60 - reach || wx > 60 + reach {
                    assert_eq!(
                        column(&a, wx, wz),
                        column(&b, wx, wz),
                        "seed {seed} ({wx},{wz})"
                    );
                }
            }
        }
    }
}

#[test]
fn water_path_connects_across_land() {
    let mut crossings = 0;
    for seed in SEEDS {
        let base = Layout::new(seed).generate_region(-70, -20, 140, 40, false);
        if (-60..=60).any(|x| is_land(column(&base, x, 0))) {
            crossings += 1;
        }
        let r = layout(seed).generate_region(-70, -20, 140, 40, false);
        // Flood fill over the top water layer from one end to the other.
        let water = |x: i32, z: i32| {
            let b = column(&r, x, z)[(SEA_LEVEL - 1) as usize];
            b == Block::Water as u8 || b == Block::Ice as u8
        };
        assert!(water(-60, 0), "seed {seed}: start is dry");
        let mut seen = HashSet::from([(-60, 0)]);
        let mut q = VecDeque::from([(-60, 0)]);
        while let Some((x, z)) = q.pop_front() {
            for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
                if (-70..70).contains(&nx)
                    && (-20..20).contains(&nz)
                    && water(nx, nz)
                    && seen.insert((nx, nz))
                {
                    q.push_back((nx, nz));
                }
            }
        }
        assert!(seen.contains(&(60, 0)), "seed {seed}: water path broken");
    }
    eprintln!("{crossings} of 12 seeds had dry land on the centre line");
    assert!(crossings > 0, "test never crosses land");
}

#[test]
fn overlapping_regions_agree_with_channel() {
    let l = Layout {
        seed: 777,
        density: vec![channel(vec![(-30.0, 10.0), (10.0, -5.0), (50.0, 20.0)])],
        ..Layout::default()
    };
    for decorate in [false, true] {
        let big = l.generate_region(-40, -30, 120, 90, decorate);
        let small = l.generate_region(-7, -9, 61, 47, decorate);
        let all = l.generate_region_all_terms(-7, -9, 61, 47, decorate);
        assert_eq!(small.blocks, all.blocks, "decorate={decorate}");
        for z in 0..small.depth {
            for x in 0..small.width {
                let (wx, wz) = (small.x0 + x as i32, small.z0 + z as i32);
                assert_eq!(
                    column(&small, wx, wz),
                    column(&big, wx, wz),
                    "decorate={decorate} ({wx},{wz})"
                );
            }
        }
    }
}

#[test]
fn far_channel_is_skipped_invisibly() {
    let l = Layout {
        seed: 5,
        density: vec![channel(vec![(5000.0, 5000.0), (5100.0, 5000.0)])],
        ..Layout::default()
    };
    let a = l.generate_region(0, 0, 50, 50, false);
    assert_eq!(
        a.blocks,
        Layout::new(5).generate_region(0, 0, 50, 50, false).blocks
    );
}

#[test]
fn channel_columns_stay_free_of_sea_ice() {
    let seed = 4242;
    let mut l = layout(seed);
    l.climate.push(ClimateBias::AlongPath {
        points: vec![(-60.0, 0.0), (60.0, 0.0)],
        d_temp_start: -1.0,
        d_temp_end: -1.0,
        d_humid_start: 0.0,
        d_humid_end: 0.0,
        half_width: 100.0,
        falloff: 10.0,
    });
    let ice = Block::Ice as u8;
    let big = l.generate_region(-50, -40, 100, 80, true);
    let mut ice_outside = 0;
    for wz in -40..40 {
        for wx in -50..50 {
            let has_ice = column(&big, wx, wz).contains(&ice);
            if wz.abs() < (HALF + FALL) as i32 {
                assert!(!has_ice, "ice in the channel at ({wx},{wz})");
            }
            ice_outside += has_ice as u32;
        }
    }
    assert!(ice_outside > 100, "cold sea not frozen: {ice_outside}");
    for decorate in [false, true] {
        let big = l.generate_region(-50, -40, 100, 80, decorate);
        let small = l.generate_region(-17, -9, 40, 30, decorate);
        for wz in -9..21 {
            for wx in -17..23 {
                assert_eq!(column(&small, wx, wz), column(&big, wx, wz));
            }
        }
    }
}
