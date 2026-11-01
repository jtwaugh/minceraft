//! Parsing and validation of `data/sea_flora.toml`, shared by `build.rs`
//! (which bakes the shipped table into the crate) and `SeaFlora::from_toml`
//! (feature `toml-io`). `build.rs` includes this file by path, so keep it
//! free of crate-internal dependencies beyond `rule_check`.

use serde::Deserialize;

use crate::rule_check::check_range;

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
    height_min: Option<u32>,
    height_max: Option<u32>,
    surface_gap_min: Option<u32>,
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

/// One entry with names resolved: blocks to block ids, habitats to rule
/// indices. `column` is `(height_min, height_max, surface_gap_min)` for a
/// `column` entry and `None` for a `cluster`.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaFloraRule {
    pub name: String,
    pub blocks: Vec<u8>,
    pub column: Option<(u32, u32, u32)>,
    pub habitats: Vec<usize>,
    pub bottom: Vec<u8>,
    pub depth_min: u32,
    pub depth_max: u32,
    pub shore_distance_max: Option<u32>,
    pub cluster_scale: f64,
    pub salt: u32,
    pub chance: f64,
}

fn resolve<T>(
    what: &str,
    kind: &str,
    names: &[String],
    f: &dyn Fn(&str) -> Option<T>,
) -> Result<Vec<T>, String> {
    if names.is_empty() {
        return Err(format!("{what}: {kind} is empty"));
    }
    names
        .iter()
        .map(|n| f(n).ok_or_else(|| format!("{what}: unknown {kind} '{n}'")))
        .collect()
}

/// Parse and validate the sea flora file.
pub fn parse_sea_flora(
    text: &str,
    block_id: &dyn Fn(&str) -> Option<u8>,
    habitat_index: &dyn Fn(&str) -> Option<usize>,
) -> Result<Vec<SeaFloraRule>, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<SeaFloraRule> = Vec::new();
    for r in raw.sea_flora {
        let what = format!("sea flora '{}'", r.name);
        if out.iter().any(|p| p.name == r.name) {
            return Err(format!("duplicate sea flora name '{}'", r.name));
        }
        if let Some(p) = out.iter().find(|p| p.salt == r.salt) {
            return Err(format!(
                "{what}: salt {} already used by '{}'",
                r.salt, p.name
            ));
        }
        let blocks = resolve(&what, "block", &r.blocks, block_id)?;
        let habitats = resolve(&what, "habitat", &r.habitats, habitat_index)?;
        let bottom = resolve(&what, "bottom block", &r.bottom, block_id)?;
        check_range(&what, "depth", Some(r.depth_min), Some(r.depth_max))?;
        check_range(&what, "height", r.height_min, r.height_max)?;
        let column_fields = (r.height_min, r.height_max, r.surface_gap_min);
        let (column, n_blocks) = match (r.shape.as_str(), column_fields) {
            ("column", (Some(lo), Some(hi), Some(gap))) => (Some((lo, hi, gap)), 2),
            ("column", _) => {
                return Err(format!(
                    "{what}: a column needs height_min, height_max and surface_gap_min"
                ))
            }
            ("cluster", (None, None, None)) => (None, 1),
            ("cluster", _) => return Err(format!("{what}: a cluster takes no height fields")),
            (s, _) => return Err(format!("{what}: unknown shape '{s}'")),
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
        out.push(SeaFloraRule {
            name: r.name,
            blocks,
            column,
            habitats,
            bottom,
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
