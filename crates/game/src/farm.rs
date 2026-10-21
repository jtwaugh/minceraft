//! Farm tiles over the generated world (basic-gameplay.md section 9 and
//! reconciliation note 7, decided in #288).
//!
//! Farmland and crop stages are ordinary blocks in `data/blocks.toml` that the
//! generator never places. `FarmWorld` keeps a per-position edit map and
//! `block_at` consults it first, so collision and targeting see the tiles
//! through the usual `CollisionWorld` queries. The map is keyed by world
//! position, so it does not depend on any region origin or size. `terrain`
//! knows nothing about it.

use crate::collision::CollisionWorld;
use std::collections::HashMap;
use terrain::{Block, HEIGHT};

/// Crop kinds, in the order of `data/crops.toml` entries that have blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropKind {
    Wheat,
}

const WHEAT: [Block; 8] = [
    Block::Wheat0,
    Block::Wheat1,
    Block::Wheat2,
    Block::Wheat3,
    Block::Wheat4,
    Block::Wheat5,
    Block::Wheat6,
    Block::Wheat7,
];

impl CropKind {
    /// Number of growth stages; the last is mature.
    pub fn stages(self) -> u32 {
        match self {
            CropKind::Wheat => WHEAT.len() as u32,
        }
    }
}

/// A tile in the overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FarmTile {
    Farmland {
        wet: bool,
    },
    /// Tilled planter: farmland that keeps the planter's sides and bottom.
    PlanterFarmland {
        wet: bool,
    },
    /// `stage` counts from 0 (just planted) to `kind.stages() - 1` (mature).
    Crop {
        kind: CropKind,
        stage: u32,
    },
}

impl FarmTile {
    /// The block that stands for this tile; `None` for a stage out of range.
    pub fn block(self) -> Option<Block> {
        match self {
            FarmTile::Farmland { wet: false } => Some(Block::FarmlandDry),
            FarmTile::Farmland { wet: true } => Some(Block::FarmlandWet),
            FarmTile::PlanterFarmland { wet: false } => Some(Block::PlanterFarmlandDry),
            FarmTile::PlanterFarmland { wet: true } => Some(Block::PlanterFarmlandWet),
            FarmTile::Crop {
                kind: CropKind::Wheat,
                stage,
            } => WHEAT.get(stage as usize).copied(),
        }
    }

    /// The tile a block stands for, if it is a farm block.
    pub fn from_block(block: Block) -> Option<FarmTile> {
        match block {
            Block::FarmlandDry => Some(FarmTile::Farmland { wet: false }),
            Block::FarmlandWet => Some(FarmTile::Farmland { wet: true }),
            Block::PlanterFarmlandDry => Some(FarmTile::PlanterFarmland { wet: false }),
            Block::PlanterFarmlandWet => Some(FarmTile::PlanterFarmland { wet: true }),
            b => WHEAT.iter().position(|w| *w == b).map(|i| FarmTile::Crop {
                kind: CropKind::Wheat,
                stage: i as u32,
            }),
        }
    }
}

/// A world with farm tiles laid over `base`.
pub struct FarmWorld<W> {
    base: W,
    edits: HashMap<(i32, i32, i32), Block>,
}

impl<W: CollisionWorld> FarmWorld<W> {
    pub fn new(base: W) -> FarmWorld<W> {
        FarmWorld {
            base,
            edits: HashMap::new(),
        }
    }

    pub fn base(&self) -> &W {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut W {
        &mut self.base
    }

    /// Put `tile` at `(x, y, z)`. Returns false, changing nothing, when the
    /// position is outside the world's height or the stage is out of range.
    pub fn set(&mut self, x: i32, y: i32, z: i32, tile: FarmTile) -> bool {
        match tile.block() {
            Some(b) if (0..HEIGHT as i32).contains(&y) => {
                self.edits.insert((x, y, z), b);
                true
            }
            _ => false,
        }
    }

    /// Record `block` at `(x, y, z)` as an edit over the generated world; any
    /// block, not only farm tiles. Returns false, changing nothing, when `y`
    /// is outside the world's height.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: Block) -> bool {
        if !(0..HEIGHT as i32).contains(&y) {
            return false;
        }
        self.edits.insert((x, y, z), block);
        true
    }

