//! Crop helpers shared by picking. See docs/design/basic-gameplay.md section 9.4.
//!
//! The yields come from `data/crops.toml`.

use crate::crops::Range;
use crate::farm::{CropKind, FarmTile};
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
