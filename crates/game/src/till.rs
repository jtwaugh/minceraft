//! Tilling grass and dirt with the hoe, from `data/items.toml` `[till]`.
//! See docs/design/basic-gameplay.md section 9.1.
//!
//! Tilling is a block transform in the farm overlay; generated blocks are
//! untouched. A planter becomes tilled planter farmland and never drops.

use crate::collision::CollisionWorld;
use crate::farm::{FarmTile, FarmWorld};
use crate::rng::Rng;
use std::fmt;
use terrain::Block;

const DEFAULT_ITEMS: &str = include_str!("../../../data/items.toml");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TillConfig {
    /// Item dropped when grass is tilled.
    pub item: String,
    /// The drop happens 1 in `chance`.
    pub chance: u32,
    /// Ticks between repeats while secondary is held.
    pub repeat_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TillError(pub String);

impl fmt::Display for TillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "items.toml: till: {}", self.0)
    }
}

impl std::error::Error for TillError {}

impl TillConfig {
    pub fn default_from_data() -> TillConfig {
        TillConfig::parse(DEFAULT_ITEMS).expect("data/items.toml is valid")
    }

    pub fn parse(text: &str) -> Result<TillConfig, TillError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| TillError(e.to_string()))?;
        let t = root
            .get("till")
            .ok_or_else(|| TillError("missing [till]".into()))?;
        let int = |key: &str| {
            t.get(key)
                .and_then(|v| v.as_integer())
                .filter(|n| *n >= 1)
                .ok_or_else(|| TillError(format!("`{key}` must be a positive integer")))
        };
        let item = t
            .get("item")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| TillError("`item` must be a non-empty string".into()))?;
        Ok(TillConfig {
            item: item.to_string(),
            chance: u32::try_from(int("chance")?)
                .map_err(|_| TillError("`chance` is too large".into()))?,
            repeat_ticks: int("repeat_ticks")? as u64,
        })
    }
}

/// The roll and the hold-repeat state.
#[derive(Clone, Debug)]
pub struct Tiller {
    cfg: TillConfig,
    rng: Rng,
    /// Block and tick of the last till, for hold repeats.
    last: Option<([i32; 3], u64)>,
}

impl Tiller {
    pub fn new(cfg: TillConfig, seed: u64) -> Tiller {
        Tiller {
            cfg,
            rng: Rng::new(seed),
            last: None,
        }
    }

    pub fn config(&self) -> &TillConfig {
        &self.cfg
    }

    /// Whether a held secondary should act again on `pos` at `tick`: the
    /// repeat interval has passed since the last till and `pos` is a
    /// different block.
    pub fn repeat_due(&self, pos: [i32; 3], tick: u64) -> bool {
        self.last
            .is_some_and(|(p, t)| p != pos && tick >= t.saturating_add(self.cfg.repeat_ticks))
    }

