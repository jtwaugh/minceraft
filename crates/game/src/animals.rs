//! Animal kinds loaded from `data/animals.toml`: health and drops.
//! See docs/design/basic-gameplay.md section 8.3.
//!
//! Drop items must exist in `data/items.toml`; `Animals::check_items`
//! verifies that.

use crate::items::Items;
use std::fmt;

const DEFAULT_ANIMALS: &str = include_str!("../../../data/animals.toml");

/// One drop: between `min` and `max` (inclusive) of `item`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drop {
    pub item: String,
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimalDef {
    pub kind: String,
    pub health: u32,
    /// Box width (x and z) and height (y), in blocks.
    pub box_size: [f64; 2],
    pub drops: Vec<Drop>,
}

/// The animal kinds, sorted by name.
#[derive(Clone, Debug, PartialEq)]
pub struct Animals {
    pub kinds: Vec<AnimalDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimalsError {
    /// Malformed TOML, including a kind defined twice.
    Parse(String),
    BadEntry {
        kind: String,
        problem: String,
    },
    /// A drop names an item that `items.toml` does not define.
    UnknownItem {
        kind: String,
        item: String,
    },
}

impl fmt::Display for AnimalsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimalsError::Parse(e) => write!(f, "animals.toml: {e}"),
            AnimalsError::BadEntry { kind, problem } => {
                write!(f, "animals.toml: {kind}: {problem}")
            }
            AnimalsError::UnknownItem { kind, item } => {
                write!(
                    f,
                    "animals.toml: {kind}: drop `{item}` is not in items.toml"
                )
            }
        }
    }
}

impl std::error::Error for AnimalsError {}

fn parse_kind(kind: &str, value: &toml::Value) -> Result<AnimalDef, AnimalsError> {
    let bad = |problem: String| AnimalsError::BadEntry {
        kind: kind.to_string(),
        problem,
    };
    let table = value
        .as_table()
        .ok_or_else(|| bad("not a table".to_string()))?;
    for field in table.keys() {
        if !["health", "drops", "box"].contains(&field.as_str()) {
            return Err(bad(format!("unknown field `{field}`")));
        }
    }
    let health = table
        .get("health")
        .and_then(|v| v.as_integer())
        .filter(|h| (1..=i64::from(u32::MAX)).contains(h))
        .ok_or_else(|| bad("`health` must be a positive integer".to_string()))?
        as u32;
    let box_size = table
        .get("box")
        .and_then(|v| v.as_array())
        .filter(|a| a.len() == 2)
        .and_then(|a| {
            let n = |v: &toml::Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
            Some([n(&a[0])?, n(&a[1])?])
        })
        .filter(|b| b.iter().all(|c| c.is_finite() && *c > 0.0))
        .ok_or_else(|| bad("`box` must be [width, height], both above zero".to_string()))?;
    let list = table
        .get("drops")
        .and_then(|v| v.as_array())
        .ok_or_else(|| bad("`drops` must be an array".to_string()))?;
    let mut drops = Vec::new();
    for (i, entry) in list.iter().enumerate() {
        let n = i + 1;
        let entry = entry
            .as_table()
            .ok_or_else(|| bad(format!("drop {n}: not a table")))?;
        for field in entry.keys() {
            if !["item", "min", "max"].contains(&field.as_str()) {
                return Err(bad(format!("drop {n}: unknown field `{field}`")));
            }
        }
        let item = entry
            .get("item")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| bad(format!("drop {n}: `item` must be a non-empty string")))?
            .to_string();
        let count = |field: &str| {
            entry
                .get(field)
                .and_then(|v| v.as_integer())
                .filter(|c| (0..=i64::from(u32::MAX)).contains(c))
                .map(|c| c as u32)
                .ok_or_else(|| {
                    bad(format!(
                        "drop {n}: `{field}` must be a non-negative integer"
                    ))
                })
        };
        let (min, max) = (count("min")?, count("max")?);
        if min > max {
            return Err(bad(format!("drop {n}: `min` is greater than `max`")));
        }
        drops.push(Drop { item, min, max });
    }
    Ok(AnimalDef {
        kind: kind.to_string(),
        health,
        box_size,
        drops,
    })
}

