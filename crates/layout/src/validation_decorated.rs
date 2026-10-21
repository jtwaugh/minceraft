//! Phase G validation checks (design 6.1): measured on a **decorated**
//! region. The Phase B flat-grass check stays on undecorated terrain.

use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

use crate::validation::{
    block_at, flat_grass_tiles_with, island_columns_with, CheckResult, Thresholds,
    ValidationReport, CHECK_CLEAR_FLAT, CHECK_TREES,
};
use crate::Level;

fn is_log(b: u8) -> bool {
    [Block::OakLog, Block::BirchLog, Block::SpruceLog]
        .iter()
        .any(|&l| l as u8 == b)
}

/// Blocks that make a tile decorated (design 6.1, decided in #848).
fn is_decoration(b: u8) -> bool {
    is_log(b)
        || [
            Block::OakLeaves,
            Block::BirchLeaves,
            Block::SpruceLeaves,
            Block::TallGrass,
            Block::Fern,
            Block::Dandelion,
            Block::Rose,
            Block::BrownMushroom,
            Block::RedMushroom,
            Block::SugarCane,
            Block::Pumpkin,
            Block::Cactus,
            Block::SnowLayer,
        ]
        .iter()
        .any(|&d| d as u8 == b)
}

/// Top block `(block, y)` of a column that is not air, water, ice or
/// decoration.
fn ground_under_decoration(region: &Region, x: i32, z: i32) -> Option<(u8, i32)> {
    (0..HEIGHT as i32).rev().find_map(|y| {
        let b = block_at(region, x, y, z)?;
        (b != Block::Air as u8
            && b != Block::Water as u8
            && b != Block::Ice as u8
            && !is_decoration(b))
        .then_some((b, y))
    })
}

fn decorated(region: &Region, x: i32, z: i32) -> bool {
    let Some((_, gy)) = ground_under_decoration(region, x, z) else {
        return false;
    };
    (gy + 1..HEIGHT as i32).any(|y| block_at(region, x, y, z).is_some_and(is_decoration))
}

/// Run the Phase G checks for `level` on a **decorated** `region`: flat grass
/// tiles with nothing above the ground, and whether any log stands on the
/// island.
pub fn validate_decorated(region: &Region, level: &Level, t: &Thresholds) -> ValidationReport {
    let island = island_columns_with(region, level.island_center, |r, x, z| {
        ground_under_decoration(r, x as i32 + r.x0, z as i32 + r.z0)
            .is_some_and(|(_, y)| y >= SEA_LEVEL)
    });
    let clear = flat_grass_tiles_with(region, &island, ground_under_decoration, |x, z| {
        !decorated(region, x, z)
    });
    let has_tree = island
        .iter()
        .any(|&(x, z)| (0..HEIGHT as i32).any(|y| block_at(region, x, y, z).is_some_and(is_log)));
    let trees = usize::from(has_tree);
    ValidationReport {
        checks: vec![
            CheckResult {
                name: CHECK_CLEAR_FLAT,
                measured: clear as f64,
                required: format!(">= {}", t.min_clear_flat_tiles),
                passed: clear >= t.min_clear_flat_tiles as usize,
            },
            CheckResult {
                name: CHECK_TREES,
                measured: trees as f64,
                required: format!(">= {}", t.min_trees),
                passed: trees >= t.min_trees as usize,
            },
        ],
    }
}
