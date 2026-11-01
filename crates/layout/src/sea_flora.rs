//! Sea flora entries (docs/design/sea-flora-and-shellfish.md sections 3
//! and 4) from `data/sea_flora.toml`, parsed by `build.rs`. Nothing places
//! them yet.

use crate::habitat::HabitatId;
#[cfg(feature = "toml-io")]
use crate::habitat::Habitats;

/// An entry as emitted by `build.rs`; habitats are rule indices.
pub(crate) struct StaticFlora {
    pub name: &'static str,
    pub column: Option<[u32; 3]>,
    pub blocks: &'static [u8],
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

/// A column's height range in cells, inclusive, and the fewest water cells
/// left above its top cell (0: the top may touch the surface).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeaFloraColumn {
    pub height_min: u32,
    pub height_max: u32,
    pub surface_gap_min: u32,
}

/// How an entry grows: a stack up from the seabed (`blocks` = `[middle,
/// top]`) or one cell on it (`blocks` = `[block]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeaFloraShape {
    Column(SeaFloraColumn),
    Cluster,
}

/// One sea flora kind with names resolved. Ranges are inclusive. In the file,
/// `shape = "column"` takes a `column` table, `"cluster"` none; salts are
/// unique.
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

fn shape(column: Option<[u32; 3]>) -> SeaFloraShape {
    match column {
        Some([height_min, height_max, surface_gap_min]) => SeaFloraShape::Column(SeaFloraColumn {
            height_min,
            height_max,
            surface_gap_min,
        }),
        None => SeaFloraShape::Cluster,
    }
}

impl SeaFlora {
    /// Parse a sea flora file; habitat names are resolved against `habitats`.
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let rules = crate::sea_flora_rules::parse_sea_flora(text, &terrain::block_id, &|n| {
            habitats.id_of(n).map(|h| h.0)
        })?;
        Ok(SeaFlora {
            entries: rules
                .into_iter()
                .map(|r| SeaFloraEntry {
                    shape: shape(r.column),
                    habitats: r.habitats.into_iter().map(HabitatId).collect(),
                    name: r.name,
                    blocks: r.blocks,
                    bottom: r.bottom,
                    depth_min: r.depth_min,
                    depth_max: r.depth_max,
                    shore_distance_max: r.shore_distance_max,
                    cluster_scale: r.cluster_scale,
                    salt: r.salt,
                    chance: r.chance,
                })
                .collect(),
        })
    }

    /// The shipped `data/sea_flora.toml`, baked in at build time.
    pub fn shipped() -> Self {
        let entries = SHIPPED_FLORA
            .iter()
            .map(|r| SeaFloraEntry {
                name: r.name.to_string(),
                shape: shape(r.column),
                blocks: r.blocks.to_vec(),
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
