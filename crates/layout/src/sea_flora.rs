//! Sea flora entries (docs/design/sea-flora-and-shellfish.md sections 3
//! and 4): kelp columns and oyster and scallop clusters, with the habitats,
//! seabed blocks and depths each grows in, from `data/sea_flora.toml`.
//! Nothing places them yet. Loaded at run time (feature `toml-io`).

use crate::habitat::{HabitatId, Habitats};

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
    pub habitats: Vec<HabitatId>,
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

/// The entries, in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeaFlora {
    pub entries: Vec<SeaFloraEntry>,
}

impl SeaFlora {
    /// Parse a sea flora file; habitat names are resolved against `habitats`.
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let entries = crate::sea_flora_rules::parse_sea_flora(text, habitats)?;
        Ok(SeaFlora { entries })
    }

    /// The shipped `data/sea_flora.toml`.
    pub fn shipped() -> Self {
        let text = include_str!("../../../data/sea_flora.toml");
        Self::from_toml(text, &Habitats::shipped()).expect("data/sea_flora.toml")
    }

    pub fn get(&self, name: &str) -> Option<&SeaFloraEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}
