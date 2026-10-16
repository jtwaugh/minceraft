//! Fish species and the spawn query (docs/design/world-layout.md Phase C).
//!
//! Species come from `data/fish.toml`. `species_for` returns the species
//! whose habitat, temperature and depth ranges contain a column, with their
//! weights. Time of day is not used yet (decided in #93).

use crate::habitat::{HabitatId, Habitats};
use serde::Deserialize;

const DEFAULT_FISH: &str = include_str!("../../../data/fish.toml");

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

/// What the query knows about one water column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FishColumn {
    pub habitat: HabitatId,
    pub temperature: f64,
    pub depth: u32,
}

/// One species; `None` fields match anything. Bounds are inclusive.
#[derive(Clone, Debug, PartialEq)]
pub struct Species {
    pub id: String,
    pub weight: u32,
    pub habitats: Option<Vec<HabitatId>>,
    pub temp_min: Option<f64>,
    pub temp_max: Option<f64>,
    pub depth_min: Option<u32>,
    pub depth_max: Option<u32>,
}

impl Species {
    pub fn matches(&self, c: &FishColumn) -> bool {
        self.habitats
            .as_ref()
            .is_none_or(|h| h.contains(&c.habitat))
            && self.temp_min.is_none_or(|m| c.temperature >= m)
            && self.temp_max.is_none_or(|m| c.temperature <= m)
            && self.depth_min.is_none_or(|m| c.depth >= m)
            && self.depth_max.is_none_or(|m| c.depth <= m)
    }
}

/// The species list, in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fish {
    pub species: Vec<Species>,
}

impl Fish {
    /// Parse a fish file; habitat names are resolved against `habitats`.
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut species: Vec<Species> = Vec::new();
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
            let hab = match r.habitats {
                None => None,
                Some(names) => {
                    let mut ids = Vec::new();
                    for n in &names {
                        ids.push(habitats.id_of(n).ok_or_else(|| {
                            format!("species '{}': unknown habitat '{}'", r.id, n)
                        })?);
                    }
                    Some(ids)
                }
            };
            species.push(Species {
                id: r.id,
                weight: r.weight,
                habitats: hab,
                temp_min: r.temp_min,
                temp_max: r.temp_max,
                depth_min: r.depth_min,
                depth_max: r.depth_max,
            });
        }
        Ok(Fish { species })
    }

    /// The shipped `data/fish.toml` against the shipped habitats.
    pub fn shipped() -> Self {
        Self::from_toml(DEFAULT_FISH, &Habitats::shipped()).expect("data/fish.toml is valid")
    }

    /// Species whose ranges contain `column`, as `(species id, weight)` in
    /// file order. Empty if none fit.
    pub fn species_for(&self, column: &FishColumn) -> Vec<(&str, u32)> {
        self.species
            .iter()
            .filter(|s| s.matches(column))
            .map(|s| (s.id.as_str(), s.weight))
            .collect()
    }
}
