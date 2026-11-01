//! Climate tints, ported from `tintFor` and the per-vertex tint of `meshBlocks`
//! in `viewer/mesh.js`.
use crate::tables::b::*;

const G_GRASS: usize = 0;
const G_LEAF: usize = 1;
const G_WATER: usize = 2;
const G_DRY: usize = 3;
/// The grass block's own tint: grass shaded toward dry under leaves
/// (`groundTints` in `viewer/mesh.js`).
const G_GROUND: usize = 4;
const GRIDS: usize = 5;

/// The tint data `mesh_blocks` takes, in the wasm layout: per-column grass,
/// leaf, water, dry and ground tints (`size * size * 3` f32 each, in that
/// order), then, if `corners`, the same five grids per block corner
/// (`(size + 1)^2 * 3` each). `colors` is the block colour table (256 * 3 u8).
/// Anything missing reads as white (1.0) or black (colours).
pub struct Tints<'a> {
    pub size: usize,
    pub data: &'a [f32],
    pub corners: bool,
    pub colors: &'a [u8],
}

impl Tints<'static> {
    pub const NONE: Tints<'static> = Tints {
        size: 0,
        data: &[],
        corners: false,
        colors: &[],
    };
}

impl Tints<'_> {
    fn rgb(&self, at: usize) -> [f64; 3] {
        [0, 1, 2].map(|k| self.data.get(at + k).map_or(1.0, |&v| v as f64))
    }

    fn column(&self, grid: usize, x: usize, z: usize) -> [f64; 3] {
        let n = self.size * self.size;
        self.rgb((grid * n + z * self.size + x) * 3)
    }

    fn corner(&self, grid: usize, x: usize, z: usize) -> [f64; 3] {
        let (n, cn) = (self.size * self.size, self.size + 1);
        self.rgb(GRIDS * n * 3 + (grid * cn * cn + z * cn + x) * 3)
    }

    fn color(&self, b: u8) -> [f64; 3] {
        [0, 1, 2].map(|k| {
            self.colors
                .get(b as usize * 3 + k)
                .map_or(0.0, |&v| v as f64)
                / 255.0
        })
    }

    /// `tintFor` for block `b` from the four tints at one place; `None` is
    /// white for blocks that take no tint.
    fn tint_for(
        &self,
        b: u8,
        grass: [f64; 3],
        leaf: [f64; 3],
        water: [f64; 3],
        dry: [f64; 3],
    ) -> Option<[f64; 3]> {
        let mix = |c: [f64; 3], t: f64| [0, 1, 2].map(|k| leaf[k] + (c[k] - leaf[k]) * t);
        Some(match b {
            GRASS | TALL_GRASS | LARGE_FERN | BROADLEAF => grass,
            TALL_GRASS_DOUBLE_LOWER | TALL_GRASS_DOUBLE_UPPER => grass,
            DRY_GRASS | DRY_GRASS_TALL_LOWER | DRY_GRASS_TALL_UPPER => grass,
            FERN_DOUBLE_LOWER | FERN_DOUBLE_UPPER | CATTAIL_LOWER | CATTAIL_UPPER | FERN => {
                grass.map(|v| v * 0.88)
            }
            WATER => water,
            DUNE_GRASS_LOWER | DUNE_GRASS_UPPER => dry,
            OAK_LEAVES | LEAF_LITTER | LILY_PAD | BERRY_BUSH | BUSH | JUNGLE_BUSH => leaf,
            BIRCH_LEAVES => mix(self.color(b), 0.55),
            SPRUCE_LEAVES => mix(self.color(b), 0.7),
            _ => return None,
        })
    }

    /// Tint of block `b` in column (x, z), or `None` if it is white.
    pub fn block(&self, b: u8, x: usize, z: usize) -> Option<[f64; 3]> {
        let g = |k| self.column(k, x, z);
        self.tint_for(b, g(grass_grid(b)), g(G_LEAF), g(G_WATER), g(G_DRY))
    }

    /// Tint of block `b` at the block corner offset (ox, oz) of column (x, z).
    pub fn vertex(&self, b: u8, x: usize, z: usize, ox: usize, oz: usize) -> [f64; 3] {
        let g = |k| self.corner(k, x + ox, z + oz);
        self.tint_for(b, g(grass_grid(b)), g(G_LEAF), g(G_WATER), g(G_DRY))
            .unwrap_or([1.0; 3])
    }
}

