use super::*;
use noise::{OctavePerlin, OctaveSimplex, Perlin, Rng, Simplex};

#[test]
fn perlin_single_octave_range() {
    let mut rng = Rng::new(42);
    let p = Perlin::new(&mut rng);
    let mut r = Rng::new(7);
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for _ in 0..200_000 {
        let v = p.noise(
            (r.next_f64() - 0.5) * 1000.0,
            (r.next_f64() - 0.5) * 1000.0,
            (r.next_f64() - 0.5) * 1000.0,
        );
        lo = lo.min(v);
        hi = hi.max(v);
    }
    assert!(lo >= -1.1 && hi <= 1.1, "perlin range {lo}..{hi}");
    assert!(
        lo < -0.5 && hi > 0.5,
        "perlin range suspiciously small {lo}..{hi}"
    );
}

#[test]
fn simplex_range() {
    let mut rng = Rng::new(3);
    let s = Simplex::new(&mut rng);
    let mut r = Rng::new(9);
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for _ in 0..200_000 {
        let v = s.noise((r.next_f64() - 0.5) * 500.0, (r.next_f64() - 0.5) * 500.0);
        lo = lo.min(v);
        hi = hi.max(v);
    }
    assert!(lo >= -1.01 && hi <= 1.01, "simplex range {lo}..{hi}");
    assert!(lo < -0.5 && hi > 0.5);
}

#[test]
fn octave_sums_bounded() {
    for &n in &[4usize, 8, 16] {
        let mut rng = Rng::new(n as u64);
        let o = OctavePerlin::new(&mut rng, n);
        let bound = ((1u64 << n) - 1) as f64 * 1.1;
        let mut r = Rng::new(11);
        for _ in 0..5_000 {
            let v = o.sample(r.next_f64() * 1e5, r.next_f64() * 1e3, r.next_f64() * 1e5);
            assert!(v.abs() <= bound, "octave {n}: {v}");
        }
    }
    let mut rng = Rng::new(5);
    let os = OctaveSimplex::new(&mut rng, 4);
    for i in 0..5_000 {
        let v = os.sample(i as f64 * 13.0, i as f64 * -7.0, 0.025, 0.025, 0.25, 0.5);
        assert!(v.abs() <= 0.55 * 15.0 * 1.01);
    }
}

#[test]
fn climate_in_unit_range() {
    let g = Generator::new(1234);
    for i in -50..50 {
        let (t, h) = g.climate(i * 97, i * -61);
        assert!((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&h));
    }
}

#[test]
fn deterministic_and_seed_sensitive() {
    let a = generate_region(99, -20, 37, 40, 30);
    let b = generate_region(99, -20, 37, 40, 30);
    assert_eq!(a.blocks, b.blocks);
    let c = generate_region(100, -20, 37, 40, 30);
    assert_ne!(a.blocks, c.blocks);
}

#[test]
fn overlapping_regions_agree() {
    let seed = 0xDEAD_BEEF;
    let big = generate_region(seed, -37, -13, 70, 55);
    // Differently aligned (non multiple-of-4 origin) sub-region.
    let (ox, oz) = (-30, 3);
    let small = generate_region(seed, ox, oz, 29, 21);
    for z in 0..small.depth {
        for x in 0..small.width {
            let bx = (ox + x as i32 - -37) as u32;
            let bz = (oz + z as i32 - -13) as u32;
            for y in 0..128 {
                assert_eq!(
                    small.get(x, y, z),
                    big.get(bx, y, bz),
                    "mismatch at world ({}, {}, {})",
                    ox + x as i32,
                    y,
                    oz + z as i32
                );
            }
        }
    }
    // Heightmap path agrees with full generation.
    let hm = generate_heightmap(seed, ox, oz, 29, 21);
    for z in 0..21u32 {
        for x in 0..29u32 {
            let i = small.index(x, 0, z);
            let s = column_summary(&small.blocks[i..i + 128]);
            let j = ((z * 29 + x) * 4) as usize;
            assert_eq!(
                (s.0, s.1, s.2, s.3),
                (hm[j], hm[j + 1], hm[j + 2], hm[j + 3])
            );
        }
    }
}