    /// Till the block at `pos` on `tick`. Grass or dirt with air above becomes
    /// dry farmland, a planter dry planter farmland. Returns `None` if nothing changed, else the item to drop
    /// (`Some(None)` for no drop). Anything else changes nothing and rolls
    /// nothing.
    pub fn till<W: CollisionWorld>(
        &mut self,
        world: &mut FarmWorld<W>,
        pos: [i32; 3],
        tick: u64,
    ) -> Option<Option<&str>> {
        let [x, y, z] = pos;
        let block = world.block_at(x, y, z);
        let tile = match block {
            Block::Grass | Block::Dirt => FarmTile::Farmland { wet: false },
            Block::PlanterDirt => FarmTile::PlanterFarmland { wet: false },
            _ => return None,
        };
        if world.block_at(x, y + 1, z) != Block::Air || !world.set(x, y, z, tile) {
            return None;
        }
        self.last = Some((pos, tick));
        let drop = block == Block::Grass && self.rng.chance(self.cfg.chance);
        Some(drop.then_some(self.cfg.item.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::data_error::test_support::assert_err_contains;

    fn cfg(chance: u32) -> TillConfig {
        TillConfig {
            item: "seeds".into(),
            chance,
            repeat_ticks: 4,
        }
    }

    fn world(floor: Block, above: Block) -> FarmWorld<MemoryWorld> {
        let mut w = MemoryWorld::default();
        w.set(0, 5, 0, floor);
        w.set(0, 6, 0, above);
        w.set(1, 5, 0, Block::Stone);
        FarmWorld::new(w)
    }

    #[test]
    fn shipped_defaults() {
        let c = TillConfig::default_from_data();
        assert_eq!((c.item.as_str(), c.chance, c.repeat_ticks), ("seeds", 8, 4));
    }

    #[test]
    fn rejects_bad_data() {
        assert_err_contains(TillConfig::parse(""), "missing [till]");
        assert_err_contains(
            TillConfig::parse("[till]\nitem=\"s\"\nchance=0\nrepeat_ticks=4\n"),
            "`chance` must be a positive integer",
        );
    }

    #[test]
    fn grass_and_dirt_with_air_above_become_dry_farmland() {
        for floor in [Block::Grass, Block::Dirt] {
            let mut w = world(floor, Block::Air);
            let mut t = Tiller::new(cfg(1), 1);
            assert!(t.till(&mut w, [0, 5, 0], 0).is_some());
            assert_eq!(w.block_at(0, 5, 0), Block::FarmlandDry);
            assert_eq!(w.get(0, 5, 0), Some(FarmTile::Farmland { wet: false }));
            assert_eq!(w.block_at(1, 5, 0), Block::Stone, "neighbour unchanged");
            assert_eq!(w.block_at(0, 6, 0), Block::Air);
        }
    }

    #[test]
    fn planter_with_air_above_becomes_planter_farmland_and_never_drops() {
        let mut w = world(Block::PlanterDirt, Block::Air);
        let mut t = Tiller::new(cfg(1), 1);
        assert_eq!(t.till(&mut w, [0, 5, 0], 0), Some(None));
        assert_eq!(w.block_at(0, 5, 0), Block::PlanterFarmlandDry);
        assert_eq!(
            w.get(0, 5, 0),
            Some(FarmTile::PlanterFarmland { wet: false })
        );
        assert!(t.repeat_due([1, 5, 0], 4));
        // Already tilled: nothing more to do.
        assert_eq!(t.till(&mut w, [0, 5, 0], 9), None);
    }

    #[test]
    fn planter_with_a_block_on_top_does_not_till() {
        let mut w = world(Block::PlanterDirt, Block::Stone);
        let mut t = Tiller::new(cfg(1), 1);
        assert_eq!(t.till(&mut w, [0, 5, 0], 0), None);
        assert_eq!(w.block_at(0, 5, 0), Block::PlanterDirt);
        assert_eq!(w.get(0, 5, 0), None);
    }

    #[test]
    fn blocked_or_other_blocks_do_nothing() {
        for (floor, above) in [
            (Block::Grass, Block::Stone),
            (Block::Dirt, Block::TallGrass),
            (Block::Stone, Block::Air),
            (Block::Sand, Block::Air),
            (Block::Water, Block::Air),
            (Block::FarmlandDry, Block::Air),
        ] {
            let mut w = world(floor, above);
            let mut t = Tiller::new(cfg(1), 1);
            assert_eq!(t.till(&mut w, [0, 5, 0], 0), None, "{floor:?}/{above:?}");
            assert_eq!(w.block_at(0, 5, 0), floor);
            assert_eq!(w.get(0, 5, 0), None);
            assert!(!t.repeat_due([1, 5, 0], 100), "no till, no repeat");
        }
    }

    #[test]
    fn only_grass_drops_and_the_roll_uses_the_rng() {
        let mut w = world(Block::Dirt, Block::Air);
        assert_eq!(
            Tiller::new(cfg(1), 1).till(&mut w, [0, 5, 0], 0),
            Some(None)
        );
        let mut w = world(Block::Grass, Block::Air);
        assert_eq!(
            Tiller::new(cfg(1), 1).till(&mut w, [0, 5, 0], 0),
            Some(Some("seeds"))
        );
        // Fixed seed: results equal the raw RNG rolls.
        let mut t = Tiller::new(cfg(8), 9);
        let mut r = Rng::new(9);
        for i in 0..100 {
            let mut w = world(Block::Grass, Block::Air);
            let got = t.till(&mut w, [0, 5, 0], i).unwrap().is_some();
            assert_eq!(got, r.chance(8));
        }
    }

    #[test]
    fn repeat_needs_the_interval_and_a_new_block() {
        let mut w = world(Block::Grass, Block::Air);
        let mut t = Tiller::new(cfg(1), 1);
        assert!(!t.repeat_due([1, 5, 0], 0), "nothing tilled yet");
        t.till(&mut w, [0, 5, 0], 10);
        assert!(!t.repeat_due([1, 5, 0], 13), "too soon");
        assert!(!t.repeat_due([0, 5, 0], 14), "same block");
        assert!(t.repeat_due([1, 5, 0], 14));
    }
}
