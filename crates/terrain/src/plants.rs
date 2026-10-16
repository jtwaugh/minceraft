//! Small-plant patch generators (SPEC 12). Each returns how many blocks it
//! placed. Positions with no room, bad ground or bad light are skipped.

use crate::data::{blocks_movement, is_leaves, is_liquid, is_opaque_cube, Block};
use crate::decorate::World;
use crate::noise::Rng;
use crate::{AIR, DIRT, GRASS, SAND};

const CANE: u8 = Block::SugarCane as u8;
const CACTUS: u8 = Block::Cactus as u8;
const PUMPKIN: u8 = Block::Pumpkin as u8;
const DEAD_BUSH: u8 = Block::DeadBush as u8;
const BROWN: u8 = Block::BrownMushroom as u8;
const RED: u8 = Block::RedMushroom as u8;

/// Triangular offset in `[-(s-1), s-1]` ("spread", SPEC Part II notation).
#[inline]
fn spread(rng: &mut Rng, s: u32) -> i32 {
    rng.below(s) as i32 - rng.below(s) as i32
}

/// Lit enough for flowers / grass: light >= 8 or at/above the heightmap.
fn lit(w: &World, x: i32, y: i32, z: i32) -> bool {
    y >= w.heightmap(x, z) || w.light(x, y, z) >= 8
}

/// Survival rule of a plant at an (air) position (SPEC 12 table).
pub(crate) fn survives(w: &World, plant: u8, x: i32, y: i32, z: i32) -> bool {
    let below = w.get(x, y - 1, z);
    match plant {
        DEAD_BUSH => below == SAND && lit(w, x, y, z),
        BROWN | RED => (0..128).contains(&y) && is_opaque_cube(below) && w.light(x, y, z) < 13,
        CANE => {
            below == CANE
                || ((below == GRASS || below == DIRT)
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|&(dx, dz)| is_liquid(w.get(x + dx, y - 1, z + dz))))
        }
        CACTUS => {
            (below == SAND || below == CACTUS)
                && ![(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dz)| blocks_movement(w.get(x + dx, y, z + dz)))
        }
        PUMPKIN => below == GRASS,
        // Dandelion, rose, tall grass, fern.
        _ => (below == GRASS || below == DIRT) && lit(w, x, y, z),
    }
}

/// Flower patch (dandelion, rose, mushrooms): 64 trials at spread 8/4.
pub(crate) fn flower_patch(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32, b: u8) -> u32 {
    let mut n = 0;
    for _ in 0..64 {
        let tx = x + spread(rng, 8);
        let ty = y + spread(rng, 4);
        let tz = z + spread(rng, 8);
        if w.get(tx, ty, tz) == AIR && survives(w, b, tx, ty, tz) && w.set(tx, ty, tz, b) {
            n += 1;
        }
    }
    n
}

/// Tall grass / fern (128 trials) and dead bush (4 trials) patches: descend
/// through air and leaves first, then trials at spread 8/4.
pub(crate) fn grass_patch(
    w: &mut World,
    rng: &mut Rng,
    x: i32,
    mut y: i32,
    z: i32,
    b: u8,
    trials: u32,
) -> u32 {
    while y > 0 {
        let c = w.get(x, y, z);
        if c != AIR && !is_leaves(c) {
            break;
        }
        y -= 1;
    }
    let mut n = 0;
    for _ in 0..trials {
        let tx = x + spread(rng, 8);
        let ty = y + spread(rng, 4);
        let tz = z + spread(rng, 8);
        if w.get(tx, ty, tz) == AIR && survives(w, b, tx, ty, tz) && w.set(tx, ty, tz, b) {
            n += 1;
        }
    }
    n
}

/// Stack `b` bottom-up from (x, y, z) up to `h` blocks while each block
/// survives and finds air (B10: we check for air on the upper blocks too).
fn stack(w: &mut World, x: i32, y: i32, z: i32, b: u8, h: i32) -> u32 {
    let mut n = 0;
    for i in 0..h {
        let yy = y + i;
        if w.get(x, yy, z) != AIR || !survives(w, b, x, yy, z) || !w.set(x, yy, z, b) {
            break;
        }
        n += 1;
    }
    n
}

/// Sugar cane: 20 trials at spread 4/1, height 2 + rand(rand(3) + 1).
pub(crate) fn sugar_cane(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32) -> u32 {
    let mut n = 0;
    for _ in 0..20 {
        let tx = x + spread(rng, 4);
        let tz = z + spread(rng, 4);
        if w.get(tx, y, tz) != AIR {
            continue;
        }
        let water = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|&(dx, dz)| is_liquid(w.get(tx + dx, y - 1, tz + dz)));
        if !water {
            continue;
        }
        let inner = rng.below(3) + 1;
        let h = 2 + rng.below(inner) as i32;
        n += stack(w, tx, y, tz, CANE, h);
    }
    n
}

/// Cactus: 10 trials at spread 8/4, height 1 + rand(rand(3) + 1).
pub(crate) fn cactus(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32) -> u32 {
    let mut n = 0;
    for _ in 0..10 {
        let tx = x + spread(rng, 8);
        let ty = y + spread(rng, 4);
        let tz = z + spread(rng, 8);
        if w.get(tx, ty, tz) != AIR {
            continue;
        }
        let inner = rng.below(3) + 1;
        let h = 1 + rng.below(inner) as i32;
        n += stack(w, tx, ty, tz, CACTUS, h);
    }
    n
}

/// Pumpkin patch: 64 trials at spread 8/4 on grass. (Facing is not stored:
/// the palette has one pumpkin id; a viewer can hash the position.)
pub(crate) fn pumpkin_patch(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32) -> u32 {
    let mut n = 0;
    for _ in 0..64 {
        let tx = x + spread(rng, 8);
        let ty = y + spread(rng, 4);
        let tz = z + spread(rng, 8);
        if w.get(tx, ty, tz) == AIR && survives(w, PUMPKIN, tx, ty, tz) {
            let _facing = rng.below(4);
            if w.set(tx, ty, tz, PUMPKIN) {
                n += 1;
            }
        }
    }
    n
}

/// Whether `b` is one of the decoration plants (incl. cactus, excl. pumpkin).
pub(crate) fn is_plant(b: u8) -> bool {
    (Block::TallGrass as u8..=Block::RedMushroom as u8).contains(&b)
}

/// Support rule of an already placed plant (ground and, for cactus, the
/// side-clearance rule; light is not re-checked).
pub(crate) fn supported(w: &World, b: u8, x: i32, y: i32, z: i32) -> bool {
    let below = w.get(x, y - 1, z);
    match b {
        DEAD_BUSH => below == SAND,
        BROWN | RED => is_opaque_cube(below),
        CANE | CACTUS => survives(w, b, x, y, z),
        _ => below == GRASS || below == DIRT,
    }
}

/// Remove plants that lost their support after later features were placed
/// (e.g. leaves replacing the middle of a cane, or a canopy touching a
/// cactus), bottom-up, like the block updates that would break them in game.
pub(crate) fn cleanup_column(w: &mut World, x: i32, z: i32) {
    for y in 1..crate::HEIGHT as i32 {
        let b = w.get(x, y, z);
        if is_plant(b) && !supported(w, b, x, y, z) {
            w.set(x, y, z, AIR);
        }
    }
}
