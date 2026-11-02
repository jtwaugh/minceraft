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
                b != Block::Air && b != Block::Water && b != Block::Ice && b != Block::FloeIce
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

#[test]
fn is_water_is_water_and_waterlogged_blocks_only() {
    assert!(Block::Water.info().is_water());
    for b in LOGGED {
        let plant = b.info();
        assert!(plant.is_water() && plant.waterlogged && !plant.liquid);
        assert_eq!(plant.collision, Collision::None);
    }
    // Every other block (air, stone, ice, plants, ...) is not water.
    for b in Block::ALL {
        let water = b == Block::Water || LOGGED.contains(&b);
        assert_eq!(b.info().is_water(), water, "{}", b.info().name);
        assert_eq!(b.info().waterlogged, LOGGED.contains(&b));
    }
}

const LOGGED: [Block; 5] = [
    Block::TestWaterloggedPlant,
    Block::Kelp0,
    Block::Kelp1,
    Block::Oyster,
    Block::Scallop,
];

#[test]
fn test_waterlogged_plant_is_never_generated() {
    for (seed, x0, z0) in [(12345u64, -40, 17), (7, 300, -260), (99, 0, 0)] {
        let r = generate_decorated(seed, x0, z0, 48, 48);
        for b in LOGGED {
            assert!(!r.blocks.contains(&(b as u8)));
        }
    }
}

// Village and cooking-station blocks: game blocks the generator never places.
#[test]
fn game_blocks_have_dense_ids_and_are_never_generated() {
    let game = [
        (Block::Thatch, 69, Collision::Full, true),
        (Block::Barrel, 70, Collision::Full, true),
        (Block::PlankStep, 71, Collision::Full, false),
        (Block::DryingRack, 72, Collision::None, false),
        (Block::Net, 73, Collision::None, false),
        (Block::Counter, 74, Collision::Full, true),
        (Block::PlateRack, 75, Collision::Full, true),
        (Block::Chest, 83, Collision::Full, true),
    ];
    for (b, id, collision, opaque) in game {
        assert_eq!(b as u8, id);
        let info = b.info();
        assert_eq!(info.collision, collision, "{}", info.name);
        assert_eq!(info.opaque_cube, opaque, "{}", info.name);
        assert_eq!(info.opacity == 255, opaque, "{}", info.name);
        let fire = (info.fire_encouragement, info.flammability);
        assert_eq!(fire, (0, 0), "{}", info.name);
        for other in Block::ALL {
            if other != b {
                assert_ne!(other.info().name, info.name);
                assert_ne!(other.info().color, info.color, "{}", info.name);
            }
        }
    }
    for (seed, x0, z0) in [(12345u64, -40, 17), (7, 300, -260), (99, 0, 0)] {
        let r = generate_decorated(seed, x0, z0, 48, 48);
        for (b, ..) in game {
            assert!(!r.blocks.contains(&(b as u8)), "{}", b.info().name);
        }
    }
}

// ---- coarse density sampler (seed prefilter) ----

const BAND: (i32, i32) = (SEA_LEVEL - 8, SEA_LEVEL + 8);

fn coarse_at(seed: u64, x0: i32, z0: i32, n: u32, b: &Biases) -> (Vec<bool>, u32) {
    generate_coarse_density(seed, x0, z0, n, n, BAND, b)
}

#[test]
fn coarse_density_matches_full_terrain() {
    let (seed, n) = (12345u64, 24u32);
    let (x0, z0) = (-203, 77);
    let (ax, az) = (x0 & !3, z0 & !3);
    let (flags, ny) = coarse_at(seed, x0, z0, n, &Biases::default());
    assert_eq!(ny, 5); // y = 56, 60, 64, 68, 72
    let r = generate_region(seed, ax, az, (n - 1) * 4 + 1, (n - 1) * 4 + 1);
    let (mut bad, mut total) = (0, 0);
    for j in 0..ny {
        for k in 0..n {
            for i in 0..n {
                let b = r.get(i * 4, 56 + j * 4, k * 4);
                let solid = b != Block::Air && b != Block::Water && b != Block::Ice;
                total += 1;
                bad += (flags[((j * n + k) * n + i) as usize] != solid) as u32;
            }
        }
    }
    assert_eq!(bad, 0, "{bad}/{total} flags differ");
}

#[test]
fn coarse_density_is_origin_and_extent_independent() {
    let seed = 0xBEEF;
    let (big, ny) = coarse_at(seed, -64, -32, 30, &Biases::default());
    let (small, sny) = generate_coarse_density(seed, -41, -19, 7, 9, BAND, &Biases::default());
    assert_eq!(ny, sny);
    // Small origin rounds down to (-44, -20); big starts at (-64, -32).
    let (dx, dz) = (5usize, 3usize);
    for j in 0..ny as usize {
        for k in 0..9 {
            for i in 0..7 {
                assert_eq!(
                    small[(j * 9 + k) * 7 + i],
                    big[(j * 30 + k + dz) * 30 + i + dx]
                );
            }
        }
    }
}

