//! Parsing and validation of `data/fish.toml`, shared by `build.rs` (which
//! bakes the shipped table into the crate) and `Fish::from_toml` (feature
//! `toml-io`). `build.rs` includes this file by path, so keep it free of
//! crate-internal dependencies.

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpecies {
    id: String,
    weight: u32,
    habitats: Option<Vec<String>>,
    temp_min: Option<f64>,
    temp_max: Option<f64>,
    depth_min: Option<u32>,
    depth_max: Option<u32>,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    species: Vec<RawSpecies>,
}

/// One species with habitat names resolved to rule indices.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeciesRule {
    pub id: String,
    pub weight: u32,
    pub habitats: Option<Vec<usize>>,
    pub temp_min: Option<f64>,
    pub temp_max: Option<f64>,
    pub depth_min: Option<u32>,
    pub depth_max: Option<u32>,
}

/// Parse and validate the species file. `habitat_index` maps a habitat id to
/// its position in the habitat rule list.
pub fn parse_species(
    text: &str,
    habitat_index: &dyn Fn(&str) -> Option<usize>,
) -> Result<Vec<SpeciesRule>, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut species: Vec<SpeciesRule> = Vec::new();
    for r in raw.species {
        if species.iter().any(|p| p.id == r.id) {
            return Err(format!("duplicate species id '{}'", r.id));
        }
        if r.weight == 0 {
            return Err(format!("species '{}': weight must be > 0", r.id));
        }
        if let (Some(a), Some(b)) = (r.temp_min, r.temp_max) {
            if a > b {
                return Err(format!("species '{}': temp_min > temp_max", r.id));
            }
        }
        if let (Some(a), Some(b)) = (r.depth_min, r.depth_max) {
            if a > b {
                return Err(format!("species '{}': depth_min > depth_max", r.id));
            }
        }
        let hab =
            match r.habitats {
                None => None,
                Some(names) => {
                    let mut ids = Vec::new();
                    for n in &names {
                        ids.push(habitat_index(n).ok_or_else(|| {
                            format!("species '{}': unknown habitat '{}'", r.id, n)
                        })?);
                    }
                    Some(ids)
                }
            };
        species.push(SpeciesRule {
            id: r.id,
            weight: r.weight,
            habitats: hab,
            temp_min: r.temp_min,
            temp_max: r.temp_max,
            depth_min: r.depth_min,
            depth_max: r.depth_max,
        });
    }
    Ok(species)
}
