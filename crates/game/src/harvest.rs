//! Harvesting mature crops. See docs/design/basic-gameplay.md section 9.4.
//!
//! The yields come from `data/crops.toml`; the crop goes back to stage 0.

use crate::collision::CollisionWorld;
use crate::crops::{Crops, Range};
use crate::farm::{CropKind, FarmTile, FarmWorld};
use crate::rng::Rng;

/// The name of a crop kind, as in `data/crops.toml`.
pub fn crop_name(kind: CropKind) -> &'static str {
    match kind {
        CropKind::Wheat => "wheat",
    }
}

/// The state rules match on for a crop tile: `mature` at its last stage,
/// else `growing`. `None` for anything else.
pub fn tile_crop_state(tile: FarmTile) -> Option<&'static str> {
    match tile {
        FarmTile::Crop { kind, stage } if stage + 1 >= kind.stages() => Some("mature"),
        FarmTile::Crop { .. } => Some("growing"),
        FarmTile::Farmland { .. } | FarmTile::PlanterFarmland { .. } => None,
    }
}

pub(crate) fn count(rng: &mut Rng, r: Range) -> u32 {
    rng.range(r.min as i32, r.max as i32) as u32
}

/// Harvest the crop at `pos`: reset it to stage 0 and return the item names
/// dropped, produce first. An immature crop, or no crop, changes nothing and
/// returns `None`.
pub fn harvest<W: CollisionWorld>(
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
    if tile_crop_state(tile) != Some("mature") {
        return None;
    }
    let def = crops.get(crop_name(kind))?;
    world.set(x, y, z, FarmTile::Crop { kind, stage: 0 });
    let mut drops = vec![def.item.clone(); count(rng, def.item_count) as usize];
    let seeds = count(rng, def.seeds) as usize;
    drops.extend(std::iter::repeat_n(crops.seeds_item.clone(), seeds));
    Some(drops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use terrain::Block;

    fn world(stage: Option<u32>) -> FarmWorld<MemoryWorld> {
        let mut w = FarmWorld::new(MemoryWorld::default());
        if let Some(stage) = stage {
            let kind = CropKind::Wheat;
            w.set(0, 5, 0, FarmTile::Crop { kind, stage });
        }
        w
    }

    #[test]
    fn mature_crop_drops_its_yields_and_resets() {
        let crops = Crops::default_crops();
        let last = CropKind::Wheat.stages() - 1;
        let mut seen_seeds = [false; 3];
        for seed in 0..40 {
            let mut w = world(Some(last));
            let mut rng = Rng::new(seed);
            let drops = harvest(&mut w, &crops, &mut rng, [0, 5, 0]).unwrap();
            assert_eq!(drops[0], "wheat");
            assert!(drops[1..].iter().all(|d| d == "seeds"));
            seen_seeds[drops.len() - 1] = true;
            assert_eq!(w.block_at(0, 5, 0), Block::Wheat0);
        }
        assert_eq!(seen_seeds, [true; 3], "0 to 2 seeds all occur");
    }

    #[test]
    fn fixed_rng_gives_fixed_drops() {
        let crops = Crops::default_crops();
        let run = || {
            let mut w = world(Some(CropKind::Wheat.stages() - 1));
            harvest(&mut w, &crops, &mut Rng::new(9), [0, 5, 0])
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn immature_crop_and_empty_ground_do_nothing() {
        let crops = Crops::default_crops();
        let mut rng = Rng::new(1);
        let last = CropKind::Wheat.stages() - 1;
        for stage in [0, 3, last - 1] {
            let mut w = world(Some(stage));
            let before = w.block_at(0, 5, 0);
            assert_eq!(harvest(&mut w, &crops, &mut rng, [0, 5, 0]), None);
            assert_eq!(w.block_at(0, 5, 0), before);
            assert_eq!(tile_crop_state(w.get(0, 5, 0).unwrap()), Some("growing"));
        }
        let mut w = world(None);
        assert_eq!(harvest(&mut w, &crops, &mut rng, [0, 5, 0]), None);
    }

    #[test]
    fn a_harvested_crop_is_not_mature_until_it_regrows() {
        let crops = Crops::default_crops();
        let mut w = world(Some(CropKind::Wheat.stages() - 1));
        let mut rng = Rng::new(2);
        assert!(harvest(&mut w, &crops, &mut rng, [0, 5, 0]).is_some());
        assert_eq!(harvest(&mut w, &crops, &mut rng, [0, 5, 0]), None);
    }
}
