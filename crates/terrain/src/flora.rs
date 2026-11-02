//! Flora table, generated at compile time from `data/flora.toml` (see
//! `build.rs`), and the column-local placement pass that reads it
//! (docs/design/biome-flora.md D.2).

use crate::data::{blocks_movement, is_leaves, is_liquid, Biome, Block};
use crate::decorate::World;
use crate::niche::{self, NicheLayer};
use crate::noise::{hash_f64, mix64, OctavePerlin, Rng};
use crate::{AIR, HEIGHT};
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloraShape {
    Cross,
    TallCross,
    GroundCover,
    Pad,
    Bush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloraTint {
    None,
    Grass,
    Leaf,
    Dry,
}

/// A column condition an entry may require (`requires` in `flora.toml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloraRequires {
    /// A water block beside the ground block, within `FLORA_NEAR_WATER_RADIUS`.
    NearWater,
    /// Grass with air above it and leaves higher in the same column; the
    /// entry stands on that grass, not on the column's top block.
    Canopy,
    /// Sand at y `FLORA_COAST_MIN_Y` to `FLORA_COAST_MAX_Y` with water at the
    /// sand's y or one below in any column of the square `FLORA_COAST_RADIUS`
    /// columns around it (Chebyshev distance).
    Coast,
}

/// One plant species.
#[derive(Clone, Copy, Debug)]
pub struct Flora {
    pub name: &'static str,
    /// The block (the lower half of a two-block plant).
    pub block: Block,
    /// The upper half of a `TallCross` plant.
    pub upper: Option<Block>,
    pub shape: FloraShape,
    /// Blocks the plant may stand on.
    pub ground: &'static [Block],
    pub tint: FloraTint,
    /// Blocks per noise cell for patchiness.
    pub cluster_scale: f64,
    /// Fixed per-entry salt for the hash and noise.
    pub salt: u32,
    /// Extra condition on the column; `None` means always eligible.
    pub requires: Option<FloraRequires>,
    /// A `cross` or `tall_cross` plant the snow pass leaves standing, with no snow in its cell.
    pub snow_stays: bool,
    /// Chance per eligible column, by biome.
    pub biomes: &'static [(Biome, f64)],
    /// Preferred `[lo, hi]` range per layer; empty for an entry without a niche.
    pub niche: &'static [(NicheLayer, f64, f64)],
}

include!(concat!(env!("OUT_DIR"), "/flora.rs"));

/// Low-frequency patch noise for one entry, from the seed and its salt.
fn patch_noise(seed: u64, f: &Flora) -> OctavePerlin {
    let key = mix64(seed ^ ((FLORA_PATCH_NOISE_SALT as u64) << 32 | f.salt as u64));
    OctavePerlin::new(&mut Rng::new(key), 2)
}

/// Low-frequency noise that picks the winner among competing entries.
fn colour_noise(seed: u64) -> OctavePerlin {
    let key = mix64(seed ^ ((FLORA_COLOUR_NOISE_SALT as u64) << 32));
    OctavePerlin::new(&mut Rng::new(key), 2)
}

