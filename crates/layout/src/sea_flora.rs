//! Sea flora entries (docs/design/sea-flora-and-shellfish.md sections 3
//! and 4).
//!
//! Entries come from `data/sea_flora.toml`, parsed and validated by
//! `build.rs`: kelp columns and oyster and scallop clusters, with the
//! habitats, seabed blocks and depths each may grow in. Nothing places them
//! yet; the placement pass reads `SeaFlora::shipped`.

use crate::habitat::HabitatId;
#[cfg(feature = "toml-io")]
use crate::habitat::Habitats;

/// An entry as emitted by `build.rs`; habitats are rule indices.
pub(crate) struct StaticEntry {
    pub name: &'static str,
    pub blocks: &'static [u8],
    pub column: Option<(u32, u32, u32)>,
    pub habitats: &'static [usize],
    pub bottom: &'static [u8],
    pub depth_min: u32,
    pub depth_max: u32,
    pub shore_distance_max: Option<u32>,
    pub cluster_scale: f64,
    pub salt: u32,
    pub chance: f64,
}

include!(concat!(env!("OUT_DIR"), "/sea_flora.rs"));

/// How an entry grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeaFloraShape {
    /// A stack of cells up from the seabed: `blocks` is `[middle, top]`.
    /// The top leaves at least `surface_gap_min` water cells above it.
    Column {
        height_min: u32,
        height_max: u32,
        surface_gap_min: u32,
    },
    /// One cell on the seabed: `blocks` is `[block]`.
    Cluster,
}

impl SeaFloraShape {
    fn new(column: Option<(u32, u32, u32)>) -> Self {
        match column {
            Some((height_min, height_max, surface_gap_min)) => SeaFloraShape::Column {
                height_min,
                height_max,
                surface_gap_min,
            },
            None => SeaFloraShape::Cluster,
        }
    }
}

/// One sea flora kind. Ranges are inclusive.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaFloraEntry {
    pub name: String,
    /// Block ids (`Block as u8`).
    pub blocks: Vec<u8>,
    pub shape: SeaFloraShape,
    pub habitats: Vec<HabitatId>,
    /// Seabed block ids the entry grows on.
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
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let rules = crate::sea_flora_rules::parse_sea_flora(text, &terrain::block_id, &|n| {
            habitats.id_of(n).map(|h| h.0)
        })?;
        let entries = rules
            .into_iter()
            .map(|r| SeaFloraEntry {
                name: r.name,
                blocks: r.blocks,
                shape: SeaFloraShape::new(r.column),
                habitats: r.habitats.into_iter().map(HabitatId).collect(),
                bottom: r.bottom,
                depth_min: r.depth_min,
                depth_max: r.depth_max,
                shore_distance_max: r.shore_distance_max,
                cluster_scale: r.cluster_scale,
                salt: r.salt,
                chance: r.chance,
            })
            .collect();
        Ok(SeaFlora { entries })
    }

    /// The shipped `data/sea_flora.toml`, baked in at build time.
    pub fn shipped() -> Self {
        let entries = SHIPPED_SEA_FLORA
            .iter()
            .map(|r| SeaFloraEntry {
                name: r.name.to_string(),
                blocks: r.blocks.to_vec(),
                shape: SeaFloraShape::new(r.column),
                habitats: r.habitats.iter().copied().map(HabitatId).collect(),
                bottom: r.bottom.to_vec(),
                depth_min: r.depth_min,
                depth_max: r.depth_max,
                shore_distance_max: r.shore_distance_max,
                cluster_scale: r.cluster_scale,
                salt: r.salt,
                chance: r.chance,
            })
            .collect();
        SeaFlora { entries }
    }

    pub fn get(&self, name: &str) -> Option<&SeaFloraEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}
