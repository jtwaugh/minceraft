//! The sea flora entry type. `build.rs` includes this file by path, so keep
//! it free of dependencies (no serde).

/// How an entry grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeaFloraShape {
    /// A stack of cells up from the seabed, `blocks` = `[middle, top]`. The
    /// top leaves at least `surface_gap_min` water cells above it.
    Column {
        height_min: u32,
        height_max: u32,
        surface_gap_min: u32,
    },
    /// One cell on the seabed, `blocks` = `[block]`.
    Cluster,
}

/// One sea flora kind with names resolved. Ranges are inclusive.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaFloraEntry {
    pub name: String,
    pub shape: SeaFloraShape,
    /// Block ids (`Block as u8`).
    pub blocks: Vec<u8>,
    /// Habitat rule indices (`HabitatId.0`).
    pub habitats: Vec<usize>,
    /// Seabed block ids.
    pub bottom: Vec<u8>,
    pub depth_min: u32,
    pub depth_max: u32,
    pub shore_distance_max: Option<u32>,
    /// Blocks per patch noise cell.
    pub cluster_scale: f64,
    pub salt: u32,
    /// Chance per eligible column, 0 to 1.
    pub chance: f64,
}
