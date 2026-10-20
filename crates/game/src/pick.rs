//! Picking a crop at any growth stage. See docs/design/picking-plants.md
//! Phase A; the yields are those of docs/design/basic-gameplay.md section 9.4.
//!
//! The crop tile is removed; the farmland below is left as it is.

use crate::collision::CollisionWorld;
use crate::crops::Crops;
use crate::farm::{FarmTile, FarmWorld};
use crate::harvest::{count, crop_name, tile_crop_state};
use crate::rng::Rng;

/// Pick the crop at `pos`: remove it and return the item names dropped,
/// produce first. A mature crop drops its produce and seeds, an immature one
/// nothing. No crop at `pos` changes nothing and returns `None`.
pub fn pick_crop<W: CollisionWorld>(
    world: &mut FarmWorld<W>,
    crops: &Crops,
    rng: &mut Rng,
    pos: [i32; 3],
) -> Option<Vec<String>> {
    let [x, y, z] = pos;
    let tile = world.get(x, y, z)?;
    let FarmTile::Crop { kind, .. } = tile else {
        return None;
    };
    let mature = tile_crop_state(tile) == Some("mature");
    let def = crops.get(crop_name(kind))?;
    world.clear(x, y, z);
    if !mature {
        return Some(Vec::new());
    }
    let mut drops = vec![def.item.clone(); count(rng, def.item_count) as usize];
    let seeds = count(rng, def.seeds) as usize;
    drops.extend(std::iter::repeat_n(crops.seeds_item.clone(), seeds));
    Some(drops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::farm::CropKind;
    use crate::plant::plant;
    use crate::targeting::Face;
    use terrain::Block;

    const FARMLAND: FarmTile = FarmTile::Farmland { wet: true };

    fn world(stage: Option<u32>) -> FarmWorld<MemoryWorld> {
        let mut base = MemoryWorld::default();
        base.set(0, 5, 0, Block::Dirt);
        let mut w = FarmWorld::new(base);
        w.set(0, 5, 0, FARMLAND);
        if let Some(stage) = stage {
            let kind = CropKind::Wheat;
            w.set(0, 6, 0, FarmTile::Crop { kind, stage });
        }
        w
    }

    #[test]
    fn no_crop_changes_nothing() {
        let crops = Crops::default_crops();
        let mut rng = Rng::new(1);
        let mut w = world(None);
        assert_eq!(pick_crop(&mut w, &crops, &mut rng, [0, 6, 0]), None);
        assert_eq!(pick_crop(&mut w, &crops, &mut rng, [0, 5, 0]), None);
        assert_eq!(w.get(0, 5, 0), Some(FARMLAND));
        assert_eq!(w.block_at(0, 6, 0), Block::Air);
    }

    #[test]
    fn immature_crop_is_removed_with_no_drops() {
        let crops = Crops::default_crops();
        let mut rng = Rng::new(1);
        let last = CropKind::Wheat.stages() - 1;
        for stage in [0, 3, last - 1] {
            let mut w = world(Some(stage));
            assert_eq!(pick_crop(&mut w, &crops, &mut rng, [0, 6, 0]), Some(vec![]));
            assert_eq!(w.get(0, 6, 0), None);
            assert_eq!(w.block_at(0, 6, 0), Block::Air);
        }
    }

    #[test]
    fn mature_crop_is_removed_and_drops_its_yields() {
        let crops = Crops::default_crops();
        let last = CropKind::Wheat.stages() - 1;
        let mut seen_seeds = [false; 3];
        for seed in 0..40 {
            let mut w = world(Some(last));
            let mut rng = Rng::new(seed);
            let drops = pick_crop(&mut w, &crops, &mut rng, [0, 6, 0]).unwrap();
            assert_eq!(drops[0], "wheat");
            assert!(drops[1..].iter().all(|d| d == "seeds"));
            seen_seeds[drops.len() - 1] = true;
            assert_eq!(w.get(0, 6, 0), None);
            assert_eq!(w.block_at(0, 6, 0), Block::Air);
        }
        assert_eq!(seen_seeds, [true; 3], "0 to 2 seeds all occur");
    }

    #[test]
    fn fixed_rng_gives_fixed_drops() {
        let crops = Crops::default_crops();
        let run = || {
            let mut w = world(Some(CropKind::Wheat.stages() - 1));
            pick_crop(&mut w, &crops, &mut Rng::new(9), [0, 6, 0])
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn farmland_is_unchanged_and_can_be_replanted() {
        let crops = Crops::default_crops();
        let mut rng = Rng::new(3);
        let mut w = world(Some(2));
        assert!(pick_crop(&mut w, &crops, &mut rng, [0, 6, 0]).is_some());
        assert_eq!(w.get(0, 5, 0), Some(FARMLAND));
        assert!(plant(&mut w, [0, 5, 0], Face::Top, CropKind::Wheat));
    }
}
