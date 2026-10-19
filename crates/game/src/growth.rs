//! Crop growth on random ticks. See docs/design/basic-gameplay.md section 9.3
//! and SPEC "Crop growth".
//!
//! Each tick every crop in the overlay may get a random tick, chosen by a hash
//! of the seed, the tick and the crop's position alone, so growth does not
//! depend on iteration order or region origin. The light condition waits for
//! the game's own light model (SPEC, decided in #392) and is not checked here.
//! Trampling is off, so nothing here ever reverts a crop.

use crate::collision::CollisionWorld;
use crate::crops::Crops;
use crate::farm::{FarmTile, FarmWorld};
use crate::growth_odds::{growth_odds, GrowthInput, Surround};
use crate::rng::Rng;
use terrain::Block;

/// Water reaches this far horizontally.
const WATER_REACH: i32 = 4;

/// Whether the farmland at `(x, y, z)` is hydrated: water within 4 blocks
/// horizontally at its level or the one below.
pub fn hydrated<W: CollisionWorld>(world: &FarmWorld<W>, x: i32, y: i32, z: i32) -> bool {
    (-WATER_REACH..=WATER_REACH).any(|dx| {
        (-WATER_REACH..=WATER_REACH).any(|dz| {
            [y, y - 1]
                .iter()
                .any(|&dy| world.block_at(x + dx, dy, z + dz) == Block::Water)
        })
    })
}

/// What the square at `(x, y, z)` adds to a neighbouring crop's odds.
fn surround<W: CollisionWorld>(world: &FarmWorld<W>, x: i32, y: i32, z: i32) -> Surround {
    match world.get(x, y, z) {
        Some(FarmTile::Farmland { .. }) if hydrated(world, x, y, z) => Surround::WetFarmland,
        Some(FarmTile::Farmland { .. }) => Surround::DryFarmland,
        _ => Surround::Other,
    }
}

fn has_crop<W: CollisionWorld>(world: &FarmWorld<W>, x: i32, y: i32, z: i32) -> bool {
    matches!(world.get(x, y, z), Some(FarmTile::Crop { .. }))
}

/// The odds input for the crop at `pos`, planted on the farmland below it.
pub fn growth_input<W: CollisionWorld>(world: &FarmWorld<W>, pos: [i32; 3]) -> GrowthInput {
    let [x, y, z] = pos;
    let mut around = [Surround::Other; 8];
    let mut diagonal = [false; 4];
    let (mut i, mut d) = (0, 0);
    for dx in -1..=1 {
        for dz in -1..=1 {
            if (dx, dz) == (0, 0) {
                continue;
            }
            around[i] = surround(world, x + dx, y - 1, z + dz);
            i += 1;
            if dx != 0 && dz != 0 {
                diagonal[d] = has_crop(world, x + dx, y, z + dz);
                d += 1;
            }
        }
    }
    GrowthInput {
        wet: hydrated(world, x, y - 1, z),
        surround: around,
        crop_x: [has_crop(world, x - 1, y, z), has_crop(world, x + 1, y, z)],
        crop_z: [has_crop(world, x, y, z - 1), has_crop(world, x, y, z + 1)],
        crop_diagonal: diagonal,
    }
}

