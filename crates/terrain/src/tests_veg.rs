//! Tests for Part II: climate / biomes, vegetation, snow and ice, the
//! protected mask and position-determinism of decoration.

use super::*;
use crate::data::{blocks_movement, is_opaque_cube};

/// Expected biome for table entry (i, j), written independently from the
/// *integer* form of SPEC 8.2 (P = i * j), not from the float thresholds.
fn expected_biome(i: u32, j: u32) -> Biome {
    let p = i * j;
    if i <= 6 {
        Biome::Tundra
    } else if p <= 793 && i <= 31 {
        Biome::Tundra
    } else if p <= 793 && i <= 59 {
        Biome::Savanna
    } else if p <= 793 {
        Biome::Desert
    } else if p >= 1985 && i <= 44 {
        Biome::Swampland
    } else if i <= 31 {
        Biome::Taiga
    } else if i <= 61 && p <= 1389 {
        Biome::Shrubland
    } else if i <= 61 {
        Biome::Forest
    } else if p <= 1786 {
        Biome::Plains
    } else if p <= 3572 {
        Biome::SeasonalForest
    } else {
        Biome::Rainforest
    }
}

#[test]
fn biome_table_matches_spec() {
    let mut counts = [0u32; Biome::COUNT];
    for j in 0..64 {
        for i in 0..64 {
            let b = biome_lookup(i, j);
            assert_eq!(b, expected_biome(i, j), "entry ({i}, {j})");
            counts[b as usize] += 1;
            // Through the climate path: values inside the cell quantise to it.
            let (t, h) = ((i as f64 + 0.5) / 63.0, (j as f64 + 0.5) / 63.0);
            if i < 63 && j < 63 {
                assert_eq!(biome_for(t, h), b);
            }
        }
    }
    // SPEC 8.2 cell counts per biome.
    let want = [
        (Biome::Tundra, 1575),
        (Biome::Forest, 839),
        (Biome::Savanna, 518),
        (Biome::Taiga, 473),
        (Biome::Shrubland, 397),
        (Biome::Swampland, 138),
        (Biome::SeasonalForest, 57),
        (Biome::Desert, 54),
        (Biome::Plains, 32),
        (Biome::Rainforest, 13),
        (Biome::IceDesert, 0),
    ];
    for (b, n) in want {
        assert_eq!(counts[b as usize], n, "{:?}", b);
    }
}

#[test]
fn biome_at_deterministic_and_shared_climate() {
    let g = Generator::new(77);
    for i in -20..20 {
        let (x, z) = (i * 131, i * -57);
        assert_eq!(biome_at(77, x, z), biome_at(77, x, z));
        assert_eq!(climate_at(77, x, z), g.climate(x, z));
        assert_eq!(biome_at(77, x, z), g.biome(x, z));
    }
}

fn col(r: &Region, x: u32, z: u32) -> &[u8] {
    let i = r.index(x, 0, z);
    &r.blocks[i..i + 128]
}

fn is_log(b: u8) -> bool {
    b == Block::OakLog as u8 || b == Block::BirchLog as u8 || b == Block::SpruceLog as u8
}