impl Animals {
    /// The kinds shipped in `data/animals.toml`.
    pub fn default_animals() -> Animals {
        let animals = Animals::from_toml(DEFAULT_ANIMALS).expect("data/animals.toml is valid");
        animals
            .check_items(&Items::default_items())
            .expect("data/animals.toml drops are in data/items.toml");
        animals
    }

    pub fn from_toml(text: &str) -> Result<Animals, AnimalsError> {
        let table: toml::value::Table =
            toml::from_str(text).map_err(|e| AnimalsError::Parse(e.to_string()))?;
        let mut kinds = Vec::new();
        for (kind, value) in &table {
            kinds.push(parse_kind(kind, value)?);
        }
        Ok(Animals { kinds })
    }

    pub fn get(&self, kind: &str) -> Option<&AnimalDef> {
        self.kinds.iter().find(|a| a.kind == kind)
    }

    /// Every drop must name an item defined in `items`.
    pub fn check_items(&self, items: &Items) -> Result<(), AnimalsError> {
        for a in &self.kinds {
            for d in &a.drops {
                if !items.has_item(&d.item) {
                    return Err(AnimalsError::UnknownItem {
                        kind: a.kind.clone(),
                        item: d.item.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drops(a: &Animals, kind: &str) -> Vec<(String, u32, u32)> {
        a.get(kind)
            .unwrap()
            .drops
            .iter()
            .map(|d| (d.item.clone(), d.min, d.max))
            .collect()
    }

    #[test]
    fn defaults_match_design() {
        let a = Animals::default_animals();
        assert_eq!(a.kinds.len(), 3);
        assert_eq!(a.get("cow").unwrap().health, 10);
        assert_eq!(a.get("pig").unwrap().health, 10);
        assert_eq!(a.get("chicken").unwrap().health, 4);
        assert_eq!(
            drops(&a, "cow"),
            [("raw_beef".into(), 1, 3), ("leather".into(), 0, 1)]
        );
        assert_eq!(drops(&a, "pig"), [("raw_pork".into(), 1, 3)]);
        assert_eq!(
            drops(&a, "chicken"),
            [("raw_chicken".into(), 1, 1), ("feather".into(), 0, 2)]
        );
    }

    #[test]
    fn unknown_drop_item_rejected() {
        let a = Animals::from_toml(
            "[cow]\nhealth = 1\nbox = [1, 1]\ndrops = [{ item = 'gold', min = 1, max = 1 }]\n",
        )
        .unwrap();
        assert!(matches!(
            a.check_items(&Items::default_items()),
            Err(AnimalsError::UnknownItem { .. })
        ));
    }

    #[test]
    fn rejects_bad_entries() {
        for bad in [
            "[cow]\nhealth = 1\nbox = [1, 1]\ndrops = []\nspeed = 2\n",
            "[cow]\nhealth = 1\ndrops = []\n",
            "[cow]\nhealth = 1\nbox = [0, 1]\ndrops = []\n",
            "[cow]\nhealth = 1\nbox = [1]\ndrops = []\n",
            "[cow]\nhealth = 1\nbox = [1, 1]\ndrops = [{ item = 'a', min = 1, max = 1, extra = 1 }]\n",
            "[cow]\nhealth = 1\nbox = [1, 1]\ndrops = [{ item = 'a', min = 3, max = 2 }]\n",
            "[cow]\nhealth = 0\ndrops = []\n",
            "[cow]\nhealth = -4\ndrops = []\n",
            "[cow]\ndrops = []\n",
            "[cow]\nhealth = 1\n",
            "[cow]\nhealth = 1\nbox = [1, 1]\ndrops = [{ item = 'a', min = -1, max = 1 }]\n",
            "[cow]\nhealth = 1\nbox = [1, 1]\ndrops = [{ min = 1, max = 1 }]\n",
            "cow = 3\n",
            "[cow]\nhealth = 1\ndrops = []\n[cow]\nhealth = 2\ndrops = []\n",
            "not toml [",
        ] {
            assert!(Animals::from_toml(bad).is_err(), "{bad}");
        }
    }
}
