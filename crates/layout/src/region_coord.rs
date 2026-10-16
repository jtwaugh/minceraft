//! Region coordinates and region-keyed generation (design: world-layout 2, 4.5).

use crate::{Layout, LayoutRegion};

/// Side of a region in columns. The generator takes any size; 128 is the
/// convention of the layout and streaming code (design: world-layout 2).
pub const REGION_SIZE: u32 = 128;

/// Names one `REGION_SIZE` x `REGION_SIZE` column region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionCoord {
    pub rx: i32,
    pub rz: i32,
}

impl RegionCoord {
    pub const fn new(rx: i32, rz: i32) -> RegionCoord {
        RegionCoord { rx, rz }
    }

    /// The region holding world column `(x, z)`; floor division, so negative
    /// coordinates land in negative regions.
    pub const fn containing(x: i32, z: i32) -> RegionCoord {
        let s = REGION_SIZE as i32;
        RegionCoord {
            rx: x.div_euclid(s),
            rz: z.div_euclid(s),
        }
    }

    /// World coordinates `(x, z)` of the region's lowest column.
    pub const fn origin(self) -> (i32, i32) {
        let s = REGION_SIZE as i32;
        (self.rx * s, self.rz * s)
    }
}

impl Layout {
    /// Generates the region named by `coord`, with its raft cells.
    pub fn generate_layout_region_at(&self, coord: RegionCoord, decorate: bool) -> LayoutRegion {
        let (x0, z0) = coord.origin();
        self.generate_layout_region(x0, z0, REGION_SIZE, REGION_SIZE, decorate)
    }
}
