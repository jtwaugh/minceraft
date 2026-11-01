//! Core of `meshBlocks` in `viewer/mesh.js`: one quad per visible face of full
//! cubes, ice, water, leaves, cacti, bushes and snow layers, with climate tints
//! and per-block tile variants, cross plants, flat shapes and vertex AO on
//! full cubes and leaves. Flat-tinted water tops always merge into rectangles,
//! as in the JS; with `greedy`, so do untinted cube faces of one AO level.
use crate::ao::{face_ao, flips, rot};
use crate::greedy::{merge, GRID_AXES};
use crate::plants::cross;
use crate::shapes::{
    bounds, box_hidden, cluster, face_corners, flat, flat_lift, is_box, is_cluster, LEAF_WEIGHT,
};
use crate::tables::{face_uv, CROSS, FACES, VARIES, WATERLOGGED};
use crate::{js_round, pick_layer, variant_at, Quads, Tint, Tints, Variants};
use alloc::vec::Vec;

/// Column height in blocks.
pub const H: usize = 128;

pub const AIR: u8 = 0;
pub const WATER: u8 = 6;
pub const ICE: u8 = 7;
pub const FLOE_ICE: u8 = crate::tables::b::FLOE_ICE;
const GRASS: u8 = 3;
const CACTUS: u8 = 20;
const SNOW_LAYER: u8 = 24;
/// Full cubes that hide the faces next to them (`SOLID` in `viewer/mesh.js`).
const SOLID: [u8; 18] = [
    1, 2, 3, 4, 5, 8, 9, 10, 23, 25, 26, 27, 28, 29, 30, 39, 40, 41,
];
/// Oak, birch and spruce leaves: solid only with `fast`.
const LEAVES: [u8; 3] = [11, 12, 13];

/// Water or a waterlogged block: water faces between them are hidden.
fn is_water(b: u8) -> bool {
    b == WATER || WATERLOGGED[b as usize]
}

/// Ice or floe ice: both draw as `ice` and hide each other's faces.
fn is_ice(b: u8) -> bool {
    b == ICE || b == FLOE_ICE
}

/// Whether `b` sways in the wind, so a snow layer on it sways too: cutout
/// leaves (not with `fast`) and bushes.
fn sways_under(b: u8, fast: bool) -> bool {
    (LEAVES.contains(&b) && !fast) || matches!(b, 46 | 57 | 58)
}

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

/// The water tint of column (x, z) as bytes, if all four corners agree
/// (`waterTint` in `viewer/mesh.js`).
fn water_tint(tints: &Tints, x: usize, z: usize) -> Option<[u8; 3]> {
    let key = |t: [f64; 3]| t.map(|v| js_round(v * 255.0) as u8);
    match tints.block(WATER, x, z) {
        None => Some([255; 3]),
        Some(t) if !tints.corners => Some(key(t)),
        Some(_) => {
            let k = key(tints.vertex(WATER, x, z, 0, 0));
            [(0, 1), (1, 1), (1, 0)]
                .iter()
                .all(|&(a, b)| key(tints.vertex(WATER, x, z, a, b)) == k)
                .then_some(k)
        }
    }
}

/// A water-top cell waiting to be merged: equal cells share a quad.
#[derive(PartialEq)]
struct WaterTop {
    layer: u8,
    depth: u8,
    tint: [u8; 3],
}

/// An untinted cube-face cell waiting to be merged. Cells with equal layer,
/// shade byte, AO level and UV variant share a quad, which takes the first
/// cell's unrounded shade `k`.
struct FaceCell {
    layer: u8,
    shade: i64,
    ao: u8,
    vr: u32,
    k: f64,
}

