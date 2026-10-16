//! Habitat rules and classifier (docs/design/world-layout.md Phase C).
//!
//! Rules come from `data/habitats.toml`, checked in order; the first rule
//! whose set fields all match wins. Bounds are inclusive. The file must end
//! with a rule that sets nothing, so every column gets a habitat.

use serde::Deserialize;
use terrain::BLOCK_INFO;

const DEFAULT_HABITATS: &str = include_str!("../../../data/habitats.toml");

/// Index of a rule in the file (so the order in `data/habitats.toml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HabitatId(pub usize);

/// What the classifier knows about one water column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HabitatInputs {
    pub temperature: f64,
    pub depth: u32,
    /// Block id of the seabed (`Block as u8`).
    pub bottom: u8,
    pub shore_distance: u32,
    pub frozen: bool,
}

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

    pub fn matches(&self, i: &HabitatInputs) -> bool {
        self.frozen.is_none_or(|f| f == i.frozen)
            && self.temp_min.is_none_or(|m| i.temperature >= m)
            && self.temp_max.is_none_or(|m| i.temperature <= m)
            && self.depth_min.is_none_or(|m| i.depth >= m)
            && self.depth_max.is_none_or(|m| i.depth <= m)
            && self
                .shore_distance_min
                .is_none_or(|m| i.shore_distance >= m)
            && self
                .shore_distance_max
                .is_none_or(|m| i.shore_distance <= m)
            && self.bottom.as_ref().is_none_or(|b| b.contains(&i.bottom))
    }
}

/// The ordered rule list.
#[derive(Clone, Debug, PartialEq)]
pub struct Habitats {
    pub rules: Vec<HabitatRule>,
}

impl Habitats {
    pub fn from_toml(text: &str) -> Result<Self, String> {
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
                        let id = BLOCK_INFO
                            .iter()
                            .position(|b| b.name == n)
                            .ok_or_else(|| format!("habitat '{}': unknown block '{}'", r.id, n))?;
                        ids.push(id as u8);
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
        Ok(Habitats { rules })
    }

    /// The shipped `data/habitats.toml`.
    pub fn shipped() -> Self {
        Self::from_toml(DEFAULT_HABITATS).expect("data/habitats.toml is valid")
    }

    /// First matching rule. Pure in `inputs`.
    pub fn classify(&self, inputs: &HabitatInputs) -> HabitatId {
        let i = self
            .rules
            .iter()
            .position(|r| r.matches(inputs))
            .expect("catch-all rule matches");
        HabitatId(i)
    }

    pub fn name(&self, id: HabitatId) -> &str {
        &self.rules[id.0].id
    }

    pub fn id_of(&self, name: &str) -> Option<HabitatId> {
        self.rules.iter().position(|r| r.id == name).map(HabitatId)
    }
}

/// Classify with the shipped rules.
pub fn classify(inputs: &HabitatInputs) -> HabitatId {
    use std::sync::OnceLock;
    static SHIPPED: OnceLock<Habitats> = OnceLock::new();
    SHIPPED.get_or_init(Habitats::shipped).classify(inputs)
}
