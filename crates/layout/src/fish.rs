//! Fish species and the spawn query (docs/design/world-layout.md Phase C).
//!
//! Species come from `data/fish.toml`, parsed by `build.rs`. `species_for` returns the species
//! whose habitat, temperature and depth ranges contain a column, with their
//! weights. Time of day is not used yet (decided in #93).

use crate::habitat::HabitatId;
#[cfg(feature = "toml-io")]
use crate::habitat::Habitats;

/// A species as emitted by `build.rs`; habitats are rule indices.
pub(crate) struct StaticSpecies {
    pub id: &'static str,
    pub weight: u32,
    pub habitats: Option<&'static [usize]>,
    pub temp_min: Option<f64>,
    pub temp_max: Option<f64>,
    pub depth_min: Option<u32>,
    pub depth_max: Option<u32>,
}

include!(concat!(env!("OUT_DIR"), "/fish.rs"));

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
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let rules = crate::fish_rules::parse_species(text, &|n| habitats.id_of(n).map(|h| h.0))?;
        let species = rules
            .into_iter()
            .map(|r| Species {
                id: r.id,
                weight: r.weight,
                habitats: r.habitats.map(|h| h.into_iter().map(HabitatId).collect()),
                temp_min: r.temp_min,
                temp_max: r.temp_max,
                depth_min: r.depth_min,
                depth_max: r.depth_max,
            })
            .collect();
        Ok(Fish { species })
    }

    /// The shipped `data/fish.toml`, baked in at build time.
    pub fn shipped() -> Self {
        let species = SHIPPED_SPECIES
            .iter()
            .map(|r| Species {
                id: r.id.to_string(),
                weight: r.weight,
                habitats: r
                    .habitats
                    .map(|h| h.iter().copied().map(HabitatId).collect()),
                temp_min: r.temp_min,
                temp_max: r.temp_max,
                depth_min: r.depth_min,
                depth_max: r.depth_max,
            })
            .collect();
        Fish { species }
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