/// Checks every plant's ground and the sugar cane / cactus rules.
fn check_plants(r: &Region) -> [usize; Block::COUNT] {
    let mut seen = [0usize; Block::COUNT];
    let at = |x: i32, y: i32, z: i32| -> Option<u8> {
        if x < 0 || z < 0 || x >= r.width as i32 || z >= r.depth as i32 || !(0..128).contains(&y) {
            None
        } else {
            Some(r.blocks[r.index(x as u32, y as u32, z as u32)])
        }
    };
    for z in 0..r.depth as i32 {
        for x in 0..r.width as i32 {
            for y in 1..127 {
                let b = at(x, y, z).unwrap();
                seen[b as usize] += 1;
                let below = at(x, y - 1, z).unwrap();
                let blk = Block::from_u8(b);
                let ok = match blk {
                    Block::TallGrass | Block::Fern | Block::Dandelion | Block::Rose => {
                        below == GRASS || below == DIRT
                    }
                    Block::DeadBush => below == SAND,
                    Block::BrownMushroom | Block::RedMushroom => is_opaque_cube(below),
                    Block::Pumpkin => below == GRASS || below == DIRT,
                    Block::SnowLayer => blocks_movement(below) && below != ICE,
                    Block::SugarCane => {
                        below == b || {
                            (below == GRASS || below == DIRT)
                                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                                    .iter()
                                    // Unknown (outside the region) counts as possible water.
                                    .any(|&(dx, dz)| {
                                        at(x + dx, y - 1, z + dz).is_none_or(|n| n == WATER)
                                    })
                        }
                    }
                    Block::Cactus => {
                        (below == SAND || below == b)
                            && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dz)| {
                                at(x + dx, y, z + dz).is_none_or(|n| !blocks_movement(n))
                            })
                    }
                    _ => true,
                };
                assert!(
                    ok,
                    "{:?} at region ({x},{y},{z}) on {:?}",
                    blk,
                    Block::from_u8(below)
                );
            }
        }
    }
    seen
}

#[test]
fn plants_have_valid_ground_and_rules() {
    let mut total = [0usize; Block::COUNT];
    // A few areas, including desert (cactus/dead bush) and wet ones (cane).
    for &(seed, x0, z0) in &[
        (12345u64, 0, 0),
        (12345, 1400, -300),
        (7, -900, 600),
        (99, 300, 2200),
    ] {
        let r = generate_decorated(seed, x0, z0, 160, 160);
        let s = check_plants(&r);
        for i in 0..Block::COUNT {
            total[i] += s[i];
        }
    }
    for b in [
        Block::TallGrass,
        Block::Rose,
        Block::OakLeaves,
        Block::OakLog,
    ] {
        assert!(total[b as usize] > 0, "no {:?} generated", b);
    }
}

/// A 256x256 area with lots of desert, found by scanning the climate.
fn find_desert(seed: u64) -> (i32, i32) {
    let c = Climate::new(seed);
    let mut best = (0, 0, 0);
    for zi in -12..12 {
        for xi in -12..12 {
            let (x, z) = (xi * 256, zi * 256);
            let mut n = 0;
            for k in 0..64 {
                let (t, h) = c.raw(x + (k % 8) * 32, z + (k / 8) * 32);
                n += (biome_for(t, h) == Biome::Desert) as i32;
            }
            if n > best.0 {
                best = (n, x, z);
            }
        }
    }
    assert!(best.0 > 32, "no desert found");
    (best.1, best.2)
}

#[test]
fn desert_plants_exist_and_obey_rules() {
    let seed = 12345u64;
    let (x0, z0) = find_desert(seed);
    let r = generate_decorated(seed, x0, z0, 256, 256);
    let s = check_plants(&r);
    assert!(s[Block::Cactus as usize] > 0, "no cactus in a desert");
    assert!(s[Block::DeadBush as usize] > 0, "no dead bush in a desert");
}

#[test]
fn no_tree_overwrites_another_trunk() {
    trees::TRUNKS.with(|t| t.borrow_mut().clear());
    let (x0, z0, w, d) = (-64, 128, 192, 192);
    let r = generate_decorated(4242, x0, z0, w, d);
    let trunks = trees::TRUNKS.with(|t| t.borrow().clone());
    assert!(trunks.len() > 20, "too few trees: {}", trunks.len());
    let mut owner = std::collections::HashMap::new();
    for (ti, cells) in trunks.iter().enumerate() {
        for &(x, y, z, b) in cells {
            if let Some(prev) = owner.insert((x, y, z), ti).filter(|&p| p != ti) {
                panic!("trunks of trees {prev} and {ti} share ({x},{y},{z})");
            }
            let (rx, rz) = (x - x0, z - z0);
            if rx >= 0 && rz >= 0 && rx < w as i32 && rz < d as i32 {
                let got = r.blocks[r.index(rx as u32, y as u32, rz as u32)];
                assert_eq!(got, b, "trunk cell ({x},{y},{z}) overwritten");
            }
        }
    }
}

