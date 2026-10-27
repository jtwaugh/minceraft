//! Flora table, generated at compile time from `data/flora.toml` (see
//! `build.rs`), and the column-local placement pass that reads it
//! (docs/design/biome-flora.md D.2).

use crate::data::{blocks_movement, is_liquid, Biome, Block};
use crate::decorate::World;
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
    /// Chance per eligible column, by biome.
    pub biomes: &'static [(Biome, f64)],
}

include!(concat!(env!("OUT_DIR"), "/flora.rs"));

/// Low-frequency patch noise for one entry, from the seed and its salt.
fn patch_noise(seed: u64, f: &Flora) -> OctavePerlin {
    let key = mix64(seed ^ ((FLORA_PATCH_NOISE_SALT as u64) << 32 | f.salt as u64));
    OctavePerlin::new(&mut Rng::new(key), 2)
}

/// Place single-block flora (`cross`, `ground_cover`, `bush`) over the
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
    for z in z0..z1 {
        for x in x0..x1 {
            let Some(top) = surface(world, x, z) else {
                continue;
            };
            let ground = Block::from_u8(world.get(x, top - 1, z));
            let biome = world.biome(x, z);
            for (i, f) in table.iter().enumerate() {
                if !matches!(
                    f.shape,
                    FloraShape::Cross | FloraShape::GroundCover | FloraShape::Bush
                ) || !f.ground.contains(&ground)
                {
                    continue;
                }
                let Some(&(_, chance)) = f.biomes.iter().find(|(b, _)| *b == biome) else {
                    continue;
                };
                let n = noises[i].sample2d(x as f64 / f.cluster_scale, z as f64 / f.cluster_scale);
                let patch = (n * 0.5 + 0.5).clamp(0.0, 1.0) * 2.0;
                if hash_f64(seed, x, z, 0, f.salt as u64) < chance * patch
                    && world.set(x, top, z, f.block as u8)
                {
                    placed[i] += 1;
                    break;
                }
            }
        }
    }
    placed
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
        let (mut a, mut b) = (entry(), entry());
        a.name = "a".into();
        b.name = "b".into();
        assert!(check(&[a, b]).unwrap_err().contains("repeats"));
    }
}
