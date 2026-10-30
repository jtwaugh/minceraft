//! Box-shaped blocks of `meshBlocks` in `viewer/mesh.js`: cactus, bushes and
//! snow layers, the face-hiding rules of the blocks drawn as cubes or boxes,
//! and the flat shapes (leaf litter, lily pads).
use crate::tables::{b::*, face_uv, Face, FACES};
use crate::{variant_at, Quads, Tint};

/// Sway weight (vertex byte 14) of cutout leaf and bush vertices.
pub const LEAF_WEIGHT: u8 = 64;
const CACTUS_INSET: f64 = 1.0 / 16.0;
const BUSH_INSET: f64 = 2.0 / 16.0;
const SNOW_H: f64 = 1.0 / 8.0;

const FULL: ([f64; 3], [f64; 3]) = ([0.0; 3], [1.0; 3]);

/// The box a block fills, block-local: `(lo, hi)`.
pub fn bounds(b: u8) -> ([f64; 3], [f64; 3]) {
    match b {
        CACTUS => (
            [CACTUS_INSET, 0.0, CACTUS_INSET],
            [1.0 - CACTUS_INSET, 1.0, 1.0 - CACTUS_INSET],
        ),
        BERRY_BUSH | BUSH | JUNGLE_BUSH => (
            [BUSH_INSET, 0.0, BUSH_INSET],
            [1.0 - BUSH_INSET, 1.0 - BUSH_INSET, 1.0 - BUSH_INSET],
        ),
        SNOW_LAYER => ([0.0; 3], [1.0, SNOW_H, 1.0]),
        _ => FULL,
    }
}

/// Cactus, bushes and snow layers: boxes smaller than a block.
pub fn is_box(b: u8) -> bool {
    matches!(b, CACTUS | BERRY_BUSH | BUSH | JUNGLE_BUSH | SNOW_LAYER)
}

/// Is a box block's face with vertical normal `dy` hidden by neighbour `n`
/// (`solid`: it hides faces)? Inset sides of cacti and bushes always show.
pub fn box_hidden(b: u8, dy: i32, n: u8, solid: bool) -> bool {
    match b {
        CACTUS => (dy > 0 && n == CACTUS) || (dy < 0 && (n == CACTUS || solid)),
        SNOW_LAYER => (dy < 0 && solid) || (dy == 0 && (solid || n == SNOW_LAYER)),
        _ => dy < 0 && solid,
    }
}

/// Height of a flat shape's quad above the block's floor (`SHAPES` in
/// `viewer/mesh.js`), or `None` for blocks that aren't flat shapes.
pub fn flat_lift(b: u8) -> Option<f64> {
    match b {
        LEAF_LITTER | LILY_PAD => Some(1.0 / 16.0),
        _ => None,
    }
}

/// A flat shape's one up-facing quad at block (x, y, z), `lift` above its
/// floor, its UVs turned and mirrored by the world coordinate `w`.
pub fn flat(
    out: &mut Quads,
    [x, y, z]: [usize; 3],
    w: [i32; 3],
    lift: f64,
    layer: u8,
    tint: &Tint,
) {
    let top = &FACES[0];
    let vr = variant_at(w[0], w[1], w[2]);
    let p = top.c.map(|v| [v[0] as f64, lift, v[2] as f64]);
    let uv = p.map(|v| face_uv(top, [v[0], 0.0, v[2]], vr, [1.0; 3]));
    let k = top.s * (0.75 + y as f64 / 256.0);
    out.quad(
        [x as f64, y as f64, z as f64],
        &p,
        &uv,
        layer,
        k,
        tint,
        None,
        None,
        0,
    );
}

/// The corners of face `f` of the box `lo..hi`.
pub fn face_corners(f: &Face, (lo, hi): ([f64; 3], [f64; 3])) -> [[f64; 3]; 4] {
    f.c.map(|c| [0, 1, 2].map(|j| lo[j] + f64::from(c[j]) * (hi[j] - lo[j])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cactus_top_and_bottom_hide_but_sides_show() {
        assert!(box_hidden(CACTUS, 1, CACTUS, false));
        assert!(!box_hidden(CACTUS, 1, STONE, true));
        assert!(box_hidden(CACTUS, -1, CACTUS, false));
        assert!(box_hidden(CACTUS, -1, SAND, true));
        assert!(!box_hidden(CACTUS, 0, STONE, true));
    }

    #[test]
    fn snow_hides_bottom_and_sides_on_solids_and_snow() {
        assert!(box_hidden(SNOW_LAYER, -1, STONE, true));
        assert!(!box_hidden(SNOW_LAYER, -1, SNOW_LAYER, false));
        assert!(box_hidden(SNOW_LAYER, 0, SNOW_LAYER, false));
        assert!(box_hidden(SNOW_LAYER, 0, STONE, true));
        assert!(!box_hidden(SNOW_LAYER, 0, AIR, false));
        assert!(!box_hidden(SNOW_LAYER, 1, STONE, true));
    }

    #[test]
    fn bush_hides_only_the_bottom_on_solids() {
        for b in [BERRY_BUSH, BUSH, JUNGLE_BUSH] {
            assert!(box_hidden(b, -1, STONE, true));
            assert!(!box_hidden(b, -1, AIR, false));
            assert!(!box_hidden(b, 1, STONE, true));
            assert!(!box_hidden(b, 0, STONE, true));
        }
    }

    #[test]
    fn boxes_are_inset_as_in_the_viewer() {
        let (lo, hi) = bounds(BUSH);
        assert_eq!((lo, hi), ([0.125, 0.0, 0.125], [0.875, 0.875, 0.875]));
        assert_eq!(bounds(SNOW_LAYER).1, [1.0, 0.125, 1.0]);
        assert_eq!(bounds(CACTUS).0, [0.0625, 0.0, 0.0625]);
        assert_eq!(bounds(STONE), FULL);
    }

    #[test]
    fn flat_shapes_turn_their_uvs_by_world_coordinate() {
        let quad = |w: [i32; 3]| {
            let mut q = Quads::new();
            flat(&mut q, [0, 40, 0], w, 1.0 / 16.0, 7, &Tint::One([1.0; 3]));
            q.bytes().to_vec()
        };
        // two world blocks with different variants: same positions, different uvs
        let (a, b) = ([3, 40, 9], [4, 40, 9]);
        assert_ne!(variant_at(a[0], a[1], a[2]), variant_at(b[0], b[1], b[2]));
        let (qa, qb) = (quad(a), quad(b));
        let uv = |q: &[u8]| {
            q.chunks(16)
                .map(|v| [v[6], v[7]])
                .collect::<alloc::vec::Vec<_>>()
        };
        let pos = |q: &[u8]| {
            q.chunks(16)
                .map(|v| v[..6].to_vec())
                .collect::<alloc::vec::Vec<_>>()
        };
        assert_ne!(uv(&qa), uv(&qb));
        assert_eq!(pos(&qa), pos(&qb));
        // lifted 1/16 above the floor
        assert!(qa
            .chunks(16)
            .all(|v| i16::from_le_bytes([v[2], v[3]]) == 40 * 16 + 1));
        assert_eq!(flat_lift(LILY_PAD), Some(0.0625));
        assert_eq!(flat_lift(GRASS), None);
    }
}