#[test]
fn desert_columns_have_sand_surface() {
    let seed = 12345u64;
    let (x0, z0) = find_desert(seed);
    let (w, d) = (256, 256);
    let r = generate_region(seed, x0, z0, w, d);
    let cm = generate_climate(seed, x0, z0, w, d, &[]);
    let (mut desert, mut sand) = (0, 0);
    for z in 0..d {
        for x in 0..w {
            if cm.biome[(z * w + x) as usize] != Biome::Desert as u8 {
                continue;
            }
            let c = col(&r, x, z);
            let (tb, ty, _, _) = column_summary(c);
            assert_ne!(tb, GRASS, "grass in desert at ({x},{z})");
            if ty > 65 {
                desert += 1;
                // Sand, or a bare-stone patch (soil <= 0).
                assert!(
                    tb == SAND || tb == STONE,
                    "desert top {:?}",
                    Block::from_u8(tb)
                );
                sand += (tb == SAND) as u32;
            }
        }
    }
    assert!(desert > 500, "sample has too little desert ({desert})");
    assert!(
        sand * 10 > desert * 8,
        "only {sand}/{desert} desert columns are sand"
    );
}

/// Snow and ice per SPEC 13, checked against the shared climate.
fn check_snow_ice(r: &Region, cm: &ClimateMap) -> (usize, usize) {
    let (mut snow, mut ice) = (0, 0);
    for z in 0..r.depth {
        for x in 0..r.width {
            let c = col(r, x, z);
            let t = cm.temperature[(z * r.width + x) as usize] as f64;
            // Sea ice: y = 63 is ice iff cold, never water when cold.
            if t < 0.5 - 1e-6 {
                assert_ne!(c[63], WATER, "unfrozen sea at ({x},{z}), T={t}");
            } else if t > 0.5 + 1e-6 {
                assert_ne!(c[63], ICE, "ice in warm column ({x},{z}), T={t}");
            }
            ice += (c[63] == ICE) as usize;
            let mut top = -1i32;
            for y in (1..128).rev() {
                let b = c[y];
                if blocks_movement(b) || b == WATER {
                    top = y as i32 + 1;
                    break;
                }
            }
            if top <= 0 || top >= 128 {
                continue;
            }
            let tp = t - (top - 64) as f64 / 64.0 * 0.3;
            let below = c[top as usize - 1];
            let at = c[top as usize];
            let want = tp < 0.5 && blocks_movement(below) && below != ICE;
            if (tp - 0.5).abs() < 1e-5 {
                continue; // f32 rounding of the exported temperature
            }
            if want {
                // Snow, unless a plant already occupies the spot.
                assert!(
                    at == Block::SnowLayer as u8 || crate::plants::is_plant(at),
                    "no snow at ({x},{top},{z}): {:?}",
                    Block::from_u8(at)
                );
            } else {
                assert_ne!(
                    at,
                    Block::SnowLayer as u8,
                    "unexpected snow at ({x},{top},{z})"
                );
            }
            snow += (at == Block::SnowLayer as u8) as usize;
        }
    }
    (snow, ice)
}

#[test]
fn snow_and_ice_follow_temperature() {
    let (x0, z0, w, d) = (-200, 50, 160, 160);
    // Natural climate.
    let r = generate_decorated(12345, x0, z0, w, d);
    let cm = generate_climate(12345, x0, z0, w, d, &[]);
    check_snow_ice(&r, &cm);
    // Forced cold: snow on all exposed land and ice on the sea.
    let cold = |_x: i32, _z: i32| -0.8;
    let tb: [&dyn TemperatureBias; 1] = [&cold];
    let b = Biases {
        temperature: &tb,
        ..Biases::default()
    };
    let r = generate_region_with_biases(12345, x0, z0, w, d, &b, true);
    let cm = generate_climate(12345, x0, z0, w, d, &tb);
    let (snow, ice) = check_snow_ice(&r, &cm);
    assert!(snow > (w * d / 4) as usize, "too little snow: {snow}");
    let water_cols = (0..d)
        .flat_map(|z| (0..w).map(move |x| (x, z)))
        .filter(|&(x, z)| col(&r, x, z)[62] == WATER)
        .count();
    assert!(water_cols == 0 || ice > 0, "cold sea without ice");
}