#[test]
fn coarse_density_honours_biases() {
    let seed = 7;
    let plain = coarse_at(seed, 0, 0, 20, &Biases::default()).0;
    let count = |v: &[bool]| v.iter().filter(|&&s| s).count();
    let raise = |_: i32, _: i32, _: f64| 40.0;
    let up: [&dyn BaseHeightBias; 1] = [&raise];
    let higher = coarse_at(
        seed,
        0,
        0,
        20,
        &Biases {
            base_height: &up,
            ..Biases::default()
        },
    )
    .0;
    assert!(count(&higher) > count(&plain));
    let lift = |_: i32, y: i32, _: i32| if y < 70 { 1000.0 } else { 0.0 };
    let d: [&dyn DensityBias; 1] = [&lift];
    let (solid, ny) = coarse_at(
        seed,
        0,
        0,
        20,
        &Biases {
            density: &d,
            ..Biases::default()
        },
    );
    for (idx, &s) in solid.iter().enumerate() {
        let y = 56 + 4 * (idx / (20 * 20)) as i32;
        assert_eq!(s, y < 70 || plain[idx], "y={y}");
    }
    let cut = |_: i32, _: i32, _: i32, _: f64| -1.0;
    let c: [&dyn DensityClamp; 1] = [&cut];
    let none = coarse_at(
        seed,
        0,
        0,
        20,
        &Biases {
            clamp: &c,
            ..Biases::default()
        },
    )
    .0;
    assert!(none.iter().all(|&s| !s) && ny == 5);
}

// ---- open water (sea-ice mask) ----

fn cold(_x: i32, _z: i32) -> f64 {
    -1.0
}

fn in_mask(x: i32, z: i32) -> bool {
    (10..50).contains(&x) && (-20..30).contains(&z)
}

#[test]
fn open_water_replaces_ice_only_on_masked_columns() {
    let t: [&dyn TemperatureBias; 1] = [&cold];
    let open: &dyn OpenWater = &in_mask;
    let plain = Biases {
        temperature: &t,
        ..Biases::default()
    };
    let masked = Biases {
        open_water: Some(open),
        ..plain
    };
    let ice = Block::Ice as u8;
    let water = Block::Water as u8;
    for seed in [1u64, 12345, 987654321] {
        for decorate in [false, true] {
            let a = generate_region_with_biases(seed, -30, -40, 120, 90, &plain, decorate);
            let b = generate_region_with_biases(seed, -30, -40, 120, 90, &masked, decorate);
            let mut ice_seen = 0;
            for z in -40..50 {
                for x in -30..90 {
                    let i = a.index((x + 30) as u32, 0, (z + 40) as u32);
                    let (ca, cb) = (&a.blocks[i..i + HEIGHT], &b.blocks[i..i + HEIGHT]);
                    if in_mask(x, z) {
                        assert!(!cb.contains(&ice), "seed {seed} ({x},{z})");
                        if !decorate {
                            let swapped: Vec<u8> = ca
                                .iter()
                                .map(|&v| if v == ice { water } else { v })
                                .collect();
                            assert_eq!(swapped, cb, "seed {seed} ({x},{z})");
                        }
                        ice_seen += ca.contains(&ice) as u32;
                    } else {
                        assert_eq!(ca, cb, "seed {seed} ({x},{z})");
                    }
                }
            }
            if !decorate {
                eprintln!("seed {seed}: {ice_seen} masked columns had ice");
            }
        }
    }
}

#[test]
fn no_open_water_mask_is_identical() {
    let t: [&dyn TemperatureBias; 1] = [&cold];
    let b = Biases {
        temperature: &t,
        ..Biases::default()
    };
    let none = |_: i32, _: i32| false;
    let m = Biases {
        open_water: Some(&none),
        ..b
    };
    assert_eq!(
        generate_region_with_biases(7, -20, -20, 60, 60, &b, true).blocks,
        generate_region_with_biases(7, -20, -20, 60, 60, &m, true).blocks
    );
}

#[test]
fn sea_ice_band_data_is_sane() {
    use data::{SEA_ICE_FLOE_SCALE, SEA_ICE_FLOE_STRENGTH, SEA_ICE_T_ICE};
    // The band is [t_ice, 0.5): nothing warmer than today's edge freezes.
    const { assert!(SEA_ICE_T_ICE >= 0.0 && SEA_ICE_T_ICE < 0.5) };
    const { assert!(SEA_ICE_FLOE_SCALE > 0.0 && SEA_ICE_FLOE_STRENGTH > 0.0) };
    assert!(!Biases::default().hard_ice_edge, "band is on by default");
}

#[test]
fn legend_exposes_blend_radius() {
    assert!(LEGEND_JSON.contains("\"blend\":{\"radius\":3,"));
}

#[test]
fn legend_exposes_blend_shade_keys() {
    assert!(LEGEND_JSON.contains(",\"shade_mix\":0.25,\"shade_radius\":1}"));
}

#[test]
fn dock_blocks_load_by_name_as_full_opaque_cubes() {
    for (name, id) in [("dock_planks", 81), ("dock_post", 82)] {
        assert_eq!(block_id(name), Some(id));
        let info = &BLOCK_INFO[id as usize];
        assert_eq!(info.collision, Collision::Full);
        assert_eq!(info.opacity, 255);
        assert!(info.opaque_cube && !info.liquid && !info.waterlogged);
    }
}

#[test]
fn existing_block_ids_are_unchanged() {
    for (name, id) in [
        ("air", 0),
        ("stone", 1),
        ("thatch", 69),
        ("barrel", 70),
        ("plank_step", 71),
        ("floe_ice", 76),
        ("kelp_0", 77),
        ("oyster", 79),
        ("scallop", 80),
    ] {
        assert_eq!(block_id(name), Some(id), "{name}");
    }
    let mut names: Vec<_> = BLOCK_INFO.iter().map(|b| b.name).collect();
    assert_eq!(names.len(), 84);
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 84);
}
