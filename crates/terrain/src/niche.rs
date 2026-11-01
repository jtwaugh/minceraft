//! Environmental niches for flora (docs/design/swedish-summer-flora.md
//! sections 3 and 4): the layers an entry's `niche` may name and the
//! trapezoid response to one layer's value. Computed so far: height above
//! sea (ground y minus `SEA_LEVEL`), soil depth (dirt and grass blocks
//! from the ground block down, before anything else), slope and sun-facing
//! (from the surface height of the four neighbours).

use crate::data::{blocks_movement, is_leaves, is_liquid, Block};
use crate::decorate::World;
use crate::flora::{
    Flora, FLORA_SUN, FLORA_SUN_FACING_FLAT, NICHE_MARGIN_HEIGHT_ABOVE_SEA, NICHE_MARGIN_HOLLOW,
    NICHE_MARGIN_LIGHT, NICHE_MARGIN_SLOPE, NICHE_MARGIN_SOIL_DEPTH, NICHE_MARGIN_SUN_FACING,
};
use crate::trees::libm_sqrt;
use crate::{HEIGHT, SEA_LEVEL};
use alloc::vec::Vec;

/// A column layer a `niche` range applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NicheLayer {
    HeightAboveSea,
    SoilDepth,
    Light,
    Slope,
    SunFacing,
    Hollow,
}

impl NicheLayer {
    /// Soft margin from `[settings]` `niche_margin_<layer>` in `data/flora.toml`.
    pub fn margin(self) -> f64 {
        match self {
            NicheLayer::HeightAboveSea => NICHE_MARGIN_HEIGHT_ABOVE_SEA,
            NicheLayer::SoilDepth => NICHE_MARGIN_SOIL_DEPTH,
            NicheLayer::Light => NICHE_MARGIN_LIGHT,
            NicheLayer::Slope => NICHE_MARGIN_SLOPE,
            NicheLayer::SunFacing => NICHE_MARGIN_SUN_FACING,
            NicheLayer::Hollow => NICHE_MARGIN_HOLLOW,
        }
    }
}

/// Value of a column-only layer for the column at `(x, z)` whose ground block
/// is at `y`; `None` for a layer not computed yet, which leaves the chance as is.
pub(crate) fn column(world: &World, layer: NicheLayer, x: i32, y: i32, z: i32) -> Option<f64> {
    match layer {
        NicheLayer::HeightAboveSea => Some((y - SEA_LEVEL) as f64),
        NicheLayer::SoilDepth => {
            let soil = |y: &i32| {
                matches!(
                    Block::from_u8(world.get(x, *y, z)),
                    Block::Dirt | Block::Grass
                )
            };
            Some((0..=y).rev().take_while(soil).count() as f64)
        }
        NicheLayer::Slope => {
            let (gx, gz) = gradient(world, x, z);
            Some(libm_sqrt(gx * gx + gz * gz))
        }
        NicheLayer::SunFacing => {
            let (gx, gz) = gradient(world, x, z);
            let len = libm_sqrt(gx * gx + gz * gz);
            if len == 0.0 {
                return Some(FLORA_SUN_FACING_FLAT);
            }
            // Downhill is minus the gradient; its cosine with the sun, as 0 to 1.
            let cos = -(gx * FLORA_SUN.0 + gz * FLORA_SUN.1) / len;
            Some((1.0 + cos) * 0.5)
        }
        _ => None,
    }
}

/// Central-difference gradient `(d/dx, d/dz)` of [`surface_height`] at `(x, z)`.
fn gradient(world: &World, x: i32, z: i32) -> (f64, f64) {
    let h = |x, z| surface_height(world, x, z) as f64;
    (
        (h(x + 1, z) - h(x - 1, z)) * 0.5,
        (h(x, z + 1) - h(x, z - 1)) * 0.5,
    )
}

/// The highest movement-blocking or liquid block of a column, skipping
/// leaves, logs and plants so that trees and flora leave it unchanged; -1 if none.
pub(crate) fn surface_height(world: &World, x: i32, z: i32) -> i32 {
    let terrain = |&y: &i32| {
        let b = world.get(x, y, z);
        (blocks_movement(b) || is_liquid(b))
            && !is_leaves(b)
            && !matches!(
                Block::from_u8(b),
                Block::OakLog | Block::BirchLog | Block::SpruceLog | Block::Cactus | Block::Pumpkin
            )
    };
    (0..HEIGHT as i32).rev().find(terrain).unwrap_or(-1)
}

/// Product of `f`'s niche responses at the column whose air cell is `top`.
/// Layer values are cached per column in `cache` as `(layer, top, value)`.
pub(crate) fn fit(
    world: &World,
    f: &Flora,
    (x, top, z): (i32, i32, i32),
    cache: &mut Vec<Cached>,
) -> f64 {
    f.niche
        .iter()
        .map(|&(layer, lo, hi)| {
            let v = match cache.iter().find(|c| c.0 == layer && c.1 == top) {
                Some(c) => c.2,
                None => {
                    let v = column(world, layer, x, top - 1, z);
                    cache.push((layer, top, v));
                    v
                }
            };
            v.map_or(1.0, |v| response(v, lo, hi, layer.margin()))
        })
        .product()
}

/// One cached layer value: the layer, the air cell it was read for, the value.
pub type Cached = (NicheLayer, i32, Option<f64>);

/// 1 for `value` in `[lo, hi]`, falling linearly to 0 at `margin` outside it.
pub fn response(value: f64, lo: f64, hi: f64, margin: f64) -> f64 {
    let out = (lo - value).max(value - hi).max(0.0);
    (1.0 - out / margin).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trapezoid() {
        let r = |v| response(v, 2.0, 4.0, 1.0);
        assert_eq!(r(3.0), 1.0);
        assert_eq!(r(2.0), 1.0);
        assert_eq!(r(4.0), 1.0);
        assert_eq!(r(1.5), 0.5);
        assert_eq!(r(4.5), 0.5);
        assert_eq!(r(1.0), 0.0);
        assert_eq!(r(5.0), 0.0);
        assert_eq!(r(-10.0), 0.0);
        assert_eq!(r(10.0), 0.0);
    }

    #[test]
    fn margins_come_from_data() {
        assert!(NicheLayer::Light.margin() > 0.0);
    }
}
