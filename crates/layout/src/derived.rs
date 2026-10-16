//! Derived water maps: depth, bottom block, frozen flag
//! (design: world-layout Phase C, 4.5).

use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

/// Per-column maps derived from a generated region. Every vector has
/// `width * depth` entries in row order `z * width + x`, like the region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedMaps {
    pub width: u32,
    pub depth: u32,
    /// Contiguous water/ice cells counted down from `SEA_LEVEL - 1`; 0 on land.
    pub water_depth: Vec<u8>,
    /// The block under the water run. Land columns, and a run that reaches
    /// y = 0, hold [`NO_BOTTOM`].
    pub bottom_block: Vec<u8>,
    /// True where the block at `SEA_LEVEL - 1` is ice (sea ice, SPEC 13).
    pub frozen: Vec<bool>,
}

/// Sentinel for `bottom_block` on columns without a water run.
pub const NO_BOTTOM: u8 = Block::Air as u8;

fn is_water(b: u8) -> bool {
    b == Block::Water as u8 || b == Block::Ice as u8
}

impl DerivedMaps {
    /// Computes the maps for every column of `region`.
    pub fn from_region(region: &Region) -> DerivedMaps {
        let n = region.width as usize * region.depth as usize;
        let mut maps = DerivedMaps {
            width: region.width,
            depth: region.depth,
            water_depth: vec![0; n],
            bottom_block: vec![NO_BOTTOM; n],
            frozen: vec![false; n],
        };
        let surface = (SEA_LEVEL - 1) as usize;
        for (i, col) in region.blocks.chunks_exact(HEIGHT).enumerate() {
            if !is_water(col[surface]) {
                continue;
            }
            maps.frozen[i] = col[surface] == Block::Ice as u8;
            let run = col[..=surface]
                .iter()
                .rev()
                .take_while(|&&b| is_water(b))
                .count();
            maps.water_depth[i] = run as u8;
            if run <= surface {
                maps.bottom_block[i] = col[surface - run];
            }
        }
        maps
    }

    /// Index of world column `(x, z)`, or `None` outside the region.
    pub fn index_of(&self, region: &Region, x: i32, z: i32) -> Option<usize> {
        let (dx, dz) = (x.checked_sub(region.x0)?, z.checked_sub(region.z0)?);
        (dx >= 0 && dz >= 0 && (dx as u32) < self.width && (dz as u32) < self.depth)
            .then(|| dz as usize * self.width as usize + dx as usize)
    }
}
