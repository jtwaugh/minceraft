//! Navigability map (docs/design/world-layout.md 6.2).
//!
//! Measured on **decorated** terrain: any non-air block from `SEA_LEVEL` to
//! `SEA_LEVEL + navigable_clearance - 1` blocks, leaves, plants and snow
//! included (Decided in #630). Frozen water is never navigable.

use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

use crate::{DerivedMaps, Thresholds};

/// Per-column navigable flags, row order `z * width + x` like the region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavMap {
    pub width: u32,
    pub depth: u32,
    pub navigable: Vec<bool>,
}

impl NavMap {
    pub fn from_region(region: &Region, t: &Thresholds) -> NavMap {
        let maps = DerivedMaps::from_region(region);
        let lo = SEA_LEVEL as usize;
        let hi = (lo + t.navigable_clearance.max(0) as usize).min(HEIGHT);
        let navigable = region
            .blocks
            .chunks_exact(HEIGHT)
            .enumerate()
            .map(|(i, col)| {
                !maps.frozen[i]
                    && maps.water_depth[i] as u32 >= t.min_navigable_depth
                    && col[lo..hi].iter().all(|&b| b == Block::Air as u8)
            })
            .collect();
        NavMap {
            width: region.width,
            depth: region.depth,
            navigable,
        }
    }

    /// Flag at local column `(x, z)`.
    pub fn at(&self, x: u32, z: u32) -> bool {
        self.navigable[(z * self.width + x) as usize]
    }
}