/// Place flora (`cross`, `tall_cross`, `ground_cover`, `bush`, and `pad` on water) over the
/// columns `[x0, x1) x [z0, z1)`. Each column reads only itself, the seed and
/// the noise, and writes only into air, so the result is independent of the
/// region and the cell RNG is untouched. Returns blocks placed per entry.
pub(crate) fn place(
    world: &mut World,
    seed: u64,
    (x0, z0, x1, z1): (i32, i32, i32, i32),
    table: &[Flora],
) -> Vec<u64> {
    let mut placed = alloc::vec![0u64; table.len()];
    let noises: Vec<OctavePerlin> = table.iter().map(|f| patch_noise(seed, f)).collect();
    let colour = colour_noise(seed);
    for z in z0..z1 {
        for x in x0..x1 {
            let Some(top) = surface(world, x, z) else {
                place_pad(world, seed, (x, z), table, &noises, &mut placed);
                continue;
            };
            let ground = Block::from_u8(world.get(x, top - 1, z));
            let biome = world.biome(x, z);
            // Entries that could grow here, with their chance in this biome.
            let mut eligible: Vec<(usize, f64, i32)> = Vec::new();
            let mut under = None;
            let mut layers = Vec::new();
            for (i, f) in table.iter().enumerate() {
                let (top, ground) = if f.requires == Some(FloraRequires::Canopy) {
                    match *under.get_or_insert_with(|| under_canopy(world, x, z)) {
                        Some(y) => (y, Block::Grass),
                        None => continue,
                    }
                } else {
                    (top, ground)
                };
                if !matches!(
                    f.shape,
                    FloraShape::Cross
                        | FloraShape::TallCross
                        | FloraShape::GroundCover
                        | FloraShape::Bush
                ) || !f.ground.contains(&ground)
                    || !f.requires.is_none_or(|r| holds(world, r, x, top - 1, z))
                {
                    continue;
                }
                let Some(&(_, mut chance)) = f.biomes.iter().find(|(b, _)| *b == biome) else {
                    continue;
                };
                if !f.niche.is_empty() {
                    chance *= niche::fit(world, f, (x, top, z), &mut layers);
                    if chance <= 0.0 {
                        continue;
                    }
                }
                eligible.push((i, chance, top));
            }
            // The colour noise picks one winner, weighted by chance, so
            // neighbouring columns favour the same entry.
            let total: f64 = eligible.iter().map(|e| e.1).sum();
            let mut pick = eligible.last().copied();
            if eligible.len() > 1 && total > 0.0 {
                let c =
                    colour.sample2d(x as f64 / FLORA_COLOUR_SCALE, z as f64 / FLORA_COLOUR_SCALE);
                let mut at = (c * 0.5 + 0.5).clamp(0.0, 1.0) * total;
                for &e in &eligible {
                    pick = Some(e);
                    at -= e.1;
                    if at < 0.0 {
                        break;
                    }
                }
            }
            let Some((i, chance, top)) = pick else {
                continue;
            };
            let f = &table[i];
            let n = noises[i].sample2d(x as f64 / f.cluster_scale, z as f64 / f.cluster_scale);
            let patch = (n * 0.5 + 0.5).clamp(0.0, 1.0) * 2.0;
            if hash_f64(seed, x, z, 0, f.salt as u64) < chance * patch {
                let ok = match (f.shape, f.upper) {
                    (FloraShape::TallCross, Some(upper)) => place_tall(world, x, top, z, f, upper),
                    (FloraShape::TallCross, None) => false,
                    _ => world.set(x, top, z, f.block as u8),
                };
                placed[i] += ok as u64;
            }
        }
    }
    placed
}

/// Float a `pad` entry on a column whose top block is water with air above,
/// when land (a solid, non-liquid block at the water's height) lies within
/// `FLORA_PAD_LAND_RADIUS` columns. Same patch noise and hash roll as `place`.
fn place_pad(
    world: &mut World,
    seed: u64,
    (x, z): (i32, i32),
    table: &[Flora],
    noises: &[OctavePerlin],
    placed: &mut [u64],
) {
    let Some(top) = (1..HEIGHT as i32 - 1)
        .rev()
        .find(|&y| world.get(x, y, z) != AIR)
        .map(|y| y + 1)
    else {
        return;
    };
    if Block::from_u8(world.get(x, top - 1, z)) != Block::Water {
        return;
    }
    let biome = world.biome(x, z);
    let n = FLORA_PAD_LAND_RADIUS;
    let land = |dx: i32, dz: i32| {
        let b = world.get(x + dx, top - 1, z + dz);
        blocks_movement(b) && !is_liquid(b)
    };
    let mut near = None;
    for (i, f) in table.iter().enumerate() {
        if f.shape != FloraShape::Pad || !f.ground.contains(&Block::Water) {
            continue;
        }
        let Some(&(_, chance)) = f.biomes.iter().find(|(b, _)| *b == biome) else {
            continue;
        };
        if !*near.get_or_insert_with(|| (-n..=n).any(|dz| (-n..=n).any(|dx| land(dx, dz)))) {
            return;
        }
        let v = noises[i].sample2d(x as f64 / f.cluster_scale, z as f64 / f.cluster_scale);
        let patch = (v * 0.5 + 0.5).clamp(0.0, 1.0) * 2.0;
        if hash_f64(seed, x, z, 0, f.salt as u64) < chance * patch {
            placed[i] += world.set(x, top, z, f.block as u8) as u64;
            return;
        }
    }
}