#[test]
fn sea_level_fill() {
    for &seed in &[1u64, 2, 3, 12345] {
        let r = generate_region(seed, -64, 200, 96, 96);
        let mut water = 0usize;
        for z in 0..r.depth {
            for x in 0..r.width {
                for y in 0..128 {
                    let b = r.get(x, y, z);
                    if y >= 64 {
                        assert_ne!(b, Block::Water, "water above sea level at y={y}");
                    } else {
                        assert_ne!(b, Block::Air, "air below sea level at y={y}");
                    }
                    if b == Block::Water {
                        water += 1;
                    }
                }
            }
        }
        let _ = water;
    }
}

#[test]
fn bias_extension_point() {
    let lift = |_x: i32, y: i32, _z: i32| if y < 100 { 1000.0 } else { 0.0 };
    let r = generate_region_with_bias(5, 0, 0, 8, 8, &[&lift]);
    for z in 0..8 {
        for x in 0..8 {
            for y in 0..90 {
                let b = r.get(x, y, z);
                assert!(b != Block::Air && b != Block::Water);
            }
        }
    }
}

#[test]
fn gradient_table_matches_spec() {
    for h in 0..16u8 {
        let (x, y, z) = (0.3, -0.7, 1.9);
        assert_eq!(
            noise::grad_reference(h, x, y, z),
            noise::grad_table(h, x, y, z),
            "h={h}"
        );
    }
}

#[test]
fn coarse_heightmap_matches_full_terrain() {
    // Samples sit on lattice columns, where the full generator's trilinear
    // interpolation reduces to the same Y interpolation. The surface pass
    // can turn a few top blocks into water at sea level, so allow a sliver
    // of mismatches there.
    let (seed, x0, z0, step, n) = (12345u64, -203, 77, 8u32, 40u32);
    let coarse = generate_coarse_heightmap(seed, x0, z0, step, n, n);
    let (ax, az) = (x0 & !3, z0 & !3);
    let span = (n - 1) * step + 1;
    let r = generate_region(seed, ax, az, span, span);
    let (mut same, mut total) = (0, 0);
    for k in 0..n {
        for i in 0..n {
            let (x, z) = (i * step, k * step);
            let solid = |y: u32| {
                let b = r.get(x, y, z);
                b != Block::Air && b != Block::Water && b != Block::Ice
            };
            let top = (0..128).rev().find(|&y| solid(y)).unwrap_or(0);
            // gaps: air runs above sea level with solid above and below
            let (mut gaps, mut in_air) = (0u8, false);
            for y in (0..=top).rev() {
                if solid(y) {
                    if in_air && y + 1 >= 64 {
                        gaps += 1;
                    }
                    in_air = false;
                } else {
                    in_air = true;
                }
            }
            let o = ((k * n + i) * 4) as usize;
            total += 1;
            if coarse[o] as u32 == top && coarse[o + 2] == gaps {
                same += 1;
            }
            assert_eq!(
                coarse[o + 1],
                biome_at(seed, ax + x as i32, az + z as i32) as u8
            );
        }
    }
    assert!(
        same * 100 >= total * 98,
        "coarse heights matched {same}/{total}"
    );
}

// ---- base-height bias (SPEC 3.3 step 6 extension point) ----

fn mean_top(hm: &[u8]) -> f64 {
    hm.chunks(4).map(|c| c[1] as f64).sum::<f64>() / (hm.len() / 4) as f64
}

#[test]
fn empty_base_height_is_identical() {
    for seed in [1u64, 12345, 987654321] {
        let b = Biases::default();
        assert_eq!(
            generate_region_with_biases(seed, -40, 25, 70, 50, &b, false).blocks,
            generate_region(seed, -40, 25, 70, 50).blocks
        );
        assert_eq!(
            generate_heightmap_with_biases(seed, -40, 25, 70, 50, &b, false),
            generate_heightmap(seed, -40, 25, 70, 50)
        );
        let none: [&dyn BaseHeightBias; 0] = [];
        let e = Biases {
            base_height: &none,
            ..Biases::default()
        };
        assert_eq!(
            generate_region_with_biases(seed, -40, 25, 70, 50, &e, true).blocks,
            generate_decorated(seed, -40, 25, 70, 50).blocks
        );
    }
}