    /// Record that `(x, y, z)` is now air. Same as `set_block` with air.
    pub fn remove_block(&mut self, x: i32, y: i32, z: i32) -> bool {
        self.set_block(x, y, z, Block::Air)
    }

    /// The overlay tile at `(x, y, z)`, if any.
    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<FarmTile> {
        self.edits
            .get(&(x, y, z))
            .and_then(|b| FarmTile::from_block(*b))
    }

    /// Positions and stages of every crop in the overlay, in no set order.
    pub fn crops(&self) -> impl Iterator<Item = ([i32; 3], CropKind, u32)> + '_ {
        self.edits
            .iter()
            .filter_map(|(&(x, y, z), &b)| match FarmTile::from_block(b) {
                Some(FarmTile::Crop { kind, stage }) => Some(([x, y, z], kind, stage)),
                _ => None,
            })
    }

    /// Every overlay block with its position, sorted by position.
    pub fn tiles(&self) -> Vec<([i32; 3], Block)> {
        let mut v: Vec<_> = self
            .edits
            .iter()
            .map(|(&(x, y, z), &b)| ([x, y, z], b))
            .collect();
        v.sort_by_key(|t| t.0);
        v
    }

    /// Remove the tile at `(x, y, z)`, restoring the generated block. Returns
    /// the tile that was there.
    pub fn clear(&mut self, x: i32, y: i32, z: i32) -> Option<FarmTile> {
        self.edits.remove(&(x, y, z)).and_then(FarmTile::from_block)
    }
}

impl<W: CollisionWorld> CollisionWorld for FarmWorld<W> {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        match self.edits.get(&(x, y, z)) {
            Some(b) => *b,
            None => self.base.block_at(x, y, z),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Aabb, MemoryWorld, RegionWorld};
    use crate::targeting::{cast_block, Face, Target};
    use terrain::generate_decorated;

    fn dry() -> FarmTile {
        FarmTile::Farmland { wet: false }
    }

    fn wheat(stage: u32) -> FarmTile {
        FarmTile::Crop {
            kind: CropKind::Wheat,
            stage,
        }
    }

    #[test]
    fn block_round_trip() {
        for t in [dry(), FarmTile::Farmland { wet: true }, wheat(0), wheat(7)] {
            assert_eq!(FarmTile::from_block(t.block().unwrap()), Some(t));
        }
        assert_eq!(wheat(8).block(), None);
        assert_eq!(FarmTile::from_block(Block::Dirt), None);
        assert_eq!(CropKind::Wheat.stages(), 8);
    }

    #[test]
    fn set_get_clear() {
        let mut m = MemoryWorld::new();
        m.set(1, 5, 1, Block::Grass);
        let mut w = FarmWorld::new(m);
        assert_eq!(w.get(1, 5, 1), None);
        assert!(w.set(1, 5, 1, dry()));
        assert_eq!(w.block_at(1, 5, 1), Block::FarmlandDry);
        assert!(w.set(1, 6, 1, wheat(3)));
        assert_eq!(w.block_at(1, 6, 1), Block::Wheat3);
        assert_eq!(w.get(1, 6, 1), Some(wheat(3)));
        assert!(!w.set(1, 7, 1, wheat(8)));
        assert!(!w.set(1, HEIGHT as i32, 1, dry()));
        assert_eq!(w.clear(1, 5, 1), Some(dry()));
        assert_eq!(w.block_at(1, 5, 1), Block::Grass);
        assert_eq!(w.clear(1, 5, 1), None);
    }

    #[test]
    fn remove_block_edits_any_cell() {
        let mut m = MemoryWorld::new();
        m.set(2, 5, 2, Block::Grass);
        let mut w = FarmWorld::new(m);
        assert!(w.remove_block(2, 5, 2));
        assert_eq!(w.block_at(2, 5, 2), Block::Air);
        assert_eq!(w.get(2, 5, 2), None);
        assert!(w.set_block(0, 3, 0, Block::Stone));
        assert_eq!(w.block_at(0, 3, 0), Block::Stone);
        assert_eq!(
            w.tiles(),
            vec![([0, 3, 0], Block::Stone), ([2, 5, 2], Block::Air)]
        );
        assert!(!w.remove_block(2, -1, 2));
        assert!(!w.set_block(2, HEIGHT as i32, 2, Block::Air));
        assert_eq!(w.tiles().len(), 2);
        assert_eq!(w.clear(2, 5, 2), None);
        assert_eq!(w.block_at(2, 5, 2), Block::Grass);
    }