#[test]
fn temperature_bias_shifts_biomes() {
    let (x0, z0, w, d) = (0, 0, 256, 256);
    let count = |bias: f64, b: Biome| {
        let f = move |_x: i32, _z: i32| bias;
        let cm = generate_climate(5, x0, z0, w, d, &[&f]);
        cm.biome.iter().filter(|&&v| v == b as u8).count()
    };
    assert!(
        count(-0.6, Biome::Tundra) + count(-0.6, Biome::Taiga)
            > count(0.0, Biome::Tundra) + count(0.0, Biome::Taiga)
    );
    assert_eq!(count(-1.0, Biome::Forest), 0);
    assert_eq!(count(1.0, Biome::Tundra), 0);
    // The bias also reaches the terrain (density uses the same climate).
    let hot = |_x: i32, _z: i32| 0.5;
    let b = Biases {
        temperature: &[&hot],
        ..Biases::default()
    };
    let a = generate_region_with_biases(5, x0, z0, 64, 64, &b, false);
    let n = generate_region(5, x0, z0, 64, 64);
    assert_ne!(a.blocks, n.blocks);
}

#[test]
fn decoration_respects_protected_mask() {
    let seed = 4242u64;
    let (x0, z0, w, d) = (-64, 128, 160, 160);
    let mut mask = ColumnMask::new(x0 + 40, z0 + 40, 80, 80);
    for z in z0 + 40..z0 + 120 {
        for x in x0 + 40..x0 + 120 {
            if (x + z) % 7 != 0 {
                mask.set(x, z, true);
            }
        }
    }
    // Also a y-range closure mask: nothing above y = 70 anywhere in a strip.
    let strip = |x: i32, y: i32, _z: i32| x >= x0 && x < x0 + 30 && y >= 70;
    let terrain = generate_region(seed, x0, z0, w, d);
    let full = generate_decorated(seed, x0, z0, w, d);
    for (m, name) in [
        (&mask as &dyn ProtectedMask, "columns"),
        (&strip as &dyn ProtectedMask, "strip"),
    ] {
        let b = Biases {
            protected: Some(m),
            ..Biases::default()
        };
        let r = generate_region_with_biases(seed, x0, z0, w, d, &b, true);
        let mut changed_unprotected = 0;
        for z in 0..d {
            for x in 0..w {
                for y in 0..128 {
                    let i = r.index(x, y, z);
                    let (wx, wz) = (x0 + x as i32, z0 + z as i32);
                    if m.protected(wx, y as i32, wz) {
                        assert_eq!(
                            r.blocks[i], terrain.blocks[i],
                            "{name}: protected ({wx},{y},{wz}) modified"
                        );
                    } else if r.blocks[i] != terrain.blocks[i] {
                        changed_unprotected += 1;
                    }
                }
            }
        }
        assert!(changed_unprotected > 1000, "{name}: decoration did nothing");
    }
    // Sanity: the masked area would have been decorated without the mask.
    let mut diff = 0;
    for z in 41..119u32 {
        for x in 41..119u32 {
            if col(&full, x, z) != col(&terrain, x, z) {
                diff += 1;
            }
        }
    }
    assert!(diff > 100, "mask test area has no decoration to protect");
}

