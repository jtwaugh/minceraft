//! Planting seeds on farmland. See docs/design/basic-gameplay.md section 9.2.
//!
//! The crop goes into the farm overlay in the air block above the farmland.

use crate::collision::CollisionWorld;
use crate::farm::{CropKind, FarmTile, FarmWorld};
use terrain::Block;

/// Which crop each seed item plants. The one place the mapping lives.
const SEED_CROPS: [(&str, CropKind); 1] = [("seeds", CropKind::Wheat)];

/// The crop a held item plants, or `None` if it is not a seed item.
pub fn crop_for_item(item: &str) -> Option<CropKind> {
    SEED_CROPS.iter().find(|(i, _)| *i == item).map(|&(_, k)| k)
}

/// Plant a stage-0 `kind` crop on the farmland at `pos`. Returns true if one was
/// placed. Anything but farmland with air above changes nothing, so an
/// already planted tile (its crop fills the air above) is left alone.
pub fn plant<W: CollisionWorld>(world: &mut FarmWorld<W>, pos: [i32; 3], kind: CropKind) -> bool {
    let [x, y, z] = pos;
    if !matches!(world.get(x, y, z), Some(FarmTile::Farmland { .. }))
        || world.block_at(x, y + 1, z) != Block::Air
    {
        return false;
    }
    let crop = FarmTile::Crop { kind, stage: 0 };
    world.set(x, y + 1, z, crop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;

    fn world(tile: Option<FarmTile>, above: Block) -> FarmWorld<MemoryWorld> {
        let mut w = MemoryWorld::default();
        w.set(0, 5, 0, Block::Dirt);
        w.set(0, 6, 0, above);
        let mut w = FarmWorld::new(w);
        if let Some(t) = tile {
            w.set(0, 5, 0, t);
        }
        w
    }

    #[test]
    fn farmland_with_air_above_gets_a_stage_zero_crop() {
        for wet in [false, true] {
            let mut w = world(Some(FarmTile::Farmland { wet }), Block::Air);
            assert!(plant(&mut w, [0, 5, 0], CropKind::Wheat));
            assert_eq!(w.block_at(0, 6, 0), Block::Wheat0);
            assert_eq!(w.get(0, 5, 0), Some(FarmTile::Farmland { wet }));
        }
    }

    #[test]
    fn items_map_to_their_crop() {
        assert_eq!(crop_for_item("seeds"), Some(CropKind::Wheat));
        assert_eq!(crop_for_item("leather"), None);
        assert_eq!(crop_for_item("tool:hoe"), None);
    }

    #[test]
    fn planting_twice_does_nothing_the_second_time() {
        let mut w = world(Some(FarmTile::Farmland { wet: false }), Block::Air);
        assert!(plant(&mut w, [0, 5, 0], CropKind::Wheat));
        assert!(!plant(&mut w, [0, 5, 0], CropKind::Wheat));
    }

    #[test]
    fn other_blocks_and_blocked_tops_do_nothing() {
        let mut w = world(None, Block::Air);
        assert!(!plant(&mut w, [0, 5, 0], CropKind::Wheat), "plain dirt");
        let mut w = world(Some(FarmTile::Farmland { wet: false }), Block::Stone);
        assert!(!plant(&mut w, [0, 5, 0], CropKind::Wheat), "blocked above");
        assert_eq!(w.block_at(0, 6, 0), Block::Stone);
        let crop = FarmTile::Crop {
            kind: CropKind::Wheat,
            stage: 3,
        };
        let mut w = world(Some(crop), Block::Air);
        assert!(
            !plant(&mut w, [0, 5, 0], CropKind::Wheat),
            "a crop is not farmland"
        );
    }
}
