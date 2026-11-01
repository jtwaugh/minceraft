//! Parsing and validation of `data/sea_flora.toml`, shared by `build.rs`
//! (which bakes the shipped table into the crate) and `SeaFlora::from_toml`
//! (feature `toml-io`). `build.rs` includes this file by path, so keep it
//! free of crate-internal dependencies beyond `sea_flora_entry` and
//! `rule_check`.

use serde::Deserialize;

use crate::rule_check::check_range;
use crate::sea_flora_entry::{SeaFloraEntry, SeaFloraShape};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColumn {
    height_min: u32,
    height_max: u32,
    surface_gap_min: u32,
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
    column: Option<RawColumn>,
    cluster_scale: f64,
    salt: u32,
    chance: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    sea_flora: Vec<RawEntry>,
}

fn resolve<T>(
    what: &str,
    kind: &str,
    names: &[String],
    f: &dyn Fn(&str) -> Option<T>,
) -> Result<Vec<T>, String> {
    if names.is_empty() {
        return Err(format!("{what}: no {kind}"));
    }
    names
        .iter()
        .map(|n| f(n).ok_or_else(|| format!("{what}: unknown {kind} '{n}'")))
        .collect()
}

/// Parse and validate the sea flora file. `habitat_index` maps a habitat id
/// to its position in the habitat rule list.
pub fn parse_sea_flora(
    text: &str,
    block_id: &dyn Fn(&str) -> Option<u8>,
    habitat_index: &dyn Fn(&str) -> Option<usize>,
) -> Result<Vec<SeaFloraEntry>, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<SeaFloraEntry> = Vec::new();
    for r in raw.sea_flora {
        let what = format!("sea flora '{}'", r.name);
        if let Some(p) = out.iter().find(|p| p.name == r.name || p.salt == r.salt) {
            return Err(format!("{what}: name or salt already used by '{}'", p.name));
        }
        check_range(&what, "depth", Some(r.depth_min), Some(r.depth_max))?;
        let shape = match (r.shape.as_str(), r.column) {
            ("column", Some(c)) => {
                check_range(&what, "height", Some(c.height_min), Some(c.height_max))?;
                SeaFloraShape::Column {
                    height_min: c.height_min,
                    height_max: c.height_max,
                    surface_gap_min: c.surface_gap_min,
                }
            }
            ("cluster", None) => SeaFloraShape::Cluster,
            (s, _) => {
                return Err(format!(
                    "{what}: shape '{s}' (only a column has a column table)"
                ))
            }
        };
        let blocks = resolve(&what, "block", &r.blocks, block_id)?;
        let n_blocks = if shape == SeaFloraShape::Cluster {
            1
        } else {
            2
        };
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
            habitats: resolve(&what, "habitat", &r.habitats, habitat_index)?,
            bottom: resolve(&what, "bottom block", &r.bottom, block_id)?,
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
