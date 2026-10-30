//! Box-shaped blocks of `meshBlocks` in `viewer/mesh.js`: cactus, bushes and
//! snow layers, and the face-hiding rules of the blocks drawn as cubes or boxes.
use crate::tables::{b::*, Face};

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
}
