//! Parsing and validation of `data/sea_flora.toml`, shared by `build.rs`

//! (feature `toml-io`). `build.rs` includes this file by path, so keep it free
//! of crate-internal dependencies beyond `rule_check`.

use serde::Deserialize;

use crate::rule_check::check_range;

/// A column's height range in cells, inclusive, and the fewest water cells
/// left above its top cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnRule {
    pub height_min: u32,
    pub height_max: u32,
    pub surface_gap_min: u32,
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
    column: Option<ColumnRule>,
    cluster_scale: f64,
    salt: u32,
    chance: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    sea_flora: Vec<RawEntry>,
}

/// One entry with names resolved: `column` is set for a `column` shape,
/// habitats are rule indices.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaFloraRule {
    pub name: String,
    pub column: Option<ColumnRule>,
    pub blocks: Vec<u8>,
    pub habitats: Vec<usize>,
    pub bottom: Vec<u8>,
    pub depth_min: u32,
    pub depth_max: u32,
    pub shore_distance_max: Option<u32>,
    pub cluster_scale: f64,
    pub salt: u32,
    pub chance: f64,
}

/// Each of `names` through `f`, or an error naming the entry.
fn resolve<T>(
    what: &str,
    kind: &str,
    names: &[String],
    f: &dyn Fn(&str) -> Option<T>,
) -> Result<Vec<T>, String> {
    if names.is_empty() {
        return Err(format!("{what}: no {kind}"));
    }
    let id = |n: &String| f(n).ok_or_else(|| format!("{what}: unknown {kind} '{n}'"));
    names.iter().map(id).collect()
}

/// Parse and validate the sea flora file. `block_id` maps a block name to its
/// id and `habitat_index` a habitat id to its position in the rule list.
pub fn parse_sea_flora(
    text: &str,
    block_id: &dyn Fn(&str) -> Option<u8>,
    habitat_index: &dyn Fn(&str) -> Option<usize>,
) -> Result<Vec<SeaFloraRule>, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<SeaFloraRule> = Vec::new();
    for r in raw.sea_flora {
        let what = format!("sea flora '{}'", r.name);
        if let Some(p) = out.iter().find(|p| p.name == r.name || p.salt == r.salt) {
            return Err(format!("{what}: name or salt already used by '{}'", p.name));
        }
        check_range(&what, "depth", Some(r.depth_min), Some(r.depth_max))?;
        let (column, n_blocks) = match (r.shape.as_str(), r.column) {
            ("column", Some(c)) => {
                check_range(&what, "height", Some(c.height_min), Some(c.height_max))?;
                (Some(c), 2)
            }
            ("cluster", None) => (None, 1),
            (s, _) => return Err(format!("{what}: shape '{s}' and column table disagree")),
        };
        let blocks = resolve(&what, "block", &r.blocks, block_id)?;
        if blocks.len() != n_blocks {
            return Err(format!("{what}: a {} takes {n_blocks} block(s)", r.shape));
        }
        if !r.cluster_scale.is_finite() || r.cluster_scale <= 0.0 {
            return Err(format!("{what}: cluster_scale must be > 0"));
        }
        if !(0.0..=1.0).contains(&r.chance) {
            return Err(format!("{what}: chance must be in 0 to 1"));
        }
        out.push(SeaFloraRule {
            name: r.name,
            column,
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
