//! Crops loaded from `data/crops.toml`: stage count and harvest yield.
//! See docs/design/basic-gameplay.md sections 9.2 and 9.4.
//!
//! Items named must exist in `data/items.toml`; `Crops::check_items`
//! verifies that.

use crate::items::Items;
use std::fmt;

const DEFAULT_CROPS: &str = include_str!("../../../data/crops.toml");

/// An inclusive count range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CropDef {
    pub name: String,
    /// Growth stages; the crop is mature at the last one.
    pub stages: u32,
    /// Produce item dropped on harvest.
    pub item: String,
    pub item_count: Range,
    /// Seeds dropped on harvest, of the item named by `Crops::seeds_item`.
    pub seeds: Range,
}

/// The crops, sorted by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crops {
    pub seeds_item: String,
    pub crops: Vec<CropDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CropsError {
    /// Malformed TOML, including a crop defined twice.
    Parse(String),
    BadEntry {
        crop: String,
        problem: String,
    },
    /// A crop's produce or the seeds item is not defined in `items.toml`.
    UnknownItem {
        crop: String,
        item: String,
    },
}

impl fmt::Display for CropsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CropsError::Parse(e) => write!(f, "crops.toml: {e}"),
            CropsError::BadEntry { crop, problem } => write!(f, "crops.toml: {crop}: {problem}"),
            CropsError::UnknownItem { crop, item } => {
                write!(f, "crops.toml: {crop}: `{item}` is not in items.toml")
            }
        }
    }
}

impl std::error::Error for CropsError {}

fn range(
    table: &toml::value::Table,
    bad: &dyn Fn(String) -> CropsError,
    what: &str,
    extra: &[&str],
) -> Result<Range, CropsError> {
    if let Some(f) = table
        .keys()
        .find(|f| !["min", "max"].contains(&f.as_str()) && !extra.contains(&f.as_str()))
    {
        return Err(bad(format!("{what}: unknown field `{f}`")));
    }
    let count = |field: &str| {
        table
            .get(field)
            .and_then(|v| v.as_integer())
            .filter(|c| (0..=i64::from(u32::MAX)).contains(c))
            .map(|c| c as u32)
            .ok_or_else(|| bad(format!("{what}: `{field}` must be a non-negative integer")))
    };
    let (min, max) = (count("min")?, count("max")?);
    if min > max {
        return Err(bad(format!("{what}: `min` is greater than `max`")));
    }
    Ok(Range { min, max })
}

fn parse_crop(name: &str, value: &toml::Value) -> Result<CropDef, CropsError> {
    let bad = |problem: String| CropsError::BadEntry {
        crop: name.to_string(),
        problem,
    };
    let table = value
        .as_table()
        .ok_or_else(|| bad("not a table".to_string()))?;
    for field in table.keys() {
        if !["stages", "yield", "seeds"].contains(&field.as_str()) {
            return Err(bad(format!("unknown field `{field}`")));
        }
    }
    let stages = table
        .get("stages")
        .and_then(|v| v.as_integer())
        .filter(|s| (1..=i64::from(u32::MAX)).contains(s))
        .ok_or_else(|| bad("`stages` must be a positive integer".to_string()))?
        as u32;
    let produce = table
        .get("yield")
        .and_then(|v| v.as_table())
        .ok_or_else(|| bad("`yield` must be a table".to_string()))?;
    let item = produce
        .get("item")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("yield: `item` must be a non-empty string".to_string()))?
        .to_string();
    let item_count = range(produce, &bad, "yield", &["item"])?;
    let seeds_table = table
        .get("seeds")
        .and_then(|v| v.as_table())
        .ok_or_else(|| bad("`seeds` must be a table".to_string()))?;
    let seeds = range(seeds_table, &bad, "seeds", &[])?;
    Ok(CropDef {
        name: name.to_string(),
        stages,
        item,
        item_count,
        seeds,
    })
}

impl Crops {
    /// The crops shipped in `data/crops.toml`.
    pub fn default_crops() -> Crops {
        let crops = Crops::from_toml(DEFAULT_CROPS).expect("data/crops.toml is valid");
        crops
            .check_items(&Items::default_items())
            .expect("data/crops.toml items are in data/items.toml");
        crops
    }

