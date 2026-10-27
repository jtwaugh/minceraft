use crate::validation::*;
use crate::{validate_decorated, Level};
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

const W: u32 = 60;
const D: u32 = 40;
const TOP: usize = SEA_LEVEL as usize + 1;

/// Region with a flat grass island on x 10..30, z 10..30 (top at TOP).
fn island(x0: i32) -> Region {
    let mut blocks = vec![Block::Air as u8; (W * D) as usize * HEIGHT];
    for z in 0..D {
        for x in 0..W {
            let base = ((z * W + x) as usize) * HEIGHT;
            let wx = x as i32 + x0;
            let land = (10..30).contains(&wx) && (10..30).contains(&z);
            for y in 0..SEA_LEVEL as usize {
                blocks[base + y] = if y < 50 || land {
                    Block::Stone as u8
                } else {
                    Block::Water as u8
                };
            }
            if land {
                blocks[base + SEA_LEVEL as usize] = Block::Grass as u8;
            }
        }
    }
    Region {
        blocks,
        width: W,
        depth: D,
        height: HEIGHT as u32,
        x0,
        z0: 0,
    }
}

fn put(r: &mut Region, x: i32, y: usize, z: i32, b: Block) {
    let i = (((z - r.z0) as u32 * r.width + (x - r.x0) as u32) as usize) * HEIGHT + y;
    r.blocks[i] = b as u8;
}

fn level() -> Level {
    Level {
        id: "t".into(),
        week: 1,
        anchorage: [0, 20],
        island_center: [20, 20],
        spawn_water_radius: 4.0,
    }
}

fn measured(r: &Region, name: &str) -> f64 {
    validate_decorated(r, &level(), &Thresholds::shipped())
        .check(name)
        .unwrap()
        .measured
}

#[test]
fn bare_island_has_clear_tiles_and_no_trees() {
    let r = island(0);
    let rep = validate_decorated(&r, &level(), &Thresholds::shipped());
    assert!(rep.check(CHECK_CLEAR_FLAT).unwrap().passed);
    assert_eq!(measured(&r, CHECK_CLEAR_FLAT), 324.0); // 18 x 18 interior
    assert!(!rep.check(CHECK_TREES).unwrap().passed);
}

#[test]
fn tree_decorates_its_tile_and_counts() {
    let mut r = island(0);
    put(&mut r, 20, TOP, 20, Block::OakLog);
    put(&mut r, 20, TOP + 1, 20, Block::OakLog);
    put(&mut r, 20, TOP + 2, 20, Block::OakLeaves);
    assert_eq!(measured(&r, CHECK_TREES), 1.0);
    // Only the decorated tile is dropped; its neighbours stay flat.
    assert_eq!(measured(&r, CHECK_CLEAR_FLAT), 323.0);
}

#[test]
fn leaves_alone_decorate_but_are_not_a_tree() {
    let mut r = island(0);
    put(&mut r, 20, TOP + 3, 20, Block::OakLeaves);
    assert_eq!(measured(&r, CHECK_TREES), 0.0);
    assert_eq!(measured(&r, CHECK_CLEAR_FLAT), 323.0);
}

#[test]
fn new_flora_decorates_its_tile() {
    for b in [
        Block::Daisy,
        Block::Bluebell,
        Block::TallGrassDoubleLower,
        Block::TallGrassDoubleUpper,
        Block::BerryBush,
        Block::DryGrass,
        Block::DryGrassTallLower,
        Block::DryGrassTallUpper,
        Block::LeafLitter,
    ] {
        let mut r = island(0);
        put(&mut r, 20, TOP, 20, b);
        assert_eq!(measured(&r, CHECK_CLEAR_FLAT), 323.0, "{b:?}");
    }
}

#[test]
fn island_covered_in_plants_fails_clear_tiles() {
    let mut r = island(0);
    for z in 10..30 {
        for x in 10..30 {
            put(&mut r, x, TOP, z, Block::TallGrass);
        }
    }
    let rep = validate_decorated(&r, &level(), &Thresholds::shipped());
    assert_eq!(rep.check(CHECK_CLEAR_FLAT).unwrap().measured, 0.0);
    assert!(!rep.check(CHECK_CLEAR_FLAT).unwrap().passed);
}

#[test]
fn canopy_over_water_is_not_island() {
    let mut r = island(0);
    // A log and canopy over open water next to the island (x 30, z 20).
    put(&mut r, 30, SEA_LEVEL as usize, 20, Block::OakLeaves);
    put(&mut r, 30, TOP, 20, Block::OakLog);
    assert_eq!(measured(&r, CHECK_TREES), 0.0);
    assert_eq!(measured(&r, CHECK_CLEAR_FLAT), 324.0);
}

#[test]
fn same_counts_from_overlapping_windows() {
    let mut a = island(0);
    let mut b = island(5);
    for r in [&mut a, &mut b] {
        put(r, 15, TOP, 15, Block::OakLog);
        put(r, 22, TOP, 12, Block::Cactus);
    }
    for name in [CHECK_CLEAR_FLAT, CHECK_TREES] {
        assert_eq!(measured(&a, name), measured(&b, name), "{name}");
    }
}
