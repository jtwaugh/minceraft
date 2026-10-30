//! Cross plants of `meshBlocks` in `viewer/mesh.js`: two crossed quads wound
//! both ways, shifted, sized, mirrored and brightened by the world coordinate.
use crate::tables::b::{FERN, TALL_GRASS};
use crate::tables::UPPER_HALF;
use crate::{js_round, pick_layer, plant_offset, plant_shape, Quads, Tint, Tints, Variants};

/// Darkening of a plant's base vertices.
const PLANT_BASE_DARK: f64 = 0.8;
const X1: [[f64; 3]; 4] = [
    [0.15, 1.0, 0.15],
    [0.85, 1.0, 0.85],
    [0.85, 0.0, 0.85],
    [0.15, 0.0, 0.15],
];
const X2: [[f64; 3]; 4] = [
    [0.85, 1.0, 0.15],
    [0.15, 1.0, 0.85],
    [0.15, 0.0, 0.85],
    [0.85, 0.0, 0.15],
];
/// Sway weights in the vertex order of X1/X2: top, top, base, base.
const XTOP: [u8; 4] = [255, 255, 0, 0];
const XUV: [[f64; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

fn flip<T: Copy>(a: [T; 4]) -> [T; 4] {
    [a[1], a[0], a[3], a[2]]
}

/// The four quads of cross plant `b` at block (x, y, z) of the area whose
/// block (0, 0) is at world `origin`. `side` is the plant's side-face layer.
/// The upper half of a tall plant takes its variation from the block below.
pub fn cross(
    out: &mut Quads,
    b: u8,
    [x, y, z]: [usize; 3],
    origin: [i32; 2],
    side: u8,
    tints: &Tints,
    variants: &Variants,
) {
    let (wx, wz) = (origin[0] + x as i32, origin[1] + z as i32);
    let py = if UPPER_HALF[b as usize] {
        y as i32 - 1
    } else {
        y as i32
    };
    let layer = pick_layer(variants, side, wx, py, wz);
    let k = 0.9 * (0.75 + py as f64 / 256.0);
    let [dx, dz] = plant_offset(wx, py, wz);
    let shape = plant_shape(wx, py, wz);
    let hgt = if b == TALL_GRASS || b == FERN {
        shape.height
    } else {
        1.0
    };
    let uvs = XUV.map(|[u, v]| {
        [
            if shape.mirror { 1.0 - u } else { u },
            if v == 0.0 { 1.0 - hgt } else { v },
        ]
    });
    let flat = tints.block(b, x, z);
    for q in [X1, X2] {
        let t = q.map(|v| match flat {
            None => [1.0; 3],
            Some(f) if !tints.corners => f,
            Some(_) => {
                let o = [v[0], v[2]].map(|c| js_round(c) as usize);
                tints.vertex(b, x, z, o[0], o[1])
            }
        });
        let mut tj = [[0.0; 3]; 4];
        for j in 0..4 {
            let dark = if XTOP[j] != 0 { 1.0 } else { PLANT_BASE_DARK };
            tj[j] = t[j].map(|c| (c * shape.bright).clamp(0.0, 1.0) * dark);
        }
        let qs = q.map(|v| [v[0], v[1] * hgt, v[2]]);
        let at = [x as f64 + dx, y as f64, z as f64 + dz];
        out.quad(
            at,
            &qs,
            &uvs,
            layer,
            k,
            &Tint::Four(tj),
            Some(XTOP),
            None,
            0,
        );
        out.quad(
            at,
            &flip(qs),
            &flip(uvs),
            layer,
            k,
            &Tint::Four(flip(tj)),
            Some(flip(XTOP)),
            None,
            0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::b::{DUNE_GRASS_LOWER, DUNE_GRASS_UPPER};
    use alloc::vec::Vec;

    fn mesh(b: u8, at: [usize; 3], origin: [i32; 2], variants: &Variants) -> Vec<u8> {
        let mut q = Quads::new();
        cross(&mut q, b, at, origin, 10, &Tints::NONE, variants);
        q.bytes().to_vec()
    }

    #[test]
    fn upper_half_shares_the_lower_blocks_variant_and_offset() {
        let mut table = alloc::vec![0.0; 256 * 9];
        table[10 * 9..11 * 9].copy_from_slice(&[4.0, 0.25, 0.5, 0.75, 1.0, 0., 0., 0., 0.]);
        let v = Variants(&table);
        for (x, z) in [(0, 0), (3, 1), (7, 5)] {
            let origin = [-37, 211];
            let lower = mesh(DUNE_GRASS_LOWER, [x, 40, z], origin, &v);
            let upper = mesh(DUNE_GRASS_UPPER, [x, 41, z], origin, &v);
            assert_eq!(lower.len(), 4 * 4 * 16);
            for (l, u) in lower.chunks(16).zip(upper.chunks(16)) {
                let y = |v: &[u8]| i16::from_le_bytes([v[2], v[3]]);
                // one block higher, and otherwise the same: offset, uv, layer, shade, tint
                assert_eq!(y(u), y(l) + 16);
                assert_eq!((&u[..2], &u[4..]), (&l[..2], &l[4..]));
            }
        }
        // a different block below gives a different offset or layer somewhere
        let a = mesh(DUNE_GRASS_UPPER, [2, 41, 2], [0, 0], &v);
        let b = mesh(DUNE_GRASS_UPPER, [2, 42, 2], [0, 0], &v);
        assert_ne!(
            a.chunks(16).map(|v| (v[0], v[4], v[8])).collect::<Vec<_>>(),
            b.chunks(16).map(|v| (v[0], v[4], v[8])).collect::<Vec<_>>()
        );
    }

    #[test]
    fn tops_sway_and_bases_darken() {
        let q = mesh(DUNE_GRASS_LOWER, [0, 40, 0], [0, 0], &Variants::NONE);
        for v in q.chunks(16) {
            let top = v[14] == 255;
            assert!(top || v[14] == 0);
            // untinted white, brightened at most 6%, base at 0.8 of that
            let c = v[10] as f64 / 255.0;
            assert!(if top { c > 0.93 } else { c < 0.81 && c > 0.74 }, "{c}");
        }
    }
}
