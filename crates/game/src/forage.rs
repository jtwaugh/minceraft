//! Foraging tall grass for seeds, from `data/items.toml` `[forage]`.
//! See docs/design/basic-gameplay.md section 9.5.
//!
//! The grass stays; each block has a cooldown, kept here by position.

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::rng::Rng;
use std::collections::HashMap;
use std::fmt;

const DEFAULT_ITEMS: &str = include_str!("../../../data/items.toml");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForageConfig {
    /// Item dropped on success.
    pub item: String,
    /// Success is 1 in `chance`.
    pub chance: u32,
    /// Ticks before a block can be foraged again.
    pub cooldown_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForageError(pub String);

impl From<&ForageError> for DataError {
    fn from(e: &ForageError) -> DataError {
        let kind = DataErrorKind::BadEntry {
            entry: "forage".into(),
            problem: e.0.clone(),
        };
        DataError::new(DataFile::Items, kind)
    }
}

impl fmt::Display for ForageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        DataError::from(self).fmt(f)
    }
}

impl std::error::Error for ForageError {}

impl ForageConfig {
    pub fn default_from_data() -> ForageConfig {
        ForageConfig::parse(DEFAULT_ITEMS).expect("data/items.toml is valid")
    }

    pub fn parse(text: &str) -> Result<ForageConfig, ForageError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| ForageError(e.to_string()))?;
        let t = root
            .get("forage")
            .ok_or_else(|| ForageError("missing [forage]".into()))?;
        let int = |key: &str| {
            t.get(key)
                .and_then(|v| v.as_integer())
                .filter(|n| *n >= 1)
                .ok_or_else(|| ForageError(format!("`{key}` must be a positive integer")))
        };
        let item = t
            .get("item")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ForageError("`item` must be a non-empty string".into()))?;
        Ok(ForageConfig {
            item: item.to_string(),
            chance: u32::try_from(int("chance")?)
                .map_err(|_| ForageError("`chance` is too large".into()))?,
            cooldown_ticks: int("cooldown_ticks")? as u64,
        })
    }
}

/// What a forage did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Forage<'a> {
    /// Inside the block's cooldown: nothing rolled, nothing happens.
    Cooldown,
    /// Rolled and dropped nothing.
    Miss,
    /// Rolled and dropped the named item.
    Drop(&'a str),
}

/// Per-block cooldowns and the roll.
#[derive(Clone, Debug)]
pub struct Forager {
    cfg: ForageConfig,
    rng: Rng,
    /// Tick at which each foraged block can be foraged again.
    ready_at: HashMap<[i32; 3], u64>,
}

impl Forager {
    pub fn new(cfg: ForageConfig, seed: u64) -> Forager {
        Forager {
            cfg,
            rng: Rng::new(seed),
            ready_at: HashMap::new(),
        }
    }

    pub fn config(&self) -> &ForageConfig {
        &self.cfg
    }

    /// Forage the grass at `pos` on `tick`. A forage inside the block's cooldown does nothing
    /// and rolls nothing; any other forage starts the cooldown, success or not.
    pub fn forage(&mut self, pos: [i32; 3], tick: u64) -> Forage<'_> {
        if self.ready_at.get(&pos).is_some_and(|&t| tick < t) {
            return Forage::Cooldown;
        }
        self.ready_at
            .insert(pos, tick.saturating_add(self.cfg.cooldown_ticks));
        if self.rng.chance(self.cfg.chance) {
            Forage::Drop(self.cfg.item.as_str())
        } else {
            Forage::Miss
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::assert_err_contains;

    fn cfg(chance: u32, cooldown_ticks: u64) -> ForageConfig {
        ForageConfig {
            item: "seeds".into(),
            chance,
            cooldown_ticks,
        }
    }

    #[test]
    fn shipped_defaults() {
        let c = ForageConfig::default_from_data();
        assert_eq!(
            (c.item.as_str(), c.chance, c.cooldown_ticks),
            ("seeds", 3, 1200)
        );
    }

    #[test]
    fn rejects_bad_data() {
        assert_err_contains(ForageConfig::parse(""), "missing [forage]");
        assert_err_contains(
            ForageConfig::parse("[forage]\nitem=\"s\"\nchance=0\ncooldown_ticks=1\n"),
            "`chance` must be a positive integer",
        );
    }

    #[test]
    fn chance_one_always_drops_and_chance_matches_rng() {
        let mut f = Forager::new(cfg(1, 0), 7);
        assert!((0..20).all(|t| matches!(f.forage([0, 0, 0], t), Forage::Drop(_))));
        // Distinct blocks, fixed seed: results equal the raw RNG rolls.
        let mut f = Forager::new(cfg(6, 100), 7);
        let mut r = Rng::new(7);
        for i in 0..200 {
            assert_eq!(
                matches!(f.forage([i, 0, 0], 0), Forage::Drop(_)),
                r.chance(6)
            );
        }
    }

    #[test]
    fn cooldown_blocks_then_expires() {
        let mut f = Forager::new(cfg(1, 100), 1);
        assert_eq!(f.forage([1, 2, 3], 10), Forage::Drop("seeds"));
        assert_eq!(f.forage([1, 2, 3], 109), Forage::Cooldown);
        assert_eq!(
            f.forage([4, 2, 3], 109),
            Forage::Drop("seeds"),
            "other block"
        );
        assert_eq!(f.forage([1, 2, 3], 110), Forage::Drop("seeds"));
    }

    #[test]
    fn a_miss_starts_the_cooldown() {
        let mut f = Forager::new(cfg(u32::MAX, 100), 5);
        assert_eq!(f.forage([0, 0, 0], 0), Forage::Miss);
        assert_eq!(f.forage([0, 0, 0], 99), Forage::Cooldown);
        assert_eq!(f.forage([0, 0, 0], 100), Forage::Miss);
    }

    #[test]
    fn replay_is_deterministic() {
        let run = || {
            let mut f = Forager::new(cfg(6, 5), 42);
            (0..300u64)
                .map(|t| format!("{:?}", f.forage([(t % 7) as i32, 0, 0], t)))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