    #[test]
    fn removed_block_ignores_region_origin() {
        let a = generate_decorated(777, 0, 0, 48, 48);
        let b = generate_decorated(777, 16, 16, 48, 48);
        let mut wa = FarmWorld::new(RegionWorld::new(&a));
        let mut wb = FarmWorld::new(RegionWorld::new(&b));
        let cells = [(20, 70, 20), (30, 64, 41), (16, 10, 16)];
        for (x, y, z) in cells {
            assert!(wa.remove_block(x, y, z));
            assert!(wb.remove_block(x, y, z));
        }
        for x in 16..48 {
            for z in 16..48 {
                for y in (0..HEIGHT as i32).step_by(3).chain([70, 64, 10]) {
                    assert_eq!(wa.block_at(x, y, z), wb.block_at(x, y, z));
                }
            }
        }
        for (x, y, z) in cells {
            assert_eq!(wa.block_at(x, y, z), Block::Air);
        }
    }

    #[test]
    fn farmland_collides_and_crops_do_not() {
        let mut w = FarmWorld::new(MemoryWorld::new());
        w.set(0, 3, 0, FarmTile::Farmland { wet: true });
        w.set(0, 4, 0, wheat(7));
        let mut out = Vec::new();
        w.boxes_in(&Aabb::new([0.1, 3.0, 0.1], [0.9, 5.0, 0.9]), &mut out);
        assert_eq!(out, vec![Aabb::new([0.0, 3.0, 0.0], [1.0, 4.0, 1.0])]);
        let mut sel = Vec::new();
        w.selection_boxes_in(&Aabb::new([0.5, 3.5, 0.5], [0.5, 4.5, 0.5]), &mut sel);
        assert_eq!(sel.len(), 2);
    }

    #[test]
    fn overlay_ignores_region_origin() {
        let a = generate_decorated(777, 0, 0, 48, 48);
        let b = generate_decorated(777, 16, 16, 48, 48);
        let mut wa = FarmWorld::new(RegionWorld::new(&a));
        let mut wb = FarmWorld::new(RegionWorld::new(&b));
        for (x, y, z) in [(20, 70, 20), (30, 64, 41), (16, 10, 16)] {
            wa.set(x, y, z, wheat(2));
            wb.set(x, y, z, wheat(2));
        }
        for x in 16..48 {
            for z in 16..48 {
                for y in (0..HEIGHT as i32).step_by(3).chain([70, 64, 10]) {
                    assert_eq!(wa.block_at(x, y, z), wb.block_at(x, y, z));
                }
            }
        }
        assert_eq!(wa.block_at(20, 70, 20), Block::Wheat2);
    }

    #[test]
    fn overlay_tile_is_targeted() {
        let mut w = FarmWorld::new(MemoryWorld::new());
        w.set(0, 0, 0, FarmTile::Farmland { wet: false });
        w.set(4, 0, 0, wheat(7));
        fn hit(w: &FarmWorld<MemoryWorld>, o: [f64; 3]) -> Option<Target> {
            cast_block(w, o, [0.0, -1.0, 0.0], 5.0).map(|h| h.0)
        }
        assert_eq!(
            hit(&w, [0.5, 3.0, 0.5]),
            Some(Target::Block {
                pos: [0, 0, 0],
                face: Face::Top
            })
        );
        // Crops have no collision box but are still targetable.
        assert!(matches!(
            hit(&w, [4.5, 3.0, 0.5]),
            Some(Target::Block { pos: [4, 0, 0], .. })
        ));
        w.clear(0, 0, 0);
        assert_eq!(hit(&w, [0.5, 3.0, 0.5]), None);
    }
}