/// The grid block `b` takes its grass tint from: the grass block reads
/// `ground`, every other block `grass`.
fn grass_grid(b: u8) -> usize {
    if b == GRASS {
        G_GROUND
    } else {
        G_GRASS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    const G: [f64; 3] = [0.25, 0.5, 0.75];
    const L: [f64; 3] = [0.5, 0.0, 1.0];
    const W: [f64; 3] = [0.125, 0.25, 0.5];
    const D: [f64; 3] = [1.0, 0.5, 0.0];

    fn colors() -> Vec<u8> {
        let mut c = vec![0u8; 768];
        c[BIRCH_LEAVES as usize * 3..][..3].copy_from_slice(&[255, 0, 51]);
        c[SPRUCE_LEAVES as usize * 3..][..3].copy_from_slice(&[0, 255, 255]);
        c
    }

    #[test]
    fn tint_for_covers_every_block_case() {
        let c = colors();
        let t = Tints {
            colors: &c,
            ..Tints::NONE
        };
        let f = |b| t.tint_for(b, G, L, W, D);
        for b in [
            GRASS,
            TALL_GRASS,
            LARGE_FERN,
            BROADLEAF,
            TALL_GRASS_DOUBLE_LOWER,
            TALL_GRASS_DOUBLE_UPPER,
            DRY_GRASS,
            DRY_GRASS_TALL_LOWER,
            DRY_GRASS_TALL_UPPER,
        ] {
            assert_eq!(f(b), Some(G), "block {b}");
        }
        for b in [
            FERN,
            FERN_DOUBLE_LOWER,
            FERN_DOUBLE_UPPER,
            CATTAIL_LOWER,
            CATTAIL_UPPER,
        ] {
            assert_eq!(f(b), Some(G.map(|v| v * 0.88)), "block {b}");
        }
        assert_eq!(f(WATER), Some(W));
        for b in [DUNE_GRASS_LOWER, DUNE_GRASS_UPPER] {
            assert_eq!(f(b), Some(D));
        }
        for b in [
            OAK_LEAVES,
            LEAF_LITTER,
            LILY_PAD,
            BERRY_BUSH,
            BUSH,
            JUNGLE_BUSH,
        ] {
            assert_eq!(f(b), Some(L), "block {b}");
        }
        assert_eq!(
            f(BIRCH_LEAVES),
            Some([0, 1, 2].map(|k| L[k] + ([1.0, 0.0, 0.2][k] - L[k]) * 0.55))
        );
        assert_eq!(
            f(SPRUCE_LEAVES),
            Some([0, 1, 2].map(|k| L[k] + ([0.0, 1.0, 1.0][k] - L[k]) * 0.7))
        );
        for b in [
            STONE, DIRT, ICE, SAND, OAK_LOG, CACTUS, SNOW_LAYER, DANDELION,
        ] {
            assert_eq!(f(b), None, "block {b}");
        }
    }

    #[test]
    fn column_and_corner_grids_are_laid_out_in_order() {
        // size 2: columns 4 * 3 floats per grid, corners 9 * 3 per grid
        let (n, cn) = (4 * 3, 9 * 3);
        let data: Vec<f32> = (0..5 * n + 5 * cn).map(|i| i as f32).collect();
        let t = Tints {
            size: 2,
            data: &data,
            corners: true,
            colors: &[],
        };
        assert_eq!(t.column(G_GRASS, 1, 1), [9.0, 10.0, 11.0]);
        assert_eq!(
            t.column(G_DRY, 0, 1),
            [(3 * n + 6) as f64, (3 * n + 7) as f64, (3 * n + 8) as f64]
        );
        assert_eq!(t.column(G_GROUND, 0, 0)[0], (4 * n) as f64);
        let o = 5 * n;
        assert_eq!(
            t.corner(G_GRASS, 2, 0),
            [(o + 6) as f64, (o + 7) as f64, (o + 8) as f64]
        );
        assert_eq!(t.corner(G_LEAF, 0, 2)[0], (o + cn + 18) as f64);
        assert_eq!(t.corner(G_GROUND, 0, 0)[0], (o + 4 * cn) as f64);
    }

    #[test]
    fn the_grass_block_reads_ground_and_grass_flora_reads_grass() {
        let (n, cn) = (4 * 3, 9 * 3);
        let data: Vec<f32> = (0..5 * n + 5 * cn).map(|i| i as f32).collect();
        let t = Tints {
            size: 2,
            data: &data,
            corners: true,
            colors: &[],
        };
        assert_eq!(t.block(GRASS, 0, 0).unwrap()[0], (4 * n) as f64);
        assert_eq!(t.block(TALL_GRASS, 0, 0).unwrap()[0], 0.0);
        assert_eq!(t.vertex(GRASS, 0, 0, 0, 0)[0], (5 * n + 4 * cn) as f64);
        assert_eq!(t.vertex(TALL_GRASS, 0, 0, 0, 0)[0], (5 * n) as f64);
    }

    #[test]
    fn vertex_reads_the_corner_at_the_offset() {
        let (n, cn) = (4 * 3, 9 * 3);
        let mut data = vec![0f32; 5 * n + 5 * cn];
        // water grid of the corners, corner (2, 1) = 0.5
        let at = 5 * n + 2 * cn + (3 + 2) * 3;
        data[at..at + 3].copy_from_slice(&[0.5; 3]);
        let t = Tints {
            size: 2,
            data: &data,
            corners: true,
            colors: &[],
        };
        assert_eq!(t.vertex(WATER, 1, 0, 1, 1), [0.5; 3]);
        assert_eq!(t.vertex(WATER, 1, 0, 0, 1), [0.0; 3]);
        assert_eq!(t.vertex(STONE, 1, 0, 1, 1), [1.0; 3]);
    }

    #[test]
    fn missing_data_is_white() {
        assert_eq!(Tints::NONE.block(GRASS, 0, 0), Some([1.0; 3]));
    }
}
