use crate::validation::*;
use crate::{Layout, Level};
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

const W: u32 = 60;
const D: u32 = 40;

/// All-water region (stone below sea level, water up to it) at origin.
fn sea() -> Region {
    let mut blocks = vec![Block::Air as u8; (W * D) as usize * HEIGHT];
    for z in 0..D {
        for x in 0..W {
            let base = ((z * W + x) as usize) * HEIGHT;
            for y in 0..SEA_LEVEL as usize {
                blocks[base + y] = if y < 50 {
                    Block::Stone as u8
                } else {
                    Block::Water as u8
                };
            }
        }
    }
    Region {
        blocks,
        width: W,
        depth: D,
        height: HEIGHT as u32,
        x0: 0,
        z0: 0,
    }
}

fn raise(r: &mut Region, x: u32, z: u32) {
    let base = ((z * r.width + x) as usize) * HEIGHT;
    for y in 50..=SEA_LEVEL as usize {
        r.blocks[base + y] = Block::Stone as u8;
    }
}

fn rect(r: &mut Region, x: std::ops::Range<u32>, z: std::ops::Range<u32>) {
    for zz in z {
        for xx in x.clone() {
            raise(r, xx, zz);
        }
    }
}

fn level(anchorage: [i32; 2], island_center: [i32; 2]) -> Level {
    Level {
        id: "t".into(),
        week: 1,
        anchorage,
        island_center,
        spawn_water_radius: 8.0,
    }
}

fn t() -> Thresholds {
    Thresholds::shipped()
}

#[cfg(feature = "toml-io")]
#[test]
fn thresholds_come_from_data_file() {
    assert_eq!(t().min_island_columns, 150);
    let custom = Thresholds::from_toml(
        "min_island_columns = 4\nwindow_margin = 1\nmin_raft_water_depth = 3\nraft_clearance = 4\nmin_flat_grass_tiles = 6",
    ).unwrap();
    let mut r = sea();
    rect(&mut r, 40..42, 20..22);
    let rep = validate_island(&r, &level([5, 5], [40, 20]), &custom);
    assert!(rep.passed());
}

#[test]
fn small_island_fails_area() {
    let mut r = sea();
    rect(&mut r, 40..52, 10..22); // 144 columns
    let rep = validate_island(&r, &level([5, 20], [45, 15]), &t());
    let c = rep.check(CHECK_ISLAND_AREA).unwrap();
    assert_eq!(c.measured, 144.0);
    assert!(!c.passed);
    assert!(!rep.passed());
}

#[test]
fn larger_island_passes() {
    let mut r = sea();
    rect(&mut r, 40..53, 10..22); // 156 columns
    let rep = validate_island(&r, &level([5, 20], [45, 15]), &t());
    assert_eq!(rep.check(CHECK_ISLAND_AREA).unwrap().measured, 156.0);
    assert!(rep.passed(), "{:?}", rep);
}

#[test]
fn centre_not_land_gives_zero_area() {
    let mut r = sea();
    rect(&mut r, 40..53, 10..22);
    let rep = validate_island(&r, &level([5, 20], [20, 30]), &t());
    let c = rep.check(CHECK_ISLAND_AREA).unwrap();
    assert_eq!(c.measured, 0.0);
    assert!(!c.passed);
}

#[test]
fn diagonal_land_is_not_connected() {
    let mut r = sea();
    rect(&mut r, 40..53, 10..22);
    rect(&mut r, 53..60, 22..30); // touches only at a corner
    let rep = validate_island(&r, &level([5, 20], [45, 15]), &t());
    assert_eq!(rep.check(CHECK_ISLAND_AREA).unwrap().measured, 156.0);
}

#[test]
fn island_touching_spawn_zone_fails() {
    let mut r = sea();
    rect(&mut r, 20..33, 10..22);
    // Anchorage 7 blocks from the island edge column x=20: inside radius 8.
    let bad = validate_island(&r, &level([13, 15], [25, 15]), &t());
    let c = bad.check(CHECK_SPAWN_CLEAR).unwrap();
    assert!(!c.passed);
    assert!(c.measured > 0.0);
    assert!(bad.check(CHECK_ISLAND_AREA).unwrap().passed);
    // Moved back to 10 blocks away: clear.
    let good = validate_island(&r, &level([10, 15], [25, 15]), &t());
    assert!(good.check(CHECK_SPAWN_CLEAR).unwrap().passed);
    assert!(good.passed());
}

#[test]
fn window_holds_island_and_spawn_zone() {
    let l = level([0, 0], [28, 6]);
    let (x0, z0, w, d) = window(&l, &t());
    let m = t().window_margin;
    assert!(x0 <= -8 - m && z0 <= -8 - m);
    assert!(x0 + w as i32 > 28 + m && z0 + d as i32 > 8 + m);
}