    pub fn from_toml(text: &str) -> Result<Crops, CropsError> {
        let mut table: toml::value::Table =
            toml::from_str(text).map_err(|e| CropsError::Parse(e.to_string()))?;
        let seeds_item = match table.remove("seeds_item") {
            Some(toml::Value::String(s)) if !s.is_empty() => s,
            _ => {
                return Err(CropsError::BadEntry {
                    crop: "seeds_item".to_string(),
                    problem: "must be a non-empty string".to_string(),
                })
            }
        };
        let mut crops = Vec::new();
        for (name, value) in &table {
            crops.push(parse_crop(name, value)?);
        }
        Ok(Crops { seeds_item, crops })
    }

    pub fn get(&self, name: &str) -> Option<&CropDef> {
        self.crops.iter().find(|c| c.name == name)
    }

    /// The seeds item and every crop's produce must be defined in `items`.
    pub fn check_items(&self, items: &Items) -> Result<(), CropsError> {
        if !items.has_item(&self.seeds_item) {
            return Err(CropsError::UnknownItem {
                crop: "seeds_item".to_string(),
                item: self.seeds_item.clone(),
            });
        }
        for c in &self.crops {
            if !items.has_item(&c.item) {
                return Err(CropsError::UnknownItem {
                    crop: c.name.clone(),
                    item: c.item.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "seeds_item = 'seeds'\n";
    const YIELD: &str = "yield = { item = 'wheat', min = 1, max = 1 }\n";
    const SEEDS: &str = "seeds = { min = 0, max = 2 }\n";

    #[test]
    fn wheat_yields_match_design() {
        let c = Crops::default_crops();
        assert_eq!(c.crops.len(), 1);
        let w = c.get("wheat").unwrap();
        assert!(w.stages > 0);
        assert_eq!(w.item, "wheat");
        assert_eq!(w.item_count, Range { min: 1, max: 1 });
        assert_eq!(w.seeds, Range { min: 0, max: 2 });
    }

    #[test]
    fn unknown_item_rejected() {
        let c = Crops::from_toml(&format!(
            "{HEAD}[gold]\nstages = 2\nyield = {{ item = 'gold', min = 1, max = 1 }}\n{SEEDS}"
        ))
        .unwrap();
        assert!(matches!(
            c.check_items(&Items::default_items()),
            Err(CropsError::UnknownItem { .. })
        ));
        let c = Crops::from_toml("seeds_item = 'nope'\n").unwrap();
        assert!(matches!(
            c.check_items(&Items::default_items()),
            Err(CropsError::UnknownItem { .. })
        ));
    }

    #[test]
    fn zero_stages_rejected() {
        let text = format!("{HEAD}[w]\nstages = 0\n{YIELD}{SEEDS}");
        assert!(matches!(
            Crops::from_toml(&text),
            Err(CropsError::BadEntry { .. })
        ));
    }

    #[test]
    fn inverted_ranges_rejected() {
        for bad in [
            format!(
                "{HEAD}[w]\nstages = 2\nyield = {{ item = 'wheat', min = 3, max = 2 }}\n{SEEDS}"
            ),
            format!("{HEAD}[w]\nstages = 2\n{YIELD}seeds = {{ min = 3, max = 2 }}\n"),
        ] {
            assert!(matches!(
                Crops::from_toml(&bad),
                Err(CropsError::BadEntry { .. })
            ));
        }
    }

    #[test]
    fn rejects_bad_entries() {
        for bad in [
            format!("{HEAD}[w]\nstages = -1\n{YIELD}{SEEDS}"),
            format!("{HEAD}[w]\n{YIELD}{SEEDS}"),
            format!("{HEAD}[w]\nstages = 2\n{SEEDS}"),
            format!("{HEAD}[w]\nstages = 2\n{YIELD}"),
            format!("{HEAD}[w]\nstages = 2\n{YIELD}seeds = {{ min = -1, max = 2 }}\n"),
            format!("{HEAD}[w]\nstages = 2\nyield = {{ min = 1, max = 1 }}\n{SEEDS}"),
            format!("{HEAD}[w]\nstages = 2\n{YIELD}{SEEDS}extra = 1\n"),
            format!("{HEAD}w = 3\n"),
            format!("{HEAD}[w]\nstages = 2\n{YIELD}{SEEDS}[w]\nstages = 3\n"),
            format!("[w]\nstages = 2\n{YIELD}{SEEDS}"),
            "not toml [".to_string(),
        ] {
            assert!(Crops::from_toml(&bad).is_err(), "{bad}");
        }
    }
}
