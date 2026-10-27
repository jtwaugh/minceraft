//! Flora table, generated at compile time from `data/flora.toml` (see
//! `build.rs`). Nothing reads it yet; the placement pass comes later.

use crate::data::{Biome, Block};

/// How a plant is drawn and stacked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloraShape {
    Cross,
    TallCross,
    GroundCover,
    Pad,
    Bush,
}

/// Which colour ramp tints the plant.
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
        let (mut a, mut b) = (entry(), entry());
        a.name = "a".into();
        b.name = "b".into();
        assert!(check(&[a, b]).unwrap_err().contains("repeats"));
    }
}