/// Mix the seed, tick and position into one 64-bit value.
fn roll_seed(seed: u64, tick: u64, pos: [i32; 3]) -> u64 {
    let mut h = seed;
    for v in [tick, pos[0] as u64, pos[1] as u64, pos[2] as u64] {
        h = (h ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= h >> 31;
    }
    h
}

/// One tick of growth: every crop that gets a random tick and passes its
/// 1-in-N roll advances one stage, up to the last. Neighbour checks see the
/// world as it was at the start of the tick.
pub fn grow_crops<W: CollisionWorld>(
    world: &mut FarmWorld<W>,
    crops: &Crops,
    seed: u64,
    tick: u64,
) {
    let mut grown = Vec::new();
    for (pos, kind, stage) in world.crops() {
        if stage + 1 >= kind.stages() {
            continue;
        }
        let mut rng = Rng::new(roll_seed(seed, tick, pos));
        if rng.chance(crops.random_tick_interval)
            && rng.chance(growth_odds(&growth_input(world, pos)).denominator)
        {
            grown.push((
                pos,
                FarmTile::Crop {
                    kind,
                    stage: stage + 1,
                },
            ));
        }
    }
    for ([x, y, z], tile) in grown {
        world.set(x, y, z, tile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::farm::CropKind;

    const SOIL: i32 = 5;

    fn crop(stage: u32) -> FarmTile {
        FarmTile::Crop {
            kind: CropKind::Wheat,
            stage,
        }
    }

    fn field(cells: &[(i32, i32)], water: Option<(i32, i32, i32)>) -> FarmWorld<MemoryWorld> {
        let mut m = MemoryWorld::new();
        if let Some((x, y, z)) = water {
            m.set(x, y, z, Block::Water);
        }
        let mut w = FarmWorld::new(m);
        for &(x, z) in cells {
            w.set(x, SOIL, z, FarmTile::Farmland { wet: false });
            w.set(x, SOIL + 1, z, crop(0));
        }
        w
    }

    fn stage(w: &FarmWorld<MemoryWorld>, x: i32, z: i32) -> u32 {
        match w.get(x, SOIL + 1, z) {
            Some(FarmTile::Crop { stage, .. }) => stage,
            t => panic!("no crop: {t:?}"),
        }
    }

    #[test]
    fn hydration_reach() {
        for (water, expect) in [
            ((4, SOIL, 4), true),
            ((-4, SOIL - 1, 0), true),
            ((5, SOIL, 0), false),
            ((0, SOIL + 1, 0), false),
            ((0, SOIL - 2, 0), false),
        ] {
            let w = field(&[(0, 0)], Some((water.0, water.1, water.2)));
            assert_eq!(hydrated(&w, 0, SOIL, 0), expect, "{water:?}");
        }
        assert!(!hydrated(&field(&[(0, 0)], None), 0, SOIL, 0));
    }

    #[test]
    fn odds_see_water_and_neighbours() {
        let dry = field(&[(0, 0)], None);
        assert_eq!(growth_odds(&growth_input(&dry, [0, SOIL + 1, 0])).f, 2.0);
        let wet = field(&[(0, 0)], Some((2, SOIL, 0)));
        assert_eq!(growth_odds(&growth_input(&wet, [0, SOIL + 1, 0])).f, 4.0);
        // A single wet row: 4 + 2 * 0.75 for the row neighbours.
        let row = field(&[(-1, 0), (0, 0), (1, 0)], Some((2, SOIL, 0)));
        let o = growth_odds(&growth_input(&row, [0, SOIL + 1, 0]));
        assert_eq!((o.f, o.denominator), (5.5, 18));
        // A crop on a diagonal halves it.
        let diag = field(&[(0, 0), (1, 1)], Some((2, SOIL, 0)));
        assert_eq!(growth_odds(&growth_input(&diag, [0, SOIL + 1, 0])).f, 2.375);
    }

    /// Mean ticks for 200 plots (a crop in the middle of 3x3 farmland, with
    /// or without water beside it) to reach the last stage.
    fn mean_ticks_to_mature(water: bool) -> f64 {
        let crops = Crops::default_crops();
        let centres: Vec<(i32, i32)> = (0..200).map(|i| (i * 5, 0)).collect();
        let mut m = MemoryWorld::new();
        if water {
            for &(x, z) in &centres {
                m.set(x + 2, SOIL, z, Block::Water);
            }
        }
        let mut w = FarmWorld::new(m);
        for &(cx, cz) in &centres {
            for (dx, dz) in (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (dx, dz))) {
                w.set(cx + dx, SOIL, cz + dz, FarmTile::Farmland { wet: false });
            }
            w.set(cx, SOIL + 1, cz, crop(0));
        }
        let mut done = vec![0u64; centres.len()];
        for tick in 1..=150_000 {
            grow_crops(&mut w, &crops, 7, tick);
            for (i, &(x, z)) in centres.iter().enumerate() {
                if done[i] == 0 && stage(&w, x, z) == 7 {
                    done[i] = tick;
                }
            }
            if done.iter().all(|&t| t > 0) {
                break;
            }
        }
        assert!(done.iter().all(|&t| t > 0));
        done.iter().sum::<u64>() as f64 / done.len() as f64
    }

    #[test]
    fn hydrated_crops_mature_in_about_two_days_and_dry_ones_slower() {
        // A day is 4800 ticks; the data is tuned for about two.
        let wet = mean_ticks_to_mature(true);
        let dry = mean_ticks_to_mature(false);
        assert!((9_000.0..10_600.0).contains(&wet), "wet {wet}");
        assert!(dry > wet * 2.0, "dry {dry} wet {wet}");
    }

    #[test]
    fn growth_is_repeatable_and_stops_at_the_cap() {
        let crops = Crops::default_crops();
        let run = || {
            let cells: Vec<_> = (0..10).map(|i| (i * 2, 0)).collect();
            let mut w = field(&cells, Some((1, SOIL, 0)));
            for tick in 1..=3000 {
                grow_crops(&mut w, &crops, 3, tick);
            }
            cells
                .iter()
                .map(|&(x, z)| stage(&w, x, z))
                .collect::<Vec<_>>()
        };
        let a = run();
        assert_eq!(a, run());
        assert!(a.iter().any(|&s| s > 0));
        let mut w = field(&[(0, 0)], None);
        w.set(0, SOIL + 1, 0, crop(7));
        let one = Crops {
            random_tick_interval: 1,
            ..crops
        };
        for tick in 1..=2000 {
            grow_crops(&mut w, &one, 3, tick);
        }
        assert_eq!(stage(&w, 0, 0), 7);
    }

    #[test]
    fn growth_is_independent_of_region_origin() {
        use crate::collision::RegionWorld;
        use terrain::generate_decorated;
        let crops = Crops::default_crops();
        let a = generate_decorated(777, 0, 0, 48, 48);
        let b = generate_decorated(777, 16, 16, 48, 48);
        let mut wa = FarmWorld::new(RegionWorld::new(&a));
        let mut wb = FarmWorld::new(RegionWorld::new(&b));
        let cells = [(20, 70, 20), (21, 70, 20), (30, 64, 41), (40, 66, 30)];
        for w in [&mut wa, &mut wb] {
            for &(x, y, z) in &cells {
                w.set(x, y, z, FarmTile::Farmland { wet: false });
                w.set(x, y + 1, z, crop(0));
            }
        }
        let fast = Crops {
            random_tick_interval: 3,
            ..crops
        };
        for tick in 1..=500 {
            grow_crops(&mut wa, &fast, 9, tick);
            grow_crops(&mut wb, &fast, 9, tick);
        }
        let stages = |w: &FarmWorld<RegionWorld>| cells.map(|(x, y, z)| w.get(x, y + 1, z));
        assert_eq!(stages(&wa), stages(&wb));
        assert!(stages(&wa).iter().any(|t| *t != Some(crop(0))));
    }
}
