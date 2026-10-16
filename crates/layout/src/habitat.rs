//! Habitat rules and classifier (docs/design/world-layout.md Phase C).
//!
//! Rules come from `data/habitats.toml`, parsed and validated by `build.rs`,
//! and are checked in order; the first rule whose set fields all match wins.
//! Bounds are inclusive. The file must end with a rule that sets nothing, so
//! every column gets a habitat.

pub use crate::habitat_rule::HabitatRule;

/// A rule as emitted by `build.rs`.
pub(crate) struct StaticRule {
    pub id: &'static str,
    pub frozen: Option<bool>,
    pub temp_min: Option<f64>,
    pub temp_max: Option<f64>,
    pub depth_min: Option<u32>,
    pub depth_max: Option<u32>,
    pub shore_distance_min: Option<u32>,
    pub shore_distance_max: Option<u32>,
    pub bottom: Option<&'static [u8]>,
}

include!(concat!(env!("OUT_DIR"), "/habitats.rs"));

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

impl HabitatRule {
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
    /// Parse and validate a rule file. For tests and tools: the shipped
    /// rules are baked in at build time and never go through this.
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let rules = crate::habitat_rules::parse_rules(text, &|n| {
            terrain::BLOCK_INFO
                .iter()
                .position(|b| b.name == n)
                .map(|i| i as u8)
        })?;
        Ok(Habitats { rules })
    }

    /// The shipped `data/habitats.toml`.
    pub fn shipped() -> Self {
        let rules = SHIPPED_RULES
            .iter()
            .map(|r| HabitatRule {
                id: r.id.to_string(),
                frozen: r.frozen,
                temp_min: r.temp_min,
                temp_max: r.temp_max,
                depth_min: r.depth_min,
                depth_max: r.depth_max,
                shore_distance_min: r.shore_distance_min,
                shore_distance_max: r.shore_distance_max,
                bottom: r.bottom.map(<[u8]>::to_vec),
            })
            .collect();
        Habitats { rules }
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
