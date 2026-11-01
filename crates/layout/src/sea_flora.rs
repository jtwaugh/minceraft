//! Sea flora entries (docs/design/sea-flora-and-shellfish.md sections 3
//! and 4) from `data/sea_flora.toml`, loaded at run time (feature
//! `toml-io`). Nothing places them yet.

use serde::Deserialize;

use crate::habitat::{HabitatId, Habitats};
use crate::rule_check::check_range;

/// A column's height range in cells, inclusive, and the fewest water cells
/// left above its top cell (0: the top may touch the surface).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
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

impl SeaFlora {
    /// Parse a sea flora file; habitat names are resolved against `habitats`.
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let entries = parse_sea_flora(text, habitats)?;
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    name: String,
    shape: String,
    blocks: Vec<String>,
    habitats: Vec<String>,
    bottom: Vec<String>,
    depth_min: u32,
    depth_max: u32,
    shore_distance_max: Option<u32>,
    column: Option<SeaFloraColumn>,
    cluster_scale: f64,
    salt: u32,
    chance: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    sea_flora: Vec<RawEntry>,
}

/// Each of `names` through `f`, or an error naming the entry.
fn resolve<T>(
    what: &str,
    kind: &str,
    names: &[String],
    f: impl Fn(&str) -> Option<T>,
) -> Result<Vec<T>, String> {
    if names.is_empty() {
        return Err(format!("{what}: no {kind}"));
    }
    let id = |n: &String| f(n).ok_or_else(|| format!("{what}: unknown {kind} '{n}'"));
    names.iter().map(id).collect()
}

/// Parse and validate the sea flora file.
fn parse_sea_flora(text: &str, habitats: &Habitats) -> Result<Vec<SeaFloraEntry>, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<SeaFloraEntry> = Vec::new();
    for r in raw.sea_flora {
        let what = format!("sea flora '{}'", r.name);
        if let Some(p) = out.iter().find(|p| p.name == r.name || p.salt == r.salt) {
            return Err(format!("{what}: name or salt already used by '{}'", p.name));
        }
        check_range(&what, "depth", Some(r.depth_min), Some(r.depth_max))?;
        let (shape, n_blocks) = match (r.shape.as_str(), r.column) {
            ("column", Some(c)) => {
                check_range(&what, "height", Some(c.height_min), Some(c.height_max))?;
                (SeaFloraShape::Column(c), 2)
            }
            ("cluster", None) => (SeaFloraShape::Cluster, 1),
            (s, _) => return Err(format!("{what}: shape '{s}' and column table disagree")),
        };
        let blocks = resolve(&what, "block", &r.blocks, terrain::block_id)?;
        if blocks.len() != n_blocks {
            return Err(format!("{what}: a {} takes {n_blocks} block(s)", r.shape));
        }
        if !r.cluster_scale.is_finite() || r.cluster_scale <= 0.0 {
            return Err(format!("{what}: cluster_scale must be > 0"));
        }
        if !(0.0..=1.0).contains(&r.chance) {
            return Err(format!("{what}: chance must be in 0 to 1"));
        }
        out.push(SeaFloraEntry {
            name: r.name,
            shape,
            blocks,
            habitats: resolve(&what, "habitat", &r.habitats, |n| habitats.id_of(n))?,
            bottom: resolve(&what, "bottom block", &r.bottom, terrain::block_id)?,
            depth_min: r.depth_min,
            depth_max: r.depth_max,
            shore_distance_max: r.shore_distance_max,
            cluster_scale: r.cluster_scale,
            salt: r.salt,
            chance: r.chance,
        });
    }
    Ok(out)
}