#[test]
fn base_height_raises_and_lowers() {
    let seed = 12345;
    let base = mean_top(&generate_heightmap(seed, 0, 0, 96, 96));
    let up = |_: i32, _: i32, _: f64| 16.0;
    let down = |_: i32, _: i32, _: f64| -16.0;
    let ups: [&dyn BaseHeightBias; 1] = [&up];
    let downs: [&dyn BaseHeightBias; 1] = [&down];
    let run = |t: &[&dyn BaseHeightBias]| {
        mean_top(&generate_heightmap_with_biases(
            seed,
            0,
            0,
            96,
            96,
            &Biases {
                base_height: t,
                ..Biases::default()
            },
            false,
        ))
    };
    let (hi, lo) = (run(&ups), run(&downs));
    assert!(hi > base + 4.0, "up {hi} vs base {base}");
    assert!(lo < base - 4.0, "down {lo} vs base {base}");
}

#[test]
fn base_height_overlapping_regions_agree() {
    let seed = 777;
    let a = |x: i32, z: i32, _: f64| (x as f64 * 0.05).sin() * 10.0 + z as f64 * 0.1;
    let c = |x: i32, _z: i32, _: f64| if x > 20 { 6.0 } else { -3.0 };
    let terms: [&dyn BaseHeightBias; 2] = [&a, &c];
    let b = Biases {
        base_height: &terms,
        ..Biases::default()
    };
    for decorate in [false, true] {
        let big = generate_region_with_biases(seed, -20, -20, 90, 90, &b, decorate);
        let small = generate_region_with_biases(seed, 7, -3, 40, 50, &b, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let bx = (small.x0 + x as i32 + 20) as u32;
                let bz = (small.z0 + z as i32 + 20) as u32;
                let i = small.index(x, 0, z);
                let j = big.index(bx, 0, bz);
                assert_eq!(
                    small.blocks[i..i + HEIGHT],
                    big.blocks[j..j + HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}

#[test]
fn raft_blocks_have_distinct_ids_and_old_ids_unchanged() {
    // Ids 0..=24 are the original palette; they must never move.
    let old = [
        (Block::Air, 0),
        (Block::Stone, 1),
        (Block::Dirt, 2),
        (Block::Grass, 3),
        (Block::Sand, 4),
        (Block::Gravel, 5),
        (Block::Water, 6),
        (Block::Ice, 7),
        (Block::OakLog, 8),
        (Block::BirchLog, 9),
        (Block::SpruceLog, 10),
        (Block::OakLeaves, 11),
        (Block::BirchLeaves, 12),
        (Block::SpruceLeaves, 13),
        (Block::TallGrass, 14),
        (Block::Fern, 15),
        (Block::DeadBush, 16),
        (Block::Dandelion, 17),
        (Block::Rose, 18),
        (Block::SugarCane, 19),
        (Block::Cactus, 20),
        (Block::BrownMushroom, 21),
        (Block::RedMushroom, 22),
        (Block::Pumpkin, 23),
        (Block::SnowLayer, 24),
    ];
    for (b, id) in old {
        assert_eq!(b as u8, id);
    }
    let new = [
        (Block::RaftPlanks, 25),
        (Block::ButcherBlock, 26),
        (Block::Griddle, 27),
        (Block::Crate, 28),
    ];
    for (i, (b, id)) in new.iter().enumerate() {
        assert_eq!(*b as u8, *id);
        for (c, _) in &new[i + 1..] {
            assert_ne!(*b as u8, *c as u8);
        }
        let info = &BLOCK_INFO[*id as usize];
        assert!(info.opaque_cube && info.opacity == 255);
    }
}
