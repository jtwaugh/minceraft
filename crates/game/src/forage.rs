//! Foraging tall grass for seeds, from `data/items.toml` `[forage]`.
//! See docs/design/basic-gameplay.md section 9.5.
//!
//! The grass stays; each block has a cooldown, kept here by position.

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

impl fmt::Display for ForageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "items.toml: forage: {}", self.0)
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

    /// Forage the grass at `pos` on `tick`. Returns the item name to drop on
    /// success. A forage inside the block's cooldown does nothing and rolls
    /// nothing; any other forage starts the cooldown, success or not.
    pub fn forage(&mut self, pos: [i32; 3], tick: u64) -> Option<&str> {
        if self.ready_at.get(&pos).is_some_and(|&t| tick < t) {
            return None;
        }
        self.ready_at
            .insert(pos, tick.saturating_add(self.cfg.cooldown_ticks));
        self.rng
            .chance(self.cfg.chance)
            .then_some(self.cfg.item.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            ("seeds", 6, 4800)
        );
    }

    #[test]
    fn rejects_bad_data() {
        assert!(ForageConfig::parse("").is_err());
        assert!(ForageConfig::parse("[forage]\nitem=\"s\"\nchance=0\ncooldown_ticks=1\n").is_err());
    }

    #[test]
    fn chance_one_always_drops_and_chance_matches_rng() {
        let mut f = Forager::new(cfg(1, 0), 7);
        assert!((0..20).all(|t| f.forage([0, 0, 0], t).is_some()));
        // Distinct blocks, fixed seed: results equal the raw RNG rolls.
        let mut f = Forager::new(cfg(6, 100), 7);
        let mut r = Rng::new(7);
        for i in 0..200 {
            assert_eq!(f.forage([i, 0, 0], 0).is_some(), r.chance(6));
        }
    }

    #[test]
    fn cooldown_blocks_then_expires() {
        let mut f = Forager::new(cfg(1, 100), 1);
        assert!(f.forage([1, 2, 3], 10).is_some());
        assert!(f.forage([1, 2, 3], 109).is_none());
        assert!(f.forage([4, 2, 3], 109).is_some(), "other block unaffected");
        assert!(f.forage([1, 2, 3], 110).is_some());
    }

    #[test]
    fn replay_is_deterministic() {
        let run = || {
            let mut f = Forager::new(cfg(6, 5), 42);
            (0..300u64)
                .map(|t| f.forage([(t % 7) as i32, 0, 0], t).is_some())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