#[test]
fn overlapping_decorated_regions_agree() {
    let seed = 0xDEAD_BEEF;
    let (bx, bz, bw, bd) = (-77, -45, 120, 100);
    let big = generate_decorated(seed, bx, bz, bw, bd);
    for &(ox, oz, w, d) in &[(-30, 3, 29, 21), (-77, -45, 17, 33), (10, 20, 33, 35)] {
        let small = generate_decorated(seed, ox, oz, w, d);
        for z in 0..d {
            for x in 0..w {
                let a = col(&small, x, z);
                let b = col(&big, (ox - bx) as u32 + x, (oz - bz) as u32 + z);
                assert_eq!(
                    a,
                    b,
                    "column ({}, {}) differs",
                    ox + x as i32,
                    oz + z as i32
                );
            }
        }
    }
    // Heightmap path agrees with the decorated region.
    let hm = generate_heightmap_with_biases(seed, -30, 3, 29, 21, &Biases::default(), true);
    let small = generate_decorated(seed, -30, 3, 29, 21);
    for z in 0..21u32 {
        for x in 0..29u32 {
            let s = column_summary(col(&small, x, z));
            let j = ((z * 29 + x) * 4) as usize;
            assert_eq!(
                (s.0, s.1, s.2, s.3),
                (hm[j], hm[j + 1], hm[j + 2], hm[j + 3])
            );
        }
    }
    let _ = is_log;
}

/// Tree attempts per chunk, from SPEC 10.3:
///   count = [rand(10) = 0] + biome term,  N = trunc((D/8 + 4u + 4) / 3).
/// With D = 0, N is 1 or 2; the tree-density noise D is an 8-octave sum whose
/// practical magnitude rarely exceeds |D/8| ~ 8, so N is mostly in 0..=5 and
/// the mean of N over a large sample lies in [0, 5]. Hence:
///   N + 5 biomes (Forest, Taiga, Rainforest): mean attempts in [5.1, 10.1]
///   N + 2 (Seasonal Forest):                  mean attempts in [2.1, 7.1]
///   0 (Savanna, Shrubland, Swampland):        mean 0.1 (we allow 0.03..0.2)
///   -20 (Desert, Plains, Tundra):             exactly 0
/// Grown trees can only be fewer than attempts.
#[test]
fn tree_density_matches_spec_formula() {
    let mut stats = DecorationStats::default();
    for &(seed, x0, z0) in &[(1u64, 0, 0), (2, 4000, -3000), (3, -2500, 1700)] {
        let (_, s) = generate_decorated_with_stats(seed, x0, z0, 384, 384, &Biases::default());
        for b in 0..Biome::COUNT {
            let a = &mut stats.biomes[b];
            a.columns += s.biomes[b].columns;
            a.tree_attempts += s.biomes[b].tree_attempts;
            a.trees += s.biomes[b].trees;
        }
    }
    let enough = |b: Biome| stats.biomes[b as usize].columns >= 256 * 40;
    for b in Biome::ALL {
        let a = stats.attempts_per_chunk(b);
        assert!(stats.trees_per_chunk(b) <= a + 1e-9);
        if !enough(b) {
            continue;
        }
        let (lo, hi) = match b {
            Biome::Forest | Biome::Taiga | Biome::Rainforest => (5.1, 10.1),
            Biome::SeasonalForest => (2.1, 7.1),
            Biome::Savanna | Biome::Shrubland | Biome::Swampland | Biome::IceDesert => (0.03, 0.2),
            _ => (0.0, 0.0),
        };
        assert!(
            a >= lo && a <= hi,
            "{:?}: {a:.2} attempts/chunk outside [{lo}, {hi}]",
            b
        );
    }
    assert!(enough(Biome::Forest), "sample has too little forest");
    assert!(stats.trees_per_chunk(Biome::Forest) > 1.5);
}

/// Sub-regions `(x0, z0, w, d)` for the determinism tests, all inside
/// `REF`: negative, zero and cell-straddling origins, regions below one
/// 16x16 cell, and regions ending exactly on a cell boundary.
const SUBS: &[(i32, i32, u32, u32)] = &[
    (0, 0, 20, 20),
    (-1, -1, 20, 20),
    (5, 5, 1, 1),
    (0, 0, 1, 1),
    (-1, -1, 1, 1),
    (3, -7, 1, 17),
    (15, 15, 2, 2),
    (16, 16, 16, 16),
    (8, 8, 16, 16),
    (-8, -8, 16, 16),
    (-16, -16, 16, 16),
    (-17, -17, 33, 33),
    (-40, 7, 41, 9),
    (-60, -70, 100, 90),
];
/// The large reference region `(x0, z0, w, d)`.
const REF: (i32, i32, u32, u32) = (-100, -100, 232, 232);
const SEEDS: &[u64] = &[1, 12345, 0xDEAD_BEEF];

