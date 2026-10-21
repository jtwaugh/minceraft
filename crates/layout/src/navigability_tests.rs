use super::*;
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

const SURFACE: usize = (SEA_LEVEL - 1) as usize;

/// Columns: 0 depth 2, 1 depth 1, 2 ice over water, 3 depth 2 with a
/// leaf block at SEA_LEVEL + 3, 4 same with the leaf at SEA_LEVEL + 4.
fn hand_built() -> Region {
    let n = 5;
    let mut blocks = vec![Block::Air as u8; n * HEIGHT];
    let mut fill = |x: usize, from: usize, to: usize, b: Block| {
        for y in from..=to {
            blocks[x * HEIGHT + y] = b as u8;
        }
    };
    fill(0, 0, SURFACE - 2, Block::Sand);
    fill(0, SURFACE - 1, SURFACE, Block::Water);
    fill(1, 0, SURFACE - 1, Block::Sand);
    fill(1, SURFACE, SURFACE, Block::Water);
    fill(2, 0, SURFACE - 4, Block::Stone);
    fill(2, SURFACE - 3, SURFACE - 1, Block::Water);
    fill(2, SURFACE, SURFACE, Block::Ice);
    for x in [3, 4] {
        fill(x, 0, SURFACE - 2, Block::Sand);
        fill(x, SURFACE - 1, SURFACE, Block::Water);
    }
    fill(3, SURFACE + 4, SURFACE + 4, Block::OakLeaves);
    fill(4, SURFACE + 5, SURFACE + 5, Block::OakLeaves);
    Region {
        blocks,
        width: n as u32,
        depth: 1,
        height: HEIGHT as u32,
        x0: 0,
        z0: 0,
    }
}

fn t() -> Thresholds {
    Thresholds::shipped()
}

#[test]
fn constants_come_from_data_file() {
    assert_eq!((t().min_navigable_depth, t().navigable_clearance), (2, 4));
}

#[test]
fn depth_ice_and_overhang() {
    let m = NavMap::from_region(&hand_built(), &t());
    assert_eq!(m.navigable, [true, false, false, false, true]);
}

#[test]
fn removing_overhang_makes_column_navigable() {
    let mut r = hand_built();
    r.blocks[3 * HEIGHT + SURFACE + 4] = Block::Air as u8;
    assert!(NavMap::from_region(&r, &t()).at(3, 0));
}

#[test]
fn any_non_air_block_blocks() {
    for b in [Block::OakLeaves, Block::Stone, Block::Water] {
        let mut r = hand_built();
        r.blocks[HEIGHT + SURFACE + 1] = Block::Air as u8;
        r.blocks[SURFACE + 1] = b as u8;
        assert!(!NavMap::from_region(&r, &t()).at(0, 0), "{b:?}");
    }
}