/// Whether the column at `(x, z)`, whose ground block is at `y`, meets `r`.
fn holds(world: &World, r: FloraRequires, x: i32, y: i32, z: i32) -> bool {
    match r {
        FloraRequires::Canopy => true,
        FloraRequires::Coast => {
            let n = FLORA_COAST_RADIUS;
            world.get(x, y, z) == Block::Sand as u8
                && (FLORA_COAST_MIN_Y..=FLORA_COAST_MAX_Y).contains(&y)
                && (-n..=n).any(|dz| {
                    (-n..=n).any(|dx| {
                        (y - 1..=y).any(|wy| world.get(x + dx, wy, z + dz) == Block::Water as u8)
                    })
                })
        }
        FloraRequires::NearWater => {
            let n = FLORA_NEAR_WATER_RADIUS;
            (-n..=n).any(|dz| {
                (-n..=n).any(|dx| (dx, dz) != (0, 0) && is_liquid(world.get(x + dx, y, z + dz)))
            })
        }
    }
}

/// Height of the air cell above the highest grass block that has air directly
/// above it and leaves somewhere higher in the column.
fn under_canopy(world: &World, x: i32, z: i32) -> Option<i32> {
    let mut leaves = false;
    for y in (1..HEIGHT as i32 - 1).rev() {
        let b = world.get(x, y, z);
        if leaves && b == Block::Grass as u8 && world.get(x, y + 1, z) == AIR {
            return Some(y + 1);
        }
        leaves |= is_leaves(b);
    }
    None
}

/// Place both halves of a two-block plant, only if the cell above the lower
/// one is inside the world. `surface` has already found the lower cell open;
/// every cell above the highest solid block is air. Never leaves a lone half.
fn place_tall(world: &mut World, x: i32, y: i32, z: i32, f: &Flora, upper: Block) -> bool {
    if y + 1 >= HEIGHT as i32 {
        return false;
    }
    if !world.set(x, y + 1, z, upper as u8) {
        return false;
    }
    if world.set(x, y, z, f.block as u8) {
        return true;
    }
    world.set(x, y + 1, z, AIR);
    false
}

/// Height of the air cell above a column's highest solid block, if the cell
/// is air and the block below is not liquid.
fn surface(world: &World, x: i32, z: i32) -> Option<i32> {
    for y in (1..HEIGHT as i32).rev() {
        let b = world.get(x, y, z);
        if blocks_movement(b) || is_liquid(b) {
            let top = y + 1;
            let open = top < HEIGHT as i32 && world.get(x, top, z) == AIR && !is_liquid(b);
            return open.then_some(top);
        }
    }
    None
}

#[cfg(test)]
#[path = "../flora_check.rs"]
mod flora_check;

#[cfg(test)]
mod tests {
    use super::*;

    /// A 21 by 21 world with sand at (10, sand_y, 10) and one water block at `water`.
    fn coast(sand_y: i32, water: (i32, i32, i32)) -> bool {
        let mut blocks = alloc::vec![0u8; 21 * 21 * HEIGHT];
        let mut w = World::for_test(&mut blocks, 21, 21);
        w.set(10, sand_y, 10, Block::Sand as u8);
        w.set(water.0, water.1, water.2, Block::Water as u8);
        holds(&w, FloraRequires::Coast, 10, sand_y, 10)
    }

    #[test]
    fn coast_reaches_three_columns_in_a_square() {
        assert!(coast(64, (13, 64, 13)));
        assert!(coast(64, (7, 63, 10)));
        assert!(!coast(64, (14, 64, 10)));
        assert!(!coast(64, (10, 64, 14)));
        assert!(!coast(64, (14, 64, 14)));
    }

    #[test]
    fn coast_water_must_be_at_sand_y_or_one_below() {
        assert!(coast(64, (11, 64, 10)));
        assert!(coast(64, (11, 63, 10)));
        assert!(!coast(64, (11, 65, 10)));
        assert!(!coast(64, (11, 62, 10)));
    }

