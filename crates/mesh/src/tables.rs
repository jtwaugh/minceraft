//! Block ids, block flag tables and the face table, as in `viewer/mesh.js`.

/// Block ids (`data/blocks.toml`).
pub mod b {
    pub const AIR: u8 = 0;
    pub const STONE: u8 = 1;
    pub const DIRT: u8 = 2;
    pub const GRASS: u8 = 3;
    pub const SAND: u8 = 4;
    pub const GRAVEL: u8 = 5;
    pub const WATER: u8 = 6;
    pub const ICE: u8 = 7;
    pub const OAK_LOG: u8 = 8;
    pub const BIRCH_LOG: u8 = 9;
    pub const SPRUCE_LOG: u8 = 10;
    pub const OAK_LEAVES: u8 = 11;
    pub const BIRCH_LEAVES: u8 = 12;
    pub const SPRUCE_LEAVES: u8 = 13;
    pub const TALL_GRASS: u8 = 14;
    pub const FERN: u8 = 15;
    pub const DEAD_BUSH: u8 = 16;
    pub const DANDELION: u8 = 17;
    pub const ROSE: u8 = 18;
    pub const SUGAR_CANE: u8 = 19;
    pub const CACTUS: u8 = 20;
    pub const BROWN_MUSHROOM: u8 = 21;
    pub const RED_MUSHROOM: u8 = 22;
    pub const PUMPKIN: u8 = 23;
    pub const SNOW_LAYER: u8 = 24;
    pub const RAFT_PLANKS: u8 = 25;
    pub const BUTCHER_BLOCK: u8 = 26;
    pub const GRIDDLE: u8 = 27;
    pub const CRATE: u8 = 28;
    pub const FARMLAND_DRY: u8 = 29;
    pub const FARMLAND_WET: u8 = 30;
    pub const WHEAT_0: u8 = 31;
    pub const WHEAT_1: u8 = 32;
    pub const WHEAT_2: u8 = 33;
    pub const WHEAT_3: u8 = 34;
    pub const WHEAT_4: u8 = 35;
    pub const WHEAT_5: u8 = 36;
    pub const WHEAT_6: u8 = 37;
    pub const WHEAT_7: u8 = 38;
    pub const PLANTER_DIRT: u8 = 39;
    pub const PLANTER_FARMLAND_DRY: u8 = 40;
    pub const PLANTER_FARMLAND_WET: u8 = 41;
    pub const DAISY: u8 = 42;
    pub const BLUEBELL: u8 = 43;
    pub const TALL_GRASS_DOUBLE_LOWER: u8 = 44;
    pub const TALL_GRASS_DOUBLE_UPPER: u8 = 45;
    pub const BERRY_BUSH: u8 = 46;
    pub const DRY_GRASS: u8 = 47;
    pub const DRY_GRASS_TALL_LOWER: u8 = 48;
    pub const DRY_GRASS_TALL_UPPER: u8 = 49;
    pub const LEAF_LITTER: u8 = 50;
    pub const FERN_DOUBLE_LOWER: u8 = 51;
    pub const FERN_DOUBLE_UPPER: u8 = 52;
    pub const CATTAIL_LOWER: u8 = 53;
    pub const CATTAIL_UPPER: u8 = 54;
    pub const LILY_PAD: u8 = 55;
    pub const MARSH_ORCHID: u8 = 56;
    pub const BUSH: u8 = 57;
    pub const JUNGLE_BUSH: u8 = 58;
    pub const LARGE_FERN: u8 = 59;
    pub const BROADLEAF: u8 = 60;
    pub const CACTUS_FLOWER: u8 = 61;
    pub const FROST_GRASS: u8 = 62;
    pub const TULIP_RED: u8 = 63;
    pub const TULIP_SALMON: u8 = 64;
    pub const TULIP_YELLOW: u8 = 65;
    pub const DUNE_GRASS_LOWER: u8 = 66;
    pub const DUNE_GRASS_UPPER: u8 = 67;
}

use b::*;

const fn flags(ids: &[u8]) -> [bool; 256] {
    let mut a = [false; 256];
    let mut i = 0;
    while i < ids.len() {
        a[ids[i] as usize] = true;
        i += 1;
    }
    a
}

