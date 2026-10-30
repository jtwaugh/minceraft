//! The core of `meshBlocks` from `viewer/mesh.js`: one quad per visible face
//! of full cubes, ice and water, with flat shade, white tint and AO 0.
//!
//! Leaves, cacti, bushes, snow layers, cross plants and flat shapes are not
//! meshed yet (fast leaves still hide faces like solid blocks), and nothing is
//! merged. Positions are local to the meshed area, so the output depends only
//! on the blocks, never on where the area is in the world.
use crate::quads::{Quads, Tint};

/// World height in blocks; `blocks` is laid out `((z * w) + x) * H + y`.
pub const H: usize = 128;

/// Block ids from `data/blocks.toml` (the `B` table in `viewer/mesh.js`).
pub mod id {
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
    pub const CACTUS: u8 = 20;
    pub const PUMPKIN: u8 = 23;
    pub const SNOW_LAYER: u8 = 24;
    pub const RAFT_PLANKS: u8 = 25;
    pub const CRATE: u8 = 28;
    pub const BERRY_BUSH: u8 = 46;
    pub const LEAF_LITTER: u8 = 50;
    pub const LILY_PAD: u8 = 55;
    pub const BUSH: u8 = 57;
    pub const JUNGLE_BUSH: u8 = 58;
}

const fn flags(ids: &[u8]) -> [bool; 256] {
    let mut a = [false; 256];
    let mut i = 0;
    while i < ids.len() {
        a[ids[i] as usize] = true;
        i += 1;
    }
    a
}

/// Full cubes that hide the faces next to them (`SOLID` in the JS).
const SOLID: [u8; 18] = [
    1, 2, 3, 4, 5, 8, 9, 10, 23, 25, 26, 27, 28, 29, 30, 39, 40, 41,
];
const LEAF_IDS: [u8; 3] = [id::OAK_LEAVES, id::BIRCH_LEAVES, id::SPRUCE_LEAVES];
const OPAQUE: [bool; 256] = flags(&SOLID);
/// With fast leaves, leaves hide faces too.
const OPAQUE_FAST: [bool; 256] = {
    let mut a = OPAQUE;
    let mut i = 0;
    while i < LEAF_IDS.len() {
        a[LEAF_IDS[i] as usize] = true;
        i += 1;
    }
    a
};
/// Blocks this mesher skips for now: leaves, cross plants, flat shapes,
/// cacti, bushes and snow layers (`CROSS` and `SHAPE_OF` in the JS, plus the
/// special shapes).
const NOT_YET: [bool; 256] = flags(&[
    11, 12, 13, // leaves
    14, 15, 16, 17, 18, 19, 21, 22, 31, 32, 33, 34, 35, 36, 37, 38, 42, 43, 44, 45, 47, 48, 49, 51,
    52, 53, 54, 56, 59, 60, 61, 62, 63, 64, 65, 66, 67, // cross plants
    50, 55, // leaf litter, lily pad
    20, 46, 57, 58, 24, // cactus, bushes, snow layer
]);

/// A face: normal, the four corner offsets of the unit cube, and its shade.
struct Face {
    d: [i32; 3],
    c: [[f64; 3]; 4],
    s: f64,
}