impl PartialEq for FaceCell {
    fn eq(&self, o: &Self) -> bool {
        (self.layer, self.shade, self.ao, self.vr) == (o.layer, o.shade, o.ao, o.vr)
    }
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
/// (0, 0), which picks the tile variants. `greedy` merges untinted cube faces.
pub fn mesh_blocks(
    blocks: &[u8],
    size: usize,
    pad: usize,
    fast: bool,
    greedy: bool,
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
            while y > 0
                && (matches!(blocks[base + y], AIR | ICE | FLOE_ICE) || is_water(blocks[base + y]))
            {
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
    // y -> per-column water-top cells, in the order the y values first appear
    let mut water_tops: Vec<(usize, Vec<Option<WaterTop>>)> = Vec::new();
    // (face, plane) -> grid of cube-face cells, in first-seen order; `slot` finds them
    let mut face_cells: Vec<((usize, usize), Vec<Option<FaceCell>>)> = Vec::new();
    let mut slot = alloc::vec![usize::MAX; FACES.len() * H.max(size)];
    for z in 0..size {
        for x in 0..size {
            let base = col(x, z);
            for y in y0..H {
                let mut b = blocks[base + y];
                if WATERLOGGED[b as usize] {
                    // the plant inside, then the cell's water faces as `water`
                    if is_cluster(b) {
                        let w = [origin[0] + x as i32, y as i32, origin[1] + z as i32];
                        let floor = solid(at(x as i32, y as i32 - 1, z as i32));
                        let layer = |dy| layer_of(face_layer, b, dy, false);
                        cluster(&mut out.cutout, [x, y, z], w, layer, floor);
                    } else {
                        let side = layer_of(face_layer, b, 0, false);
                        cross(&mut out.cutout, b, [x, y, z], origin, side, tints, variants);
                    }
                    b = WATER;
                }
                if let Some(lift) = flat_lift(b) {
                    let w = [origin[0] + x as i32, y as i32, origin[1] + z as i32];
                    let layer = layer_of(face_layer, b, 1, false);
                    let tint = match tints.block(b, x, z) {
                        None => Tint::One([1.0; 3]),
                        Some(t) if !tints.corners => Tint::One(t),
                        Some(_) => Tint::Four(
                            FACES[0]
                                .c
                                .map(|c| tints.vertex(b, x, z, c[0] as usize, c[2] as usize)),
                        ),
                    };
                    flat(&mut out.cutout, [x, y, z], w, lift, layer, &tint);
                    continue;
                }
                if CROSS[b as usize] {
                    let side = layer_of(face_layer, b, 0, false);
                    cross(&mut out.cutout, b, [x, y, z], origin, side, tints, variants);
                    continue;
                }
                if b != WATER
                    && !is_ice(b)
                    && !SOLID.contains(&b)
                    && !LEAVES.contains(&b)
                    && !is_box(b)
                {
                    continue;
                }
                let bx = bounds(b);
                for (fi, f) in FACES.iter().enumerate() {
                    let (d, s) = (&f.d, f.s);
                    let mut c = face_corners(f, bx);
                    if y == y0 && d[1] < 0 {
                        continue;
                    }
                    let n = at(x as i32 + d[0], y as i32 + d[1], z as i32 + d[2]);
                    let (mut depth, mut weight, mut ao) = (0u8, None, None);
                    let vr = if VARIES[b as usize] {
                        variant_at(origin[0] + x as i32, y as i32, origin[1] + z as i32)
                    } else {
                        0
                    };
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
                    let quads = match b {
                        WATER => {
                            if is_water(n) || is_ice(n) || solid(n) {
                                continue;
                            }
                            let mut k = 1;
                            while k < 255 && y >= k && is_water(blocks[base + y - k]) {
                                k += 1;
                            }
                            depth = k as u8;
                            if let Some(tint) = water_tint(tints, x, z).filter(|_| d[1] > 0) {
                                let layer = layer_of(face_layer, b, 1, fast);
                                let i = match water_tops.iter().position(|e| e.0 == y) {
                                    Some(i) => i,
                                    None => {
                                        water_tops
                                            .push((y, (0..size * size).map(|_| None).collect()));
                                        water_tops.len() - 1
                                    }
                                };
                                water_tops[i].1[z * size + x] =
                                    Some(WaterTop { layer, depth, tint });
                                continue;
                            }
                            &mut out.water
                        }
                        ICE | FLOE_ICE => {
                            if is_ice(n) || solid(n) {
                                continue;
                            }
                            &mut out.ice
                        }
                        _ if is_box(b) => {
                            if box_hidden(b, d[1], n, solid(n)) {
                                continue;
                            }
                            // snow on swaying foliage sways with it (`swaysUnder` in viewer/mesh.js)
                            let on_foliage =
                                b == SNOW_LAYER && y > 0 && sways_under(blocks[base + y - 1], fast);
                            if b == CACTUS || (b == SNOW_LAYER && !on_foliage) {
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
                            let p = [x as i32, y as i32, z as i32];
                            let a = face_ao(f, p, |x, y, z| solid(at(x, y, z)));
                            ao = Some(a);
                            let cut_leaf = LEAVES.contains(&b) && !fast;
                            let untinted = tints.block(b, x, z).is_none();
                            if greedy && !cut_leaf && untinted && a.iter().all(|&v| v == a[0]) {
                                let [pa, ua, va] = GRID_AXES[fi];
                                let (u_n, at3) = (if ua == 1 { H } else { size }, [x, y, z]);
                                let v_n = if va == 1 { H } else { size };
                                let key = fi * H.max(size) + at3[pa];
                                if slot[key] == usize::MAX {
                                    slot[key] = face_cells.len();
                                    let grid = (0..u_n * v_n).map(|_| None).collect();
                                    face_cells.push(((fi, at3[pa]), grid));
                                }
                                let k = s * (0.75 + y as f64 / 256.0);
                                let shade = js_round(k * 128.0) as i64;
                                let cell = FaceCell {
                                    layer,
                                    shade,
                                    ao: a[0],
                                    vr,
                                    k,
                                };
                                face_cells[slot[key]].1[at3[va] * u_n + at3[ua]] = Some(cell);
                                continue;
                            }
                            if cut_leaf {
                                weight = Some([LEAF_WEIGHT; 4]);
                                &mut out.cutout
                            } else {
                                &mut out.opaque
                            }
                        }
                    };
                    let mut uv = c.map(|q| face_uv(f, q, vr, [1.0; 3]));
                    let mut tint = match tints.block(b, x, z) {
                        None => Tint::One([1.0; 3]),
                        Some(flat) if !tints.corners => Tint::One(flat),
                        Some(_) => Tint::Four(c.map(|q| {
                            let o = [q[0], q[2]].map(|v| js_round(v) as usize);
                            tints.vertex(b, x, z, o[0], o[1])
                        })),
                    };
                    if let Some(a) = ao.filter(|&a| flips(a)) {
                        (c, uv, ao) = (rot(c), rot(uv), Some(rot(a)));
                        if let Tint::Four(t) = tint {
                            tint = Tint::Four(rot(t));
                        }
                    }
                    quads.quad(
                        [x as f64, y as f64, z as f64],
                        &c,
                        &uv,
                        layer,
                        s * (0.75 + y as f64 / 256.0),
                        &tint,
                        weight,
                        ao,
                        depth,
                    );
                }
            }
        }
    }
    let top = &FACES[0];
    for (y, cells) in &water_tops {
        merge(cells, size, size, |x, z, w, h, cell| {
            let p = top
                .c
                .map(|v| [(v[0] as usize * w) as f64, 1.0, (v[2] as usize * h) as f64]);
            let uv = p.map(|v| [v[0], v[2]]);
            let tint = Tint::One(cell.tint.map(|v| v as f64 / 255.0));
            let k = top.s * (0.75 + *y as f64 / 256.0);
            let at = [x as f64, *y as f64, z as f64];
            out.water
                .quad(at, &p, &uv, cell.layer, k, &tint, None, None, cell.depth);
        });
    }
    for ((fi, plane), cells) in &face_cells {
        let (f, [pa, ua, va]) = (&FACES[*fi], GRID_AXES[*fi]);
        let u_n = if ua == 1 { H } else { size };
        let v_n = if va == 1 { H } else { size };
        merge(cells, u_n, v_n, |u, v, w, h, cell| {
            let (mut at3, mut e) = ([0.0; 3], [1.0; 3]);
            (at3[pa], at3[ua], at3[va]) = (*plane as f64, u as f64, v as f64);
            (e[ua], e[va]) = (w as f64, h as f64);
            let p = f.c.map(|q| [0, 1, 2].map(|j| f64::from(q[j]) * e[j]));
            let uv = p.map(|q| face_uv(f, q, cell.vr, e));
            let (tint, ao) = (Tint::One([1.0; 3]), Some([cell.ao; 4]));
            out.opaque
                .quad(at3, &p, &uv, cell.layer, cell.k, &tint, None, ao, 0);
        });
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
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        // one merged 2x2 top, sides of the 2x2x3 column: 4 sides * 2 * 3
        assert_eq!(m.water.faces(), 1 + 24);
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
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        let max = m.water.bytes().chunks(16).map(|v| v[15]).max().unwrap();
        assert_eq!(max, 127);
    }

    const LOGGED: u8 = crate::tables::b::TEST_WATERLOGGED_PLANT;

    fn plain(b: &[u8], size: usize, pad: usize, origin: [i32; 2]) -> Mesh {
        mesh_blocks(
            b,
            size,
            pad,
            false,
            false,
            &layers(),
            &Tints::NONE,
            origin,
            &Variants::NONE,
        )
    }

    #[test]
    fn waterlogged_cells_draw_as_water_plus_a_plant() {
        // stone to 40, water 41..=44 with waterlogged cells at 42 and at the surface 44
        let column = |logged: bool| {
            world(1, 0, move |_, _, y| match y {
                0..=40 => STONE,
                42 | 44 if logged => LOGGED,
                41..=44 => WATER,
                _ => 0,
            })
        };
        let (m, w) = (
            plain(&column(true), 1, 0, [0, 0]),
            plain(&column(false), 1, 0, [0, 0]),
        );
        // one top and 4 sides * 4 cells, nothing between water and waterlogged cells
        assert_eq!(m.water.faces(), 1 + 16);
        // the same water bytes as plain water: faces, top quad, tint and depth
        assert_eq!(m.water.bytes(), w.water.bytes());
        let top = m.water.bytes().chunks(64).filter(|q| {
            q.chunks(16)
                .all(|v| i16::from_le_bytes([v[2], v[3]]) == 45 * 16)
        });
        assert_eq!(top.count(), 1);
        let max = m.water.bytes().chunks(16).map(|v| v[15]).max().unwrap();
        assert_eq!(max, 4);
        // four cross quads in each waterlogged cell, at y 42 and 44
        assert_eq!((m.cutout.faces(), w.cutout.faces()), (8, 0));
        let mut ys: Vec<i16> = m
            .cutout
            .bytes()
            .chunks(16)
            .map(|v| i16::from_le_bytes([v[2], v[3]]).div_euclid(16))
            .collect();
        ys.sort();
        ys.dedup();
        assert_eq!(ys, [42, 43, 44, 45]);
        assert_eq!((m.min_y, m.mean_y), (w.min_y, w.mean_y));
    }

    #[test]
    fn kelp_draws_as_water_plus_a_plant() {
        use crate::tables::b::{KELP_0, KELP_1};
        // stone to 40, water 41..=44, kelp_0 at 42 and 43 and kelp_1 at 44 in a 3x3 pool
        let pool = |kelp: bool| {
            world(3, 0, move |x, z, y| match y {
                0..=40 => STONE,
                42 | 43 if kelp && x == 1 && z == 1 => KELP_0,
                44 if kelp && x == 1 && z == 1 => KELP_1,
                41..=44 => WATER,
                _ => 0,
            })
        };
        let (m, w) = (
            plain(&pool(true), 3, 0, [0, 0]),
            plain(&pool(false), 3, 0, [0, 0]),
        );
        // no faces between kelp and the water around it
        assert_eq!(m.water.bytes(), w.water.bytes());
        assert_eq!((m.cutout.faces(), w.cutout.faces()), (12, 0));
        let mut ys: Vec<i16> = m
            .cutout
            .bytes()
            .chunks(16)
            .map(|v| i16::from_le_bytes([v[2], v[3]]).div_euclid(16))
            .collect();
        ys.sort();
        ys.dedup();
        assert_eq!(ys, [42, 43, 44, 45]);
    }

    #[test]
    fn shellfish_draw_as_water_plus_a_cluster() {
        use crate::tables::b::{OYSTER, SCALLOP};
        for b in [OYSTER, SCALLOP] {
            // stone to 40, water 41..=44, the shellfish on the floor at 41 and
            // floating at 43 in the middle of a 3x3 pool
            let pool = |logged: bool| {
                world(3, 0, move |x, z, y| match y {
                    0..=40 => STONE,
                    41 | 43 if logged && x == 1 && z == 1 => b,
                    41..=44 => WATER,
                    _ => 0,
                })
            };
            let (m, w) = (
                plain(&pool(true), 3, 0, [0, 0]),
                plain(&pool(false), 3, 0, [0, 0]),
            );
            // no faces between the cluster cells and the water around them
            assert_eq!(m.water.bytes(), w.water.bytes());
            // 3 boxes of 5 faces on stone, of 6 faces in the water
            assert_eq!((m.cutout.faces(), w.cutout.faces()), (15 + 18, 0));
            for v in m.cutout.bytes().chunks(16) {
                let p = |k: usize| i16::from_le_bytes([v[k], v[k + 1]]);
                // inside the middle cell, at most 3/16 above its floor
                assert!((16 + 2..=32 - 2).contains(&p(0)), "{}", p(0));
                assert!((16 + 2..=32 - 2).contains(&p(4)), "{}", p(4));
                let floor = if p(2) >= 43 * 16 { 43 * 16 } else { 41 * 16 };
                assert!((0..=3).contains(&(p(2) - floor)), "{}", p(2));
            }
        }
    }

    #[test]
    fn waterlogged_cells_do_not_depend_on_the_area() {
        for b in [LOGGED, crate::tables::b::OYSTER] {
            logged_cells_do_not_depend_on_the_area(b);
        }
    }

    fn logged_cells_do_not_depend_on_the_area(b: u8) {
        let f = |x: i32, z: i32, y: usize| match y {
            0..=40 => STONE,
            41..=43 if (x * 3 + z * 5).rem_euclid(4) == 0 => b,
            41..=43 => WATER,
            _ => 0,
        };
        // plant vertices in world space (x, y, z, then the other bytes) inside
        // the 2x2 columns from world `keep`
        let plants = |size: usize, o: [i32; 2], keep: [i32; 2]| {
            let m = plain(
                &world(size, 1, |x, z, y| f(x + o[0], z + o[1], y)),
                size,
                1,
                o,
            );
            // the water is all tops at depth 3: no faces between the cells
            let mut tops = m.water.bytes().chunks(16);
            assert!(tops.all(|v| v[2..4] == (44i16 * 16).to_le_bytes() && v[15] == 3));
            let mut v: Vec<Vec<Vec<u8>>> = Vec::new();
            for quad in m.cutout.bytes().chunks(64) {
                let p = |i: usize, k: usize| {
                    i16::from_le_bytes([quad[i * 16 + k], quad[i * 16 + k + 1]]) as i32
                };
                // the plant's corner nearest -x, -z lies inside its own cell
                let min = |k| (0..4).map(|i| p(i, k)).min().unwrap().div_euclid(16);
                let cell = [min(0) + o[0], min(4) + o[1]];
                if (0..2).contains(&(cell[0] - keep[0])) && (0..2).contains(&(cell[1] - keep[1])) {
                    v.push(
                        (0..4)
                            .map(|i| {
                                let mut e = [p(i, 0) + o[0] * 16, p(i, 2), p(i, 4) + o[1] * 16]
                                    .iter()
                                    .flat_map(|c| c.to_le_bytes())
                                    .collect::<Vec<u8>>();
                                e.extend_from_slice(&quad[i * 16 + 6..i * 16 + 16]);
                                e
                            })
                            .collect(),
                    );
                }
            }
            v.sort();
            v
        };
        let small = plants(2, [9, -3], [9, -3]);
        assert!(!small.is_empty());
        assert_eq!(plants(6, [7, -5], [9, -3]), small);
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
            false,
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
    fn floe_ice_meshes_like_ice() {
        let b = world(1, 0, |_, _, y| match y {
            40 => STONE,
            41 => ICE,
            42 => FLOE_ICE,
            _ => 0,
        });
        let m = mesh_blocks(
            &b,
            1,
            0,
            false,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        // the ice/floe pair hides its shared faces: 4 ice sides, 5 floe faces
        assert_eq!(
            (m.ice.faces(), m.opaque.faces(), m.cutout.faces()),
            (9, 6, 0)
        );
        // the column-top scan passes floe ice like ice
        assert_eq!(m.min_y, 40);
    }

    #[test]
    fn snow_on_foliage_sways_with_it() {
        let snow_on = |under: u8, fast: bool| {
            let b = world(1, 0, |_, _, y| match y {
                40 => STONE,
                41 => under,
                42 => SNOW_LAYER,
                _ => 0,
            });
            let l = layers();
            mesh_blocks(
                &b,
                1,
                0,
                fast,
                false,
                &l,
                &Tints::NONE,
                [0, 0],
                &Variants::NONE,
            )
        };
        // stone (6 faces) stays opaque; leaves (5: the stone hides the bottom) and snow (6) cut out
        let m = snow_on(LEAF, false);
        assert_eq!((m.opaque.faces(), m.cutout.faces()), (6, 11));
        assert!(m.cutout.bytes().chunks(16).all(|v| v[14] == LEAF_WEIGHT));
        // a bush sways even with fast leaves
        let m = snow_on(57, true);
        assert_eq!((m.opaque.faces(), m.cutout.faces()), (6, 11));
        // snow on stone, or on fast (still) leaves, stays opaque
        assert_eq!(snow_on(STONE, false).cutout.faces(), 0);
        assert_eq!(snow_on(LEAF, true).cutout.faces(), 0);
    }

    #[test]
    fn ao_flips_the_diagonal_of_a_top_face() {
        // stone at (1, 40, 1) with one stone above-diagonal at (0, 41, 2): the
        // top's corner (0, 1) has AO 1, so its quad starts at that corner
        let b = world(3, 0, |x, z, y| match (x, z, y) {
            (1, 1, 40) | (0, 2, 41) => STONE,
            _ => 0,
        });
        let m = mesh_blocks(
            &b,
            3,
            0,
            false,
            false,
            &layers(),
            &Tints::NONE,
            [0, 0],
            &Variants::NONE,
        );
        let top = m
            .opaque
            .bytes()
            .chunks(64)
            .find(|q| {
                q.chunks(16)
                    .all(|v| i16::from_le_bytes([v[2], v[3]]) == 41 * 16)
            })
            .filter(|q| i16::from_le_bytes([q[0], q[1]]) >= 16)
            .unwrap();
        let ao: Vec<u8> = top.chunks(16).map(|v| v[13]).collect();
        assert_eq!(ao, [1, 0, 0, 0]);
        let first = [0, 4].map(|o| i16::from_le_bytes([top[o], top[o + 1]]) / 16);
        assert_eq!(first, [1, 2]);
    }

    /// Top quads of a 1-block-thick strip of `b` along x from world x = `x0`, greedy.
    fn greedy_tops(b: u8, x0: i32, n: usize) -> usize {
        let blocks = world(n, 0, |_, _, y| if y == 40 { b } else { 0 });
        let m = mesh_blocks(
            &blocks,
            n,
            0,
            false,
            true,
            &layers(),
            &Tints::NONE,
            [x0, 0],
            &Variants::NONE,
        );
        m.opaque
            .bytes()
            .chunks(64)
            .filter(|q| {
                q.chunks(16)
                    .all(|v| i16::from_le_bytes([v[2], v[3]]) == 41 * 16)
            })
            .count()
    }

    #[test]
    fn greedy_merges_equal_variants_only() {
        // dirt has no variants: one top for the whole 4x4 area
        assert_eq!(greedy_tops(crate::tables::b::DIRT, 0, 4), 1);
        // stone varies: two neighbours merge only when their variants match
        let v = |x| variant_at(x, 40, 0);
        let same = (-500..500).find(|&x| v(x) == v(x + 1)).unwrap();
        let diff = (-500..500).find(|&x| v(x) != v(x + 1)).unwrap();
        let pair = |x0| {
            let blocks = world(
                2,
                0,
                |x, z, y| if y == 40 && z == 0 && x < 2 { STONE } else { 0 },
            );
            let m = mesh_blocks(
                &blocks,
                2,
                0,
                false,
                true,
                &layers(),
                &Tints::NONE,
                [x0, 0],
                &Variants::NONE,
            );
            m.opaque
                .bytes()
                .chunks(64)
                .filter(|q| q[2..4] == (41i16 * 16).to_le_bytes())
                .count()
        };
        assert_eq!(pair(same), 1);
        assert_eq!(pair(diff), 2);
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
        // one grass block over stone; the grass block reads the ground grid
        // (the fifth), red in the column data
        let b = world(1, 0, |_, _, y| match y {
            40 => crate::tables::b::GRASS,
            _ => 0,
        });
        let mut data = alloc::vec![0.5f32; 5 * 3 + 5 * 4 * 3];
        data[4 * 3..5 * 3].copy_from_slice(&[1.0, 0.0, 0.25]);
        let col = Tints {
            size: 1,
            data: &data,
            corners: false,
            colors: &[],
        };
        let m = mesh_blocks(
            &b,
            1,
            0,
            false,
            false,
            &layers(),
            &col,
            [0, 0],
            &Variants::NONE,
        );
        assert!(m
            .opaque
            .bytes()
            .chunks(16)
            .all(|v| v[10..13] == [255, 0, 64]));
        // corner ground grid starts after the five column grids and four
        // corner grids: corner (1, 0) is green
        let at = 5 * 3 + 4 * 4 * 3 + 3;
        data[at..at + 3].copy_from_slice(&[0.0, 1.0, 0.0]);
        let cor = Tints {
            size: 1,
            data: &data,
            corners: true,
            colors: &[],
        };
        let m = mesh_blocks(
            &b,
            1,
            0,
            false,
            false,
            &layers(),
            &cor,
            [0, 0],
            &Variants::NONE,
        );
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