/// Full cubes that hide the faces of whatever is next to them.
#[rustfmt::skip]
const SOLID: [u8; 18] = [
    STONE, DIRT, GRASS, SAND, GRAVEL, OAK_LOG, BIRCH_LOG, SPRUCE_LOG, PUMPKIN, RAFT_PLANKS,
    BUTCHER_BLOCK, GRIDDLE, CRATE, FARMLAND_DRY, FARMLAND_WET, PLANTER_DIRT,
    PLANTER_FARMLAND_DRY, PLANTER_FARMLAND_WET,
];
pub const OPAQUE: [bool; 256] = flags(&SOLID);
pub const LEAVES: [bool; 256] = flags(&[OAK_LEAVES, BIRCH_LEAVES, SPRUCE_LEAVES]);
/// With fast leaves, leaves count as solid too.
pub const OPAQUE_FAST: [bool; 256] = {
    let mut a = OPAQUE;
    let mut i = 0;
    while i < 256 {
        a[i] = a[i] || LEAVES[i];
        i += 1;
    }
    a
};
/// Drawn as two crossed quads.
#[rustfmt::skip]
pub const CROSS: [bool; 256] = flags(&[
    TALL_GRASS, FERN, DEAD_BUSH, DANDELION, ROSE, SUGAR_CANE, BROWN_MUSHROOM, RED_MUSHROOM,
    DAISY, BLUEBELL, MARSH_ORCHID, TALL_GRASS_DOUBLE_LOWER, TALL_GRASS_DOUBLE_UPPER, DRY_GRASS,
    DRY_GRASS_TALL_LOWER, DRY_GRASS_TALL_UPPER, FERN_DOUBLE_LOWER, FERN_DOUBLE_UPPER,
    CATTAIL_LOWER, CATTAIL_UPPER, LARGE_FERN, BROADLEAF, CACTUS_FLOWER, FROST_GRASS, TULIP_RED,
    TULIP_SALMON, TULIP_YELLOW, DUNE_GRASS_LOWER, DUNE_GRASS_UPPER, WHEAT_0, WHEAT_1, WHEAT_2,
    WHEAT_3, WHEAT_4, WHEAT_5, WHEAT_6, WHEAT_7,
]);
/// Upper blocks of two-block plants.
pub const UPPER_HALF: [bool; 256] = flags(&[
    TALL_GRASS_DOUBLE_UPPER,
    DRY_GRASS_TALL_UPPER,
    FERN_DOUBLE_UPPER,
    CATTAIL_UPPER,
    DUNE_GRASS_UPPER,
]);
/// Blocks whose tiles vary from block to block.
pub const VARIES: [bool; 256] = flags(&[GRASS, SAND, GRAVEL, STONE, SNOW_LAYER]);

/// A block face: normal, four corner offsets and shade.
pub struct Face {
    pub d: [i32; 3],
    pub c: [[u8; 3]; 4],
    pub s: f64,
}

/// Top, bottom, +x, -x, +z, -z.
#[rustfmt::skip]
pub const FACES: [Face; 6] = [
    Face {
        d: [0, 1, 0],
        c: [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
        s: 1.0,
    },
    Face {
        d: [0, -1, 0],
        c: [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
        s: 0.5,
    },
    Face {
        d: [1, 0, 0],
        c: [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]],
        s: 0.8,
    },
    Face {
        d: [-1, 0, 0],
        c: [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]],
        s: 0.65,
    },
    Face {
        d: [0, 0, 1],
        c: [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
        s: 0.9,
    },
    Face {
        d: [0, 0, -1],
        c: [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]],
        s: 0.6,
    },
];

/// Texture coords for a face corner `[a, b, c]` (x, y, z within the face's
/// extents `[sa, sb, sc]`): tops map x,z and use all eight rotations and
/// mirrors of variant `v` (bit 0 swaps u and v, bit 1 flips u, bit 2 flips
/// v); bottoms stay as they are; sides map the horizontal axis to u, mirror
/// with bit 1 and put the tile's top row at the top of the block.
pub fn face_uv(f: &Face, [a, b, c]: [f64; 3], v: u32, [sa, sb, sc]: [f64; 3]) -> [f64; 2] {
    if f.d[1] > 0 {
        let (mut u, mut w, mut eu, mut ew) = (a, c, sa, sc);
        if v & 1 != 0 {
            (u, w, eu, ew) = (c, a, sc, sa);
        }
        return [
            if v & 2 != 0 { eu - u } else { u },
            if v & 4 != 0 { ew - w } else { w },
        ];
    }
    if f.d[1] < 0 {
        return [a, c];
    }
    let (u, eu) = if f.d[0] != 0 { (c, sc) } else { (a, sa) };
    [if v & 2 != 0 { eu - u } else { u }, sb - b]
}
