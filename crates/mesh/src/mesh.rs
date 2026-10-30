//! Core of `meshBlocks` in `viewer/mesh.js`: one quad per visible face of full
//! cubes, ice, water, leaves, cacti, bushes and snow layers, with climate tints
//! and per-block tile variants, and cross plants. Flat shapes, AO and greedy
//! merging come later; here AO is 0.
use crate::plants::cross;
use crate::shapes::{bounds, box_hidden, face_corners, is_box, LEAF_WEIGHT};
use crate::tables::{face_uv, CROSS, FACES, VARIES};
use crate::{js_round, pick_layer, variant_at, Quads, Tint, Tints, Variants};

/// Column height in blocks.
pub const H: usize = 128;

pub const AIR: u8 = 0;
pub const WATER: u8 = 6;
pub const ICE: u8 = 7;
const GRASS: u8 = 3;
const CACTUS: u8 = 20;
const SNOW_LAYER: u8 = 24;
/// Full cubes that hide the faces next to them (`SOLID` in `viewer/mesh.js`).
const SOLID: [u8; 18] = [
    1, 2, 3, 4, 5, 8, 9, 10, 23, 25, 26, 27, 28, 29, 30, 39, 40, 41,
];
/// Oak, birch and spruce leaves: solid only with `fast`.
const LEAVES: [u8; 3] = [11, 12, 13];

/// Texture layer of a face: `face_layer` is indexed `block * 3 + (0 top, 1 side,
/// 2 bottom)`, with the fast-leaves copy from 768 on.
fn layer_of(face_layer: &[u8], b: u8, dy: i32, fast: bool) -> u8 {
    let face = if dy > 0 {
        0
    } else if dy < 0 {
        2
    } else {
        1
    };
    face_layer[if fast { 768 } else { 0 } + b as usize * 3 + face]
}

/// The four vertex buffers of a meshed area plus the surface statistics.
pub struct Mesh {
    pub opaque: Quads,
    pub water: Quads,
    pub ice: Quads,
    pub cutout: Quads,
    pub mean_y: f64,
    pub min_y: usize,
}

