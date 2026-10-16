//! Parsing and validation of `data/habitats.toml`, shared by `build.rs`
//! (which bakes the shipped table into the crate) and `Habitats::from_toml`.
//! `build.rs` includes this file by path, so keep it free of crate-internal
//! dependencies.

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: String,
    frozen: Option<bool>,
    temp_min: Option<f64>,
    temp_max: Option<f64>,
    depth_min: Option<u32>,
    depth_max: Option<u32>,
    shore_distance_min: Option<u32>,
    shore_distance_max: Option<u32>,
    bottom: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawFile {
    habitat: Vec<RawRule>,
}

/// One rule with block names resolved to ids.
#[derive(Clone, Debug, PartialEq)]
pub struct HabitatRule {
    pub id: String,
    pub frozen: Option<bool>,
    pub temp_min: Option<f64>,
    pub temp_max: Option<f64>,
    pub depth_min: Option<u32>,
    pub depth_max: Option<u32>,
    pub shore_distance_min: Option<u32>,
    pub shore_distance_max: Option<u32>,
    pub bottom: Option<Vec<u8>>,
}

impl HabitatRule {
    fn matches_everything(&self) -> bool {
        self.frozen.is_none()
            && self.temp_min.is_none()
            && self.temp_max.is_none()
            && self.depth_min.is_none()
            && self.depth_max.is_none()
            && self.shore_distance_min.is_none()
            && self.shore_distance_max.is_none()
            && self.bottom.is_none()
    }
}

/// Parse and validate the rule file. `block_id` maps a block name to its id.
pub fn parse_rules(
    text: &str,
    block_id: &dyn Fn(&str) -> Option<u8>,
) -> Result<Vec<HabitatRule>, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut rules: Vec<HabitatRule> = Vec::new();
    for r in raw.habitat {
        if rules.iter().any(|p| p.id == r.id) {
            return Err(format!("duplicate habitat id '{}'", r.id));
        }
        let bottom = match r.bottom {
            None => None,
            Some(names) => {
                let mut ids = Vec::new();
                for n in &names {
                    let id = block_id(n)
                        .ok_or_else(|| format!("habitat '{}': unknown block '{}'", r.id, n))?;
                    ids.push(id);
                }
                Some(ids)
            }
        };
        rules.push(HabitatRule {
            id: r.id,
            frozen: r.frozen,
            temp_min: r.temp_min,
            temp_max: r.temp_max,
            depth_min: r.depth_min,
            depth_max: r.depth_max,
            shore_distance_min: r.shore_distance_min,
            shore_distance_max: r.shore_distance_max,
            bottom,
        });
    }
    if !rules.last().is_some_and(HabitatRule::matches_everything) {
        return Err("last habitat rule must set no conditions (catch-all)".into());
    }
    Ok(rules)
}