#[test]
fn validation_is_deterministic() {
    let layout = Layout::new(12345);
    let l = level([0, 0], [28, 6]);
    let a = validate_level(&layout, &l, &t());
    let b = validate_level(&layout, &l, &t());
    assert_eq!(a, b);
    assert_eq!(a.checks.len(), 5);
}

fn footprint() -> Vec<(i32, i32)> {
    (8..13).flat_map(|x| (8..13).map(move |z| (x, z))).collect()
}

#[test]
fn deep_water_passes_and_shallow_fails() {
    let mut r = sea(); // water from y=50: 14 deep
    let rep = validate_raft(&r, &footprint(), &t());
    assert_eq!(rep.check(CHECK_RAFT_WATER).unwrap().measured, 14.0);
    assert!(rep.passed(), "{rep:?}");
    // Stone up to SEA_LEVEL - 3 leaves 2 deep water in one column.
    let base = ((10 * r.width + 10) as usize) * HEIGHT;
    for y in 50..(SEA_LEVEL as usize - 2) {
        r.blocks[base + y] = Block::Stone as u8;
    }
    let rep = validate_raft(&r, &footprint(), &t());
    let c = rep.check(CHECK_RAFT_WATER).unwrap();
    assert_eq!(c.measured, 2.0);
    assert!(!c.passed);
    // Exactly 3 deep passes.
    r.blocks[base + SEA_LEVEL as usize - 3] = Block::Water as u8;
    assert!(validate_raft(&r, &footprint(), &t()).passed());
}

#[test]
fn stone_in_clearance_fails() {
    let mut r = sea();
    let base = ((9 * r.width + 11) as usize) * HEIGHT;
    r.blocks[base + SEA_LEVEL as usize + 2] = Block::Stone as u8;
    let rep = validate_raft(&r, &footprint(), &t());
    let c = rep.check(CHECK_RAFT_CLEAR).unwrap();
    assert_eq!(c.measured, 1.0);
    assert!(!c.passed);
    // Just above the clearance is fine.
    r.blocks[base + SEA_LEVEL as usize + 2] = Block::Air as u8;
    r.blocks[base + SEA_LEVEL as usize + 5] = Block::Stone as u8;
    assert!(validate_raft(&r, &footprint(), &t()).passed());
}

#[test]
fn empty_footprint_fails() {
    assert!(!validate_raft(&sea(), &[], &t()).passed());
}

/// Grass-topped columns at height `SEA_LEVEL + 1`.
fn grass_island(r: &mut Region, x: std::ops::Range<u32>, z: std::ops::Range<u32>) {
    for zz in z {
        for xx in x.clone() {
            let base = ((zz * r.width + xx) as usize) * HEIGHT;
            for y in 50..=SEA_LEVEL as usize + 1 {
                r.blocks[base + y] = Block::Stone as u8;
            }
            r.blocks[base + SEA_LEVEL as usize + 1] = Block::Grass as u8;
        }
    }
}

#[test]
fn flat_grass_counts_interior_tiles() {
    let mut r = sea();
    grass_island(&mut r, 20..25, 10..15);
    // Edge columns neighbour water (ground far below), so only the 3x3 interior is flat.
    let rep = validate_flat_grass(&r, &level([5, 20], [22, 12]), &t());
    let c = rep.check(CHECK_FLAT_GRASS).unwrap();
    assert_eq!(c.measured, 9.0);
    assert!(c.passed);
}

#[test]
fn too_few_flat_grass_tiles_fails() {
    let mut r = sea();
    grass_island(&mut r, 20..24, 10..14); // 2x2 interior = 4
    let rep = validate_flat_grass(&r, &level([5, 20], [21, 11]), &t());
    let c = rep.check(CHECK_FLAT_GRASS).unwrap();
    assert_eq!(c.measured, 4.0);
    assert!(!c.passed);
}

#[test]
fn steep_neighbour_breaks_flatness() {
    let mut r = sea();
    grass_island(&mut r, 20..25, 10..15);
    let base = ((12 * r.width + 22) as usize) * HEIGHT;
    for y in SEA_LEVEL as usize + 2..SEA_LEVEL as usize + 4 {
        r.blocks[base + y] = Block::Stone as u8; // centre rises 2 above neighbours
    }
    let rep = validate_flat_grass(&r, &level([5, 20], [22, 12]), &t());
    // Centre is no longer grass-topped and its 4 neighbours see a 3-block step.
    assert_eq!(rep.check(CHECK_FLAT_GRASS).unwrap().measured, 4.0);
}

#[test]
fn raft_footprint_at_ignores_other_anchorages() {
    let mut layout = Layout::new(1);
    for origin in [[0, 64, 0], [500, 64, 0]] {
        layout.structures.push(crate::StructurePlacement {
            structure: "raft_start".into(),
            origin,
            rotation: 0,
        });
    }
    let both = raft_footprint(&layout);
    let one = raft_footprint_at(&layout, [500, 0]);
    assert!(!one.is_empty() && one.len() * 2 == both.len());
    assert!(one.iter().all(|&(x, _)| x > 400));
}