/// Mesh `blocks`, laid out `((z * w) + x) * 128 + y` for `w = size + 2 * pad`
/// columns; only the inner `size` x `size` columns are meshed and the pad ring
/// is read for culling. Solid ground is clipped 8 blocks under the lowest
/// surface. `tints` are for the inner columns. Positions are local to the area
/// (x and z from 0, y the world y); `origin` is the world x and z of block
/// (0, 0), which picks the tile variants.
pub fn mesh_blocks(
    blocks: &[u8],
    size: usize,
    pad: usize,
    fast: bool,
    face_layer: &[u8],
    tints: &Tints,
    origin: [i32; 2],
    variants: &Variants,
) -> Mesh {
    let w = size + 2 * pad;
    let col = |x: usize, z: usize| ((z + pad) * w + x + pad) * H;
    let (mut min_y, mut sum_y) = (H, 0usize);
    for z in 0..size {
        for x in 0..size {
            let base = col(x, z);
            let mut y = H - 1;
            while y > 0 && matches!(blocks[base + y], AIR | WATER | ICE) {
                y -= 1;
            }
            min_y = min_y.min(y);
            sum_y += y;
        }
    }
    let y0 = min_y.saturating_sub(8);
    let (size_i, pad_i) = (size as i32, pad as i32);
    let at = |x: i32, y: i32, z: i32| -> u8 {
        if x < -pad_i
            || z < -pad_i
            || x >= size_i + pad_i
            || z >= size_i + pad_i
            || y < y0 as i32
            || y >= H as i32
        {
            AIR
        } else {
            blocks[(((z + pad_i) as usize * w) + (x + pad_i) as usize) * H + y as usize]
        }
    };
    let solid = |b: u8| SOLID.contains(&b) || (fast && LEAVES.contains(&b));
    let mut out = Mesh {
        opaque: Quads::new(),
        water: Quads::new(),
        ice: Quads::new(),
        cutout: Quads::new(),
        mean_y: sum_y as f64 / (size * size) as f64,
        min_y,
    };
    for z in 0..size {
        for x in 0..size {
            let base = col(x, z);
            for y in y0..H {
                let b = blocks[base + y];
                if CROSS[b as usize] {
                    let side = layer_of(face_layer, b, 0, false);
                    cross(&mut out.cutout, b, [x, y, z], origin, side, tints, variants);
                    continue;
                }
                if b != WATER
                    && b != ICE
                    && !SOLID.contains(&b)
                    && !LEAVES.contains(&b)
                    && !is_box(b)
                {
                    continue;
                }
                let bx = bounds(b);
                for f in &FACES {
                    let (d, s) = (&f.d, f.s);
                    let c = &face_corners(f, bx);
                    if y == y0 && d[1] < 0 {
                        continue;
                    }
                    let n = at(x as i32 + d[0], y as i32 + d[1], z as i32 + d[2]);
                    let (mut depth, mut weight) = (0u8, None);
                    let quads = match b {
                        WATER => {
                            if n == WATER || n == ICE || solid(n) {
                                continue;
                            }
                            let mut k = 1;
                            while k < 255 && y >= k && blocks[base + y - k] == WATER {
                                k += 1;
                            }
                            depth = k as u8;
                            &mut out.water
                        }
                        ICE => {
                            if n == ICE || solid(n) {
                                continue;
                            }
                            &mut out.ice
                        }
                        _ if is_box(b) => {
                            if box_hidden(b, d[1], n, solid(n)) {
                                continue;
                            }
                            if b == CACTUS || b == SNOW_LAYER {
                                &mut out.opaque
                            } else {
                                weight = Some([LEAF_WEIGHT; 4]);
                                &mut out.cutout
                            }
                        }
                        _ => {
                            if solid(n) {
                                continue;
                            }
                            if LEAVES.contains(&b) && !fast {
                                weight = Some([LEAF_WEIGHT; 4]);
                                &mut out.cutout
                            } else {
                                &mut out.opaque
                            }
                        }
                    };
                    let vr = if VARIES[b as usize] {
                        variant_at(origin[0] + x as i32, y as i32, origin[1] + z as i32)
                    } else {
                        0
                    };
                    let uv = c.map(|q| face_uv(f, q, vr, [1.0; 3]));
                    let mut layer = layer_of(face_layer, b, d[1], fast);
                    if b == GRASS && d[1] > 0 {
                        layer = pick_layer(
                            variants,
                            layer,
                            origin[0] + x as i32,
                            y as i32,
                            origin[1] + z as i32,
                        );
                    }
                    let tint = match tints.block(b, x, z) {
                        None => Tint::One([1.0; 3]),
                        Some(flat) if !tints.corners => Tint::One(flat),
                        Some(_) => Tint::Four(c.map(|q| {
                            let o = [q[0], q[2]].map(|v| js_round(v) as usize);
                            tints.vertex(b, x, z, o[0], o[1])
                        })),
                    };
                    quads.quad(
                        [x as f64, y as f64, z as f64],
                        c,
                        &uv,
                        layer,
                        s * (0.75 + y as f64 / 256.0),
                        &tint,
                        weight,
                        None,
                        depth,
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const STONE: u8 = 1;
    const LEAF: u8 = 11;
    const PLANT: u8 = 14;

    fn layers() -> Vec<u8> {
        (0..1536).map(|i| (i % 251) as u8).collect()
    }

    /// size x size columns with pad, stone up to `height`, built by `f(x, z, y)`.
    fn world(size: usize, pad: usize, f: impl Fn(i32, i32, usize) -> u8) -> Vec<u8> {
        let w = size + 2 * pad;
        let mut v = alloc::vec![0u8; w * w * H];
        for z in 0..w {
            for x in 0..w {
                for y in 0..H {
                    v[(z * w + x) * H + y] = f(x as i32 - pad as i32, z as i32 - pad as i32, y);
                }
            }
        }
        v
    }

    #[test]
    fn single_cube_has_six_faces_and_clips_nothing() {
        let b = world(1, 0, |_, _, y| if y == 40 { STONE } else { 0 });
        let m = mesh_blocks(
            &b,
            1,
            0,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        assert_eq!(m.opaque.faces(), 6);
        assert_eq!(m.min_y, 40);
        assert_eq!(m.mean_y, 40.0);
    }

    #[test]
    fn culls_against_solids_and_pad_cells() {
        // a 2x2 slab one block thick: shared inner faces are hidden
        let b = world(2, 0, |_, _, y| if y == 40 { STONE } else { 0 });
        // top 4, bottom 4 (y0 is 32, so the bottom shows), outer sides 8
        assert_eq!(
            mesh_blocks(
                &b,
                2,
                0,
                false,
                &layers(),
                &Tints::NONE,
                [0, 0],
                &Variants::NONE
            )
            .opaque
            .faces(),
            4 + 4 + 8
        );
        // a solid pad ring hides the four outer sides
        let b = world(2, 1, |x, z, y| {
            if y == 40 && (-1..3).contains(&x) && (-1..3).contains(&z) {
                STONE
            } else {
                0
            }
        });
        assert_eq!(
            mesh_blocks(
                &b,
                2,
                1,
                false,
                &layers(),
                &Tints::NONE,
                [0, 0],
                &Variants::NONE
            )
            .opaque
            .faces(),
            4 + 4
        );
    }

    #[test]
    fn clips_below_y0() {
        // surface at 40 everywhere, stone from 0: only y >= 32 is meshed
        let b = world(2, 0, |_, _, y| if y <= 40 { STONE } else { 0 });
        let m = mesh_blocks(
            &b,
            2,
            0,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        // 4 tops + 4 columns of sides, 9 blocks (32..=40) tall, 2 per side * 4 sides
        assert_eq!(m.opaque.faces(), 4 + 4 * 2 * 9);
    }

    #[test]
    fn water_faces_carry_depth_and_hide_next_to_solids() {
        // stone floor at 40, water at 41..=43, plant beside at 41
        let b = world(2, 1, |x, z, y| match (x, z, y) {
            (-1, 0, 41) => PLANT,
            (_, _, 0..=40) => STONE,
            (0..=1, 0..=1, 41..=43) => WATER,
            _ => 0,
        });
        let m = mesh_blocks(
            &b,
            2,
            1,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        // tops 4, sides of the 2x2x3 column: 4 sides * 2 * 3
        assert_eq!(m.water.faces(), 4 + 24);
        let max = m.water.bytes().chunks(16).map(|v| v[15]).max().unwrap();
        assert_eq!(max, 3);
    }

    #[test]
    fn water_depth_counts_down_the_column() {
        let b = world(1, 0, |_, _, y| if y >= 1 { WATER } else { STONE });
        let m = mesh_blocks(
            &b,
            1,
            0,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        let max = m.water.bytes().chunks(16).map(|v| v[15]).max().unwrap();
        assert_eq!(max, 127);
    }

    #[test]
    fn ice_and_fast_leaves() {
        let b = world(1, 0, |_, _, y| match y {
            40 => STONE,
            41 => ICE,
            42 => LEAF,
            _ => 0,
        });
        let slow = mesh_blocks(
            &b,
            1,
            0,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        assert_eq!(
            (slow.ice.faces(), slow.opaque.faces(), slow.cutout.faces()),
            (5, 6, 6)
        );
        // fast leaves are solid: they hide the ice top and are drawn opaque
        let fast = mesh_blocks(
            &b,
            1,
            0,
            true,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        assert_eq!(
            (fast.ice.faces(), fast.opaque.faces(), fast.cutout.faces()),
            (4, 12, 0)
        );
    }

    #[test]
    fn same_output_for_offset_copies() {
        let f = |x: i32, z: i32, y: usize| {
            let h = 30 + ((x * 7 + z * 3).rem_euclid(5)) as usize;
            if y <= h {
                STONE
            } else if y <= 33 {
                WATER
            } else {
                0
            }
        };
        let a = world(4, 1, f);
        let b = world(4, 1, |x, z, y| f(x + 5, z + 5, y)); // same terrain at another origin (period 5)
        let (ma, mb) = (
            mesh_blocks(
                &a,
                4,
                1,
                false,
                &layers(),
                &Tints::NONE,
                [0, 0],
                &Variants::NONE,
            ),
            mesh_blocks(
                &b,
                4,
                1,
                false,
                &layers(),
                &Tints::NONE,
                [0, 0],
                &Variants::NONE,
            ),
        );
        assert_eq!(ma.opaque.bytes(), mb.opaque.bytes());
        assert_eq!(ma.water.bytes(), mb.water.bytes());
        assert_eq!(ma.mean_y, mb.mean_y);
    }

    #[test]
    fn tinted_faces_use_column_or_corner_tints() {
        // one grass block over stone; grass grid red in the column data
        let b = world(1, 0, |_, _, y| match y {
            40 => crate::tables::b::GRASS,
            _ => 0,
        });
        let mut data = alloc::vec![0.5f32; 4 * 3 + 4 * 4 * 3];
        data[..3].copy_from_slice(&[1.0, 0.0, 0.25]);
        let col = Tints {
            size: 1,
            data: &data,
            corners: false,
            colors: &[],
        };
        let m = mesh_blocks(&b, 1, 0, false, &layers(), &col, [0, 0], &Variants::NONE);
        assert!(m
            .opaque
            .bytes()
            .chunks(16)
            .all(|v| v[10..13] == [255, 0, 64]));
        // corner grass grid starts after the four column grids: corner (1, 0) is green
        let at = 4 * 3 + 3;
        data[at..at + 3].copy_from_slice(&[0.0, 1.0, 0.0]);
        let cor = Tints {
            size: 1,
            data: &data,
            corners: true,
            colors: &[],
        };
        let m = mesh_blocks(&b, 1, 0, false, &layers(), &cor, [0, 0], &Variants::NONE);
        // top face: corners (0,0) (0,1) (1,1) (1,0) in vertex order
        let top = &m.opaque.bytes()[..64];
        let rgb: Vec<[u8; 3]> = top.chunks(16).map(|v| [v[10], v[11], v[12]]).collect();
        assert_eq!(rgb, [[128; 3], [128; 3], [128; 3], [0, 255, 0]]);
    }

    #[test]
    fn variants_follow_the_world_coordinate() {
        // two 2x2 areas of stone and grass; the second starts one block east
        let f = |x: i32, y: usize| match (x, y) {
            (_, 40) => crate::tables::b::GRASS,
            (_, 39) => STONE,
            _ => 0,
        };
        let a = world(2, 0, |x, _, y| f(x, y));
        let tops = |origin| {
            let v = [3.0, 0.6, 0.8, 1.0, 0., 0., 0., 0., 0.];
            let mut table = alloc::vec![0.0; 256 * 9];
            table[10 * 9..11 * 9].copy_from_slice(&v);
            let mut fl = layers();
            fl[crate::tables::b::GRASS as usize * 3] = 10;
            let m = mesh_blocks(
                &a,
                2,
                0,
                false,
                &fl,
                &Tints::NONE,
                origin,
                &Variants(&table),
            );
            // per block (x, z) in world coordinates: bytes of the top face (uv and layer)
            m.opaque
                .bytes()
                .chunks(64)
                .filter(|q| q[8] >= 10 && q[8] < 13)
                .map(|q| {
                    let wx = i16::from_le_bytes([q[0], q[1]]) / 16 + origin[0] as i16;
                    let wz = i16::from_le_bytes([q[4], q[5]]) / 16 + origin[1] as i16;
                    (
                        wx,
                        wz,
                        q.iter().skip(6).take(3).copied().collect::<Vec<u8>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        // all tiles are distinguished only by (uv, layer), so compare the world block (1, 0)
        // seen from origin (0, 0) as block (1, 0) and from origin (1, 0) as block (0, 0)
        let at = |v: &Vec<(i16, i16, Vec<u8>)>, x, z| {
            v.iter().find(|e| e.0 == x && e.1 == z).map(|e| e.2.clone())
        };
        let (p, q) = (tops([0, 0]), tops([1, 0]));
        assert_eq!(at(&p, 1, 0), at(&q, 1, 0));
        assert!(at(&p, 1, 0).is_some());
    }
}