/// Top, bottom, +x, -x, +z, -z, in the JS order.
const FACES: [Face; 6] = [
    Face {
        d: [0, 1, 0],
        c: [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
        s: 1.0,
    },
    Face {
        d: [0, -1, 0],
        c: [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        s: 0.5,
    },
    Face {
        d: [1, 0, 0],
        c: [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
        s: 0.8,
    },
    Face {
        d: [-1, 0, 0],
        c: [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
        s: 0.65,
    },
    Face {
        d: [0, 0, 1],
        c: [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
        s: 0.9,
    },
    Face {
        d: [0, 0, -1],
        c: [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
        s: 0.6,
    },
];

/// Texture coordinates of a unit-cube corner, variant 0 (`faceUV` in the JS):
/// tops and bottoms map x, z; sides map the horizontal axis to u and put the
/// tile's top row at the top of the block.
fn face_uv(f: &Face, [a, b, c]: [f64; 3]) -> [f64; 2] {
    if f.d[1] != 0 {
        [a, c]
    } else {
        [if f.d[0] != 0 { c } else { a }, 1.0 - b]
    }
}

/// Texture layer of block `b`'s face with normal y `dy`, from the flat
/// `faceLayer` table (`faceLayers` in the JS: block * 3 + 0 top, 1 side,
/// 2 bottom; entries from 768 on are for fast leaves).
fn layer_of(face_layer: &[u8], b: u8, dy: i32, fast: bool) -> u8 {
    let k = if dy > 0 {
        0
    } else if dy < 0 {
        2
    } else {
        1
    };
    face_layer[if fast { 768 } else { 0 } + b as usize * 3 + k]
}

/// The four buffers of a mesh and the area's surface heights.
pub struct Mesh {
    pub opaque: Quads,
    pub water: Quads,
    pub ice: Quads,
    pub cutout: Quads,
    /// Mean surface height over the inner columns.
    pub mean_y: f64,
    /// Lowest surface height; solid ground is clipped 8 blocks below it.
    pub min_y: usize,
}

impl Mesh {
    /// Quad counts of opaque, water, ice and cutout.
    pub fn faces(&self) -> [usize; 4] {
        [
            self.opaque.faces(),
            self.water.faces(),
            self.ice.faces(),
            self.cutout.faces(),
        ]
    }
}

/// Mesh the inner `size` x `size` columns of `blocks`, which covers
/// `(size + 2 * pad)^2` columns. The pad ring only culls faces, so adjacent
/// areas meet without seams; with pad 0 the outside counts as air.
/// `face_layer` is the 1536-entry table described at [`layer_of`].
pub fn mesh_blocks(blocks: &[u8], face_layer: &[u8], size: usize, pad: usize, fast: bool) -> Mesh {
    let solid = if fast { &OPAQUE_FAST } else { &OPAQUE };
    let w = size + 2 * pad;
    assert_eq!(blocks.len(), w * w * H, "blocks must cover the padded area");
    assert!(face_layer.len() >= 1536, "faceLayer table has 1536 entries");
    let col = |x: usize, z: usize| ((z + pad) * w + x + pad) * H;

    // Surface of each column: the highest block that isn't air, water or ice.
    let (mut min_y, mut sum_y) = (H, 0usize);
    for z in 0..size {
        for x in 0..size {
            let base = col(x, z);
            let mut y = H - 1;
            while y > 0 && matches!(blocks[base + y], id::AIR | id::WATER | id::ICE) {
                y -= 1;
            }
            min_y = min_y.min(y);
            sum_y += y;
        }
    }
    let y0 = min_y.saturating_sub(8);

    // Block at inner-area coordinates; air outside the pad ring or below y0.
    let (p, s) = (pad as i32, size as i32);
    let at = |x: i32, y: i32, z: i32| -> u8 {
        if x < -p || z < -p || x >= s + p || z >= s + p || y < y0 as i32 || y >= H as i32 {
            id::AIR
        } else {
            blocks[(((z + p) as usize) * w + (x + p) as usize) * H + y as usize]
        }
    };

    let mut opaque = Quads::new();
    let mut water = Quads::new();
    let mut ice = Quads::new();
    let white = Tint::One([1.0, 1.0, 1.0]);
    let uv_of = |f: &Face| f.c.map(|q| face_uv(f, q));
    for z in 0..size {
        for x in 0..size {
            let base = col(x, z);
            for y in y0..H {
                let b = blocks[base + y];
                if b == id::AIR || NOT_YET[b as usize] {
                    continue;
                }
                let k_y = 0.75 + y as f64 / 256.0;
                let pos = [x as f64, y as f64, z as f64];
                for f in &FACES {
                    if y == y0 && f.d[1] < 0 {
                        continue;
                    }
                    let n = at(x as i32 + f.d[0], y as i32 + f.d[1], z as i32 + f.d[2]);
                    let layer = layer_of(face_layer, b, f.d[1], fast);
                    if b == id::WATER {
                        // only next to air, plants or (non-fast) leaves
                        if n == id::WATER || n == id::ICE || solid[n as usize] {
                            continue;
                        }
                        // water blocks from this one down to the first non-water
                        let mut depth = 1;
                        while depth < 255 && depth <= y && blocks[base + y - depth] == id::WATER {
                            depth += 1;
                        }
                        let k = f.s * k_y;
                        water.quad(
                            pos,
                            &f.c,
                            &uv_of(f),
                            layer,
                            k,
                            &white,
                            None,
                            None,
                            depth as u8,
                        );
                    } else if b == id::ICE {
                        if n == id::ICE || solid[n as usize] {
                            continue;
                        }
                        ice.quad(
                            pos,
                            &f.c,
                            &uv_of(f),
                            layer,
                            f.s * k_y,
                            &white,
                            None,
                            None,
                            0,
                        );
                    } else {
                        if solid[n as usize] {
                            continue;
                        }
                        opaque.quad(
                            pos,
                            &f.c,
                            &uv_of(f),
                            layer,
                            f.s * k_y,
                            &white,
                            None,
                            None,
                            0,
                        );
                    }
                }
            }
        }
    }
    Mesh {
        opaque,
        water,
        ice,
        cutout: Quads::new(),
        mean_y: sum_y as f64 / (size * size) as f64,
        min_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quads::VERTEX_BYTES;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Layer table where every face of block b is layer b (fast: b + 100).
    fn layers() -> Vec<u8> {
        (0..1536)
            .map(|i| ((i % 768) / 3 + if i >= 768 { 100 } else { 0 }) as u8)
            .collect()
    }

    struct Area {
        w: usize,
        blocks: Vec<u8>,
    }

    impl Area {
        /// A `w` x `w` area filled with stone up to y = 59 inclusive.
        fn ground(w: usize) -> Self {
            let mut a = Area {
                w,
                blocks: vec![id::AIR; w * w * H],
            };
            for z in 0..w {
                for x in 0..w {
                    for y in 0..60 {
                        a.set(x, y, z, id::STONE);
                    }
                }
            }
            a
        }
        fn set(&mut self, x: usize, y: usize, z: usize, b: u8) {
            self.blocks[(z * self.w + x) * H + y] = b;
        }
    }

    /// (block x, y, z, layer, shade byte, depth) of every quad, from its
    /// lowest corner. Normal-less, but enough to tell faces of one block apart
    /// together with the count.
    fn quads(q: &Quads) -> Vec<([i16; 3], [i16; 3], u8, u8, u8)> {
        q.bytes()
            .chunks(VERTEX_BYTES * 4)
            .map(|quad| {
                let v = |j: usize, a: usize| {
                    let o = j * VERTEX_BYTES + a * 2;
                    i16::from_le_bytes([quad[o], quad[o + 1]])
                };
                let lo = [0, 1, 2].map(|a| (0..4).map(|j| v(j, a)).min().unwrap());
                let hi = [0, 1, 2].map(|a| (0..4).map(|j| v(j, a)).max().unwrap());
                (lo, hi, quad[8], quad[9], quad[15])
            })
            .collect()
    }

    #[test]
    fn single_cube_shows_six_faces_and_ground_is_clipped() {
        let mut a = Area::ground(3);
        a.set(1, 60, 1, id::CRATE);
        let m = mesh_blocks(&a.blocks, &layers(), 3, 0, false);
        assert_eq!(m.min_y, 59);
        assert_eq!(m.mean_y, (59.0 * 8.0 + 60.0) / 9.0);
        // clip at y0 = 51: the ground is 9 layers (51..=59) of a 3x3 slab
        // with pad 0 (outside is air): top 9 (one under the crate), 4 sides
        // of 3 x 9, no bottom; the crate shows 5 faces (not its bottom).
        assert_eq!(m.faces(), [8 + 4 * 27 + 5, 0, 0, 0]);
        let q = quads(&m.opaque);
        assert!(
            q.iter().all(|(lo, ..)| lo[1] >= 51 * 16),
            "nothing below y0"
        );
        assert!(
            !q.iter()
                .any(|(lo, hi, ..)| lo[1] == hi[1] && lo[1] == 51 * 16),
            "no bottom at y0"
        );
        // the crate's top: layer 28, shade 1.0 * (0.75 + 60/256), white, AO 0
        let i = q
            .iter()
            .position(|(lo, hi, l, ..)| *l == 28 && lo[1] == 61 * 16 && hi[1] == 61 * 16)
            .unwrap();
        let v = &m.opaque.bytes()[i * 64..i * 64 + 16];
        assert_eq!(v[9], crate::js_round((0.75 + 60.0 / 256.0) * 128.0) as u8);
        assert_eq!(&v[10..16], &[255, 255, 255, 0, 0, 0]);
    }

    #[test]
    fn pad_ring_culls_edge_faces() {
        // 3x3 inner area with a pad of 1: the pad is solid too, so the
        // ground's sides are hidden and only its top shows.
        let a = Area::ground(5);
        let m = mesh_blocks(&a.blocks, &layers(), 3, 1, false);
        assert_eq!(m.faces(), [9, 0, 0, 0]);
        // Air in one pad column opens the inner faces next to it.
        let mut a = Area::ground(5);
        for y in 0..60 {
            a.set(0, y, 2, id::AIR);
        }
        let m = mesh_blocks(&a.blocks, &layers(), 3, 1, false);
        assert_eq!(m.faces(), [9 + 9, 0, 0, 0], "one -x column side, y 51..=59");
    }

    #[test]
    fn water_faces_only_against_air_or_plants_with_depth() {
        // a 3x3 pool, 3 deep (y 57..=59), in a 5x5 stone area with no pad
        let mut a = Area::ground(5);
        for z in 1..4 {
            for x in 1..4 {
                for y in 57..60 {
                    a.set(x, y, z, id::WATER);
                }
            }
        }
        a.set(2, 60, 2, id::TALL_GRASS); // a plant on the centre (not meshed yet)
        a.set(1, 60, 1, id::CRATE); // a solid on a corner
        let m = mesh_blocks(&a.blocks, &layers(), 5, 0, false);
        let w = quads(&m.water);
        // 9 tops minus the one under the crate; the plant doesn't hide water
        assert_eq!(m.faces()[1], 8);
        assert!(w
            .iter()
            .all(|(lo, hi, l, _, d)| lo[1] == 60 * 16 && hi[1] == 60 * 16 && *l == 6 && *d == 3));
        // none against water or stone: the pool's sides and floor show nothing
        assert!(w.iter().all(|(lo, ..)| lo[1] == 60 * 16));
    }

    #[test]
    fn water_depth_is_capped_at_255_and_counts_to_the_floor() {
        // a column of water from y 1 to 127 over stone at 0: the top's depth is 127
        let mut a = Area {
            w: 1,
            blocks: vec![id::AIR; H],
        };
        a.set(0, 0, 0, id::STONE);
        for y in 1..H {
            a.set(0, y, 0, id::WATER);
        }
        let m = mesh_blocks(&a.blocks, &layers(), 1, 0, false);
        let w = quads(&m.water);
        let top = w
            .iter()
            .find(|(lo, hi, ..)| lo[1] == hi[1] && lo[1] == 128 * 16)
            .unwrap();
        assert_eq!(top.4, 127);
        // the bottom water block's sides have depth 1; nothing faces the stone floor
        assert!(w.iter().any(|q| q.0[1] == 16 && q.4 == 1));
        assert!(w.iter().all(|q| q.4 >= 1 && q.0[1] >= 16));
    }

    #[test]
    fn ice_hides_ice_and_fast_leaves_count_as_solid() {
        let mut a = Area::ground(3);
        a.set(0, 60, 1, id::ICE);
        a.set(1, 60, 1, id::ICE);
        a.set(2, 60, 1, id::OAK_LEAVES);
        let slow = mesh_blocks(&a.blocks, &layers(), 3, 0, false);
        let fast = mesh_blocks(&a.blocks, &layers(), 3, 0, true);
        // two ice blocks in a row: 2 * 6 - 2 shared - 2 bottoms on stone = 8;
        // the +x face of the second ice is hidden only by fast leaves
        assert_eq!(slow.faces()[2], 8);
        assert_eq!(fast.faces()[2], 7);
        // leaves themselves aren't meshed yet, and fast picks the fast layers
        assert!(quads(&slow.opaque).iter().all(|q| q.2 < 100));
        assert!(quads(&fast.opaque).iter().all(|q| q.2 >= 100));
    }

    #[test]
    fn same_faces_regardless_of_area_origin() {
        // World: stone to y 59, with a few features. Mesh two 6x6 areas whose
        // world origins differ by (2, 3), each with a pad of 1, and compare
        // the faces of the columns they share.
        let world = |x: i32, y: usize, z: i32| -> u8 {
            match (x, y, z) {
                (_, 0..=59, _) => id::STONE,
                (4, 60, 5) | (5, 60, 5) => id::ICE,
                (3, 60..=62, 4) => id::OAK_LOG,
                (6, 60, 6) | (6, 60, 7) => id::WATER,
                (7, 60, 4) => id::PUMPKIN,
                _ => id::AIR,
            }
        };
        let (size, pad) = (6usize, 1usize);
        let mesh_at = |ox: i32, oz: i32| {
            let w = size + 2 * pad;
            let mut a = Area {
                w,
                blocks: vec![0; w * w * H],
            };
            for z in 0..w {
                for x in 0..w {
                    for y in 0..H {
                        a.set(x, y, z, world(ox + x as i32 - 1, y, oz + z as i32 - 1));
                    }
                }
            }
            mesh_blocks(&a.blocks, &layers(), size, pad, false)
        };
        let (ox, oz) = (2, 3);
        let m0 = mesh_at(0, 0);
        let m1 = mesh_at(ox, oz);
        assert_eq!(m0.min_y, m1.min_y);
        // faces in world coordinates, kept if their block is in both areas
        let shared = |m: &Mesh, dx: i32, dz: i32| {
            let mut out: Vec<_> = [&m.opaque, &m.water, &m.ice]
                .iter()
                .enumerate()
                .flat_map(|(i, q)| quads(q).into_iter().map(move |f| (i, f)))
                .map(|(i, (lo, hi, l, s, d))| {
                    let sh = |v: [i16; 3]| [v[0] + dx as i16 * 16, v[1], v[2] + dz as i16 * 16];
                    (i, sh(lo), sh(hi), l, s, d)
                })
                // Shared blocks are world x 2..=5, z 3..=5. Keep faces inside
                // that box, but not on its outer planes, where a face could
                // belong to a block only one area meshes.
                .filter(|(_, lo, hi, ..)| {
                    lo[0] >= 2 * 16
                        && hi[0] <= 6 * 16
                        && lo[2] >= 3 * 16
                        && hi[2] <= 6 * 16
                        && ![2 * 16, 6 * 16].contains(&(if lo[0] == hi[0] { lo[0] } else { -1 }))
                        && ![3 * 16, 6 * 16].contains(&(if lo[2] == hi[2] { lo[2] } else { -1 }))
                })
                .collect();
            out.sort();
            out
        };
        assert_eq!(shared(&m0, 0, 0), shared(&m1, ox, oz));
        assert!(!shared(&m0, 0, 0).is_empty());
    }
}