    #[test]
    fn coast_sand_must_be_at_y_62_to_66() {
        for y in [62, 66] {
            assert!(coast(y, (11, y, 10)), "{y}");
        }
        for y in [61, 67] {
            assert!(!coast(y, (11, y, 10)), "{y}");
        }
    }

    use super::flora_check::{validate, RawFlora};

    #[test]
    fn daisy_resolves() {
        let d = FLORA.iter().find(|f| f.name == "daisy").unwrap();
        assert_eq!(d.block, Block::Daisy);
        assert_eq!(d.shape, FloraShape::Cross);
        assert_eq!(d.ground, &[Block::Grass]);
        assert_eq!(d.tint, FloraTint::None);
        assert_eq!(d.cluster_scale, 24.0);
        assert!(d.biomes.contains(&(Biome::Plains, 0.020)));
        assert!(d.biomes.contains(&(Biome::Forest, 0.004)));
        assert_eq!(d.biomes.len(), 2);
        assert!(d.niche.is_empty());
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn entry() -> RawFlora {
        RawFlora {
            name: "x".into(),
            block: names(&["daisy"]),
            ground: names(&["grass"]),
            shape: "cross".into(),
            tint: "none".into(),
            salt: 1,
            biomes: vec![("Plains".into(), 0.5)],
            requires: None,
            snow_stays: false,
            niche: Vec::new(),
        }
    }

    fn check(e: &[RawFlora]) -> Result<(), String> {
        validate(e, &names(&["daisy", "grass", "dirt"]), &names(&["Plains"]))
    }

    #[test]
    fn validator_accepts_good_entry() {
        assert!(check(&[entry()]).is_ok());
    }

    #[test]
    fn validator_rejects_bad_input() {
        let mut e = entry();
        e.block = names(&["nope"]);
        assert!(check(&[e]).unwrap_err().contains("unknown block"));
        let mut e = entry();
        e.ground = names(&["nope"]);
        assert!(check(&[e]).unwrap_err().contains("unknown block"));
        let mut e = entry();
        e.biomes = vec![("Moon".into(), 0.1)];
        assert!(check(&[e]).unwrap_err().contains("unknown biome"));
        for bad in [1.5, -0.1] {
            let mut e = entry();
            e.biomes = vec![("Plains".into(), bad)];
            assert!(check(&[e]).unwrap_err().contains("outside 0..=1"));
        }
        let mut e = entry();
        e.shape = "tree".into();
        assert!(check(&[e]).unwrap_err().contains("bad shape"));
        let mut e = entry();
        e.tint = "blue".into();
        assert!(check(&[e]).unwrap_err().contains("bad shape"));
        let mut e = entry();
        e.shape = "tall_cross".into();
        assert!(check(&[e]).unwrap_err().contains("needs 2"));
        let mut e = entry();
        e.block = names(&["daisy", "daisy"]);
        assert!(check(&[e]).unwrap_err().contains("needs 1"));
        let mut e = entry();
        e.requires = Some("near_coast".into());
        assert!(check(&[e]).unwrap_err().contains("unknown requires"));
        let mut e = entry();
        e.requires = Some("near_water".into());
        assert!(check(&[e]).is_ok());
        let mut e = entry();
        e.niche = vec![("dampness".into(), vec![Some(0.0), Some(1.0)])];
        assert!(check(&[e]).unwrap_err().contains("unknown niche layer"));
        let mut e = entry();
        e.niche = vec![("light".into(), vec![Some(0.9), Some(0.5)])];
        assert!(check(&[e]).unwrap_err().contains("lo > hi"));
        for bad in [vec![Some(0.5)], vec![None, Some(0.5)], vec![]] {
            let mut e = entry();
            e.niche = vec![("slope".into(), bad)];
            assert!(check(&[e]).unwrap_err().contains("two numbers"));
        }
        for layer in ["light", "edge", "exposure"] {
            let mut e = entry();
            e.niche = vec![(layer.into(), vec![Some(0.5), Some(0.5)])];
            assert!(check(&[e]).is_ok());
        }
        let (mut a, mut b) = (entry(), entry());
        a.name = "a".into();
        b.name = "b".into();
        assert!(check(&[a, b]).unwrap_err().contains("repeats"));
    }
}
