use super::*;
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

const SURFACE: usize = (SEA_LEVEL - 1) as usize;

/// A 4x1 region: flat sea, a shelf, an ice column, a land column.
fn hand_built() -> Region {
    let mut blocks = vec![Block::Air as u8; 4 * HEIGHT];
    let mut fill = |x: usize, from: usize, to: usize, b: Block| {
        for y in from..=to {
            blocks[x * HEIGHT + y] = b as u8;
        }
    };
    // 0: flat sea, 10 deep over sand.
    fill(0, 0, SURFACE - 10, Block::Sand);
    fill(0, SURFACE - 9, SURFACE, Block::Water);
    // 1: shelf, 2 deep over gravel.
    fill(1, 0, SURFACE - 2, Block::Gravel);
    fill(1, SURFACE - 1, SURFACE, Block::Water);
    // 2: ice cap over 3 water cells over stone.
    fill(2, 0, SURFACE - 4, Block::Stone);
    fill(2, SURFACE - 3, SURFACE - 1, Block::Water);
    fill(2, SURFACE, SURFACE, Block::Ice);
    // 3: land.
    fill(3, 0, SURFACE + 3, Block::Stone);
    Region {
        blocks,
        width: 4,
        depth: 1,
        height: HEIGHT as u32,
        x0: 0,
        z0: 0,
    }
}

#[test]
fn hand_built_columns() {
    let m = DerivedMaps::from_region(&hand_built());
    assert_eq!(m.water_depth, [10, 2, 4, 0]);
    assert_eq!(
        m.bottom_block,
        [
            Block::Sand as u8,
            Block::Gravel as u8,
            Block::Stone as u8,
            NO_BOTTOM
        ]
    );
    assert_eq!(m.frozen, [false, false, true, false]);
}

#[test]
fn water_to_bedrock_has_no_bottom() {
    let mut r = hand_built();
    for y in 0..=SURFACE {
        r.blocks[y] = Block::Water as u8;
    }
    let m = DerivedMaps::from_region(&r);
    assert_eq!(m.water_depth[0] as usize, SURFACE + 1);
    assert_eq!(m.bottom_block[0], NO_BOTTOM);
}

#[test]
fn real_seed_matches_column_scan() {
    let r = terrain::generate_decorated(12345, -100, -100, 96, 96);
    let m = DerivedMaps::from_region(&r);
    let mut water = 0;
    for z in 0..r.depth {
        for x in 0..r.width {
            let i = (z * r.width + x) as usize;
            let top = r.get(x, SURFACE as u32, z);
            let wet = top == Block::Water || top == Block::Ice;
            assert_eq!(m.water_depth[i] > 0, wet);
            assert_eq!(m.frozen[i], top == Block::Ice);
            if wet {
                water += 1;
                let d = m.water_depth[i] as u32;
                let below = r.get(x, SURFACE as u32 - d, z);
                assert!(below != Block::Water && below != Block::Ice);
                assert_eq!(m.bottom_block[i], below as u8);
            } else {
                assert_eq!(m.bottom_block[i], NO_BOTTOM);
            }
        }
    }
    assert!(water > 0, "test window should contain water");
}

#[test]
fn overlapping_regions_agree() {
    let (seed, ox, oz) = (12345, -100, -100);
    for decorate in [false, true] {
        let gen = |x0, z0, w, d| {
            let r = if decorate {
                terrain::generate_decorated(seed, x0, z0, w, d)
            } else {
                terrain::generate_region(seed, x0, z0, w, d)
            };
            (DerivedMaps::from_region(&r), r)
        };
        let (a, ra) = gen(ox, oz, 96, 96);
        let (b, rb) = gen(ox + 40, oz + 24, 80, 90);
        let mut shared = 0;
        for z in oz + 24..oz + 96 {
            for x in ox + 40..ox + 96 {
                let i = a.index_of(&ra, x, z).unwrap();
                let j = b.index_of(&rb, x, z).unwrap();
                assert_eq!(a.water_depth[i], b.water_depth[j], "({x},{z})");
                assert_eq!(a.bottom_block[i], b.bottom_block[j], "({x},{z})");
                assert_eq!(a.frozen[i], b.frozen[j], "({x},{z})");
                shared += 1;
            }
        }
        assert_eq!(shared, 56 * 72);
    }
}