/// Asserts every column of `small` equals the same world column of `big`.
fn assert_cols_agree(small: &Region, big: &Region, ctx: &str) {
    for z in 0..small.depth {
        for x in 0..small.width {
            let (wx, wz) = (small.x0 + x as i32, small.z0 + z as i32);
            let (bx, bz) = ((wx - big.x0) as u32, (wz - big.z0) as u32);
            assert_eq!(
                col(small, x, z),
                col(big, bx, bz),
                "{ctx}: column ({wx}, {wz}) differs"
            );
        }
    }
}

#[test]
fn decorated_subregions_agree_with_reference() {
    let (rx, rz, rw, rd) = REF;
    for &seed in SEEDS {
        let big = generate_decorated(seed, rx, rz, rw, rd);
        for &(ox, oz, w, d) in SUBS {
            let small = generate_decorated(seed, ox, oz, w, d);
            assert_cols_agree(
                &small,
                &big,
                &format!("seed {seed} region {ox},{oz},{w}x{d}"),
            );
        }
    }
}

#[test]
fn decorated_subregions_agree_with_protected_mask() {
    let (rx, rz, rw, rd) = REF;
    for &seed in SEEDS {
        let mut mask = ColumnMask::new(-50, -50, 100, 100);
        for z in -50i32..50 {
            for x in -50i32..50 {
                if (x + 2 * z).rem_euclid(5) != 0 {
                    mask.set(x, z, true);
                }
            }
        }
        let b = Biases {
            protected: Some(&mask),
            ..Biases::default()
        };
        let big = generate_region_with_biases(seed, rx, rz, rw, rd, &b, true);
        for &(ox, oz, w, d) in SUBS {
            let small = generate_region_with_biases(seed, ox, oz, w, d, &b, true);
            assert_cols_agree(
                &small,
                &big,
                &format!("masked seed {seed} region {ox},{oz},{w}x{d}"),
            );
        }
    }
}

#[test]
fn zero_sized_regions_are_empty() {
    let mask = ColumnMask::new(0, 0, 8, 8);
    let b = Biases {
        protected: Some(&mask),
        ..Biases::default()
    };
    for &(w, d) in &[(0u32, 0u32), (0, 5), (5, 0), (0, 40), (40, 0)] {
        for &(x0, z0) in &[(0, 0), (-1, -1), (16, 16)] {
            for decorate in [false, true] {
                let r = generate_region_with_biases(7, x0, z0, w, d, &b, decorate);
                assert!(r.blocks.is_empty(), "{w}x{d} at {x0},{z0} not empty");
                assert_eq!((r.width, r.depth), (w, d));
            }
            assert!(generate_decorated(7, x0, z0, w, d).blocks.is_empty());
            assert!(generate_region(7, x0, z0, w, d).blocks.is_empty());
        }
    }
}

#[test]
fn undecorated_regions_agree_with_decorated_where_undecorated() {
    let (rx, rz, rw, rd) = REF;
    for &seed in SEEDS {
        let terrain = generate_region(seed, rx, rz, rw, rd);
        let deco = generate_decorated(seed, rx, rz, rw, rd);
        for &(ox, oz, w, d) in SUBS {
            let ctx = format!("seed {seed} region {ox},{oz},{w}x{d}");
            let t_small = generate_region(seed, ox, oz, w, d);
            assert_cols_agree(&t_small, &terrain, &ctx);
            let d_small = generate_decorated(seed, ox, oz, w, d);
            for z in 0..d {
                for x in 0..w {
                    let (bx, bz) = ((ox - rx) as u32 + x, (oz - rz) as u32 + z);
                    if col(&deco, bx, bz) == col(&terrain, bx, bz) {
                        assert_eq!(
                            col(&d_small, x, z),
                            col(&t_small, x, z),
                            "{ctx}: ({}, {}) decorated in small only",
                            ox + x as i32,
                            oz + z as i32
                        );
                    }
                }
            }
        }
    }
}
