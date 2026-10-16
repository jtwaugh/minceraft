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

/// Water depth grid -> shore distance, via the real transform.
fn shore(depth: &[u8], w: u32, d: u32) -> Vec<u8> {
    super::derived::shore_distance(depth, w, d)
}

#[test]
fn shore_distance_hand_built() {
    // Land at the left end of a 1-row strip: distance counts columns.
    assert_eq!(shore(&[0, 5, 5, 5], 4, 1), [0, 1, 2, 3]);
    // Euclidean, rounded: land at (0,0); (1,1) -> 1.41 -> 1, (2,1) -> 2.24 -> 2,
    // (2,2) -> 2.83 -> 3.
    let mut g = vec![5u8; 9];
    g[0] = 0;
    assert_eq!(shore(&g, 3, 3), [0, 1, 2, 1, 1, 2, 2, 2, 3]);
    // No land at all reads the cap.
    assert_eq!(shore(&[5; 6], 3, 2), [SHORE_DISTANCE_CAP; 6]);
}

#[test]
fn shore_distance_caps_at_64() {
    let mut g = vec![5u8; 100];
    g[0] = 0;
    let s = shore(&g, 100, 1);
    assert_eq!(s[63], 63);
    assert_eq!(s[64], 64);
    assert_eq!(s[99], 64);
}

#[test]
fn shore_distance_sees_land_outside_the_requested_area() {
    // Find a window of open water whose nearest land lies just outside it.
    let layout = Layout::new(12345);
    let wide = DerivedMaps::generate(&layout, -300, -300, 600, 600, false);
    let mut small_hits = 0;
    for z0 in (-300..270).step_by(30) {
        for x0 in (-300..270).step_by(30) {
            let m = DerivedMaps::generate(&layout, x0, z0, 24, 24, false);
            for z in 0..24usize {
                for x in 0..24usize {
                    let wi = (z0 + 300) as usize + z;
                    let wj = (x0 + 300) as usize + x;
                    let want = wide.shore_distance[wi * 600 + wj];
                    assert_eq!(m.shore_distance[z * 24 + x], want, "({x0},{z0}) {x},{z}");
                    if m.water_depth[z * 24 + x] > 0 && want > 0 && want < 64 {
                        small_hits += 1;
                    }
                }
            }
        }
    }
    assert!(small_hits > 0);
}

#[test]
fn shore_distance_overlapping_regions_agree() {
    let layout = Layout::new(12345);
    for decorate in [false, true] {
        let a = DerivedMaps::generate(&layout, -100, -100, 96, 96, decorate);
        let b = DerivedMaps::generate(&layout, -60, -76, 80, 90, decorate);
        let mut shared = 0;
        for z in -76..-4 {
            for x in -60..-4 {
                let i = ((z + 100) * 96 + x + 100) as usize;
                let j = ((z + 76) * 80 + x + 60) as usize;
                assert_eq!(a.shore_distance[i], b.shore_distance[j], "({x},{z})");
                assert_eq!(a.water_depth[i], b.water_depth[j], "({x},{z})");
                shared += 1;
            }
        }
        assert_eq!(shared, 56 * 72);
    }
}
