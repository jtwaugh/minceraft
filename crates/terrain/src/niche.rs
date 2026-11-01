//! Environmental niches for flora (docs/design/swedish-summer-flora.md
//! sections 3 and 4): the layers an entry's `niche` may name and the
//! trapezoid response to one layer's value. Computed so far: height above
//! sea (ground y minus `SEA_LEVEL`), soil depth (dirt and grass blocks
//! from the ground block down, before anything else), light (the share of
//! columns in a small cone around the column with no leaves above the ground),
//! slope and sun-facing (from the surface height of the four neighbours),
//! hollow (surface height minus the mean of rings around the column) and
//! forest edge (distance from an open column to the nearest leaf column) and
//! exposure (open water upwind of the column) and conifer (share of spruce
//! among the leaf columns near the column).

use crate::data::{blocks_movement, is_leaves, is_liquid, Block};
use crate::decorate::World;
use crate::flora::{
    Flora, FLORA_CONIFER_NONE, FLORA_EDGE_RADIUS, FLORA_EXPOSURE_CAP, FLORA_HOLLOW_RADII,
    FLORA_LIGHT_RADIUS, FLORA_SUN, FLORA_SUN_FACING_FLAT, FLORA_TREE_KIND_RADIUS, FLORA_WIND,
    NICHE_MARGIN_CONIFER, NICHE_MARGIN_EDGE, NICHE_MARGIN_EXPOSURE, NICHE_MARGIN_HEIGHT_ABOVE_SEA,
    NICHE_MARGIN_HOLLOW, NICHE_MARGIN_LIGHT, NICHE_MARGIN_SLOPE, NICHE_MARGIN_SOIL_DEPTH,
    NICHE_MARGIN_SUN_FACING,
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
    Edge,
    Exposure,
    Conifer,
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
            NicheLayer::Edge => NICHE_MARGIN_EDGE,
            NicheLayer::Exposure => NICHE_MARGIN_EXPOSURE,
            NicheLayer::Conifer => NICHE_MARGIN_CONIFER,
        }
    }
}

/// Value of a column-only layer for the column at `(x, z)` whose ground block
/// is at `y`.
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
        NicheLayer::Light => Some(light(world, x, y, z)),
        NicheLayer::Hollow => Some(hollow(world, x, z)),
        NicheLayer::Edge => Some(edge(world, x, y, z)),
        NicheLayer::Exposure => Some(exposure(world, x, z)),
        NicheLayer::Conifer => Some(conifer(world, x, y, z)),
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

/// [`surface_height`] at `(x, z)` minus its mean over 8 columns (axis and
/// diagonal directions) on each ring of `FLORA_HOLLOW_RADII`: negative in a
/// hollow, positive on a ridge.
pub(crate) fn hollow(world: &World, x: i32, z: i32) -> f64 {
    let mut sum = 0i32;
    for r in FLORA_HOLLOW_RADII {
        // r / sqrt(2), rounded, so diagonal columns lie about r away too.
        let q = (r * 7071 + 5000) / 10000;
        for (dx, dz) in [
            (r, 0),
            (-r, 0),
            (0, r),
            (0, -r),
            (q, q),
            (q, -q),
            (-q, q),
            (-q, -q),
        ] {
            sum += surface_height(world, x + dx, z + dz);
        }
    }
    let n = (FLORA_HOLLOW_RADII.len() * 8) as f64;
    surface_height(world, x, z) as f64 - sum as f64 / n
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

/// Share of the columns within `FLORA_LIGHT_RADIUS` of `(x, z)` (a circle,
/// the column itself included) with no leaf block above `y`. Flora runs after
/// trees, so their leaves are in place.
pub(crate) fn light(world: &World, x: i32, y: i32, z: i32) -> f64 {
    let r = FLORA_LIGHT_RADIUS;
    let (mut open, mut all) = (0u32, 0u32);
    for dz in -r..=r {
        for dx in -r..=r {
            if dx * dx + dz * dz > r * r {
                continue;
            }
            all += 1;
            if !leafy(world, x + dx, y, z + dz) {
                open += 1;
            }
        }
    }
    open as f64 / all as f64
}

/// Whether the column at `(x, z)` has a leaf block above `y`.
fn leafy(world: &World, x: i32, y: i32, z: i32) -> bool {
    (y + 1..HEIGHT as i32).any(|y| is_leaves(world.get(x, y, z)))
}

/// Straight-line distance, in columns, from `(x, z)` to the nearest column
/// within `FLORA_EDGE_RADIUS` (a circle) with a leaf block above `y`: 0 if
/// `(x, z)` itself has one, `FLORA_EDGE_RADIUS` if no column in reach does.
pub(crate) fn edge(world: &World, x: i32, y: i32, z: i32) -> f64 {
    let r = FLORA_EDGE_RADIUS;
    let mut best = r * r;
    for dz in -r..=r {
        for dx in -r..=r {
            let d = dx * dx + dz * dz;
            if d < best && leafy(world, x + dx, y, z + dz) {
                best = d;
            }
        }
    }
    libm_sqrt(best as f64)
}

/// Share of `FLORA_EXPOSURE_CAP` taken by water columns met walking from
/// `(x, z)` towards `FLORA_WIND`, one rounded step at a time, before the first
/// land column. A column is water where its [`surface_height`] block is liquid.
pub(crate) fn exposure(world: &World, x: i32, z: i32) -> f64 {
    // Round to nearest; the offset keeps the truncation a floor.
    let round = |v: f64| (v + 1024.5) as i32 - 1024;
    let mut water = 0;
    for i in 1..=FLORA_EXPOSURE_CAP {
        let (wx, wz) = (
            round(i as f64 * FLORA_WIND.0),
            round(i as f64 * FLORA_WIND.1),
        );
        let (px, pz) = (x + wx, z + wz);
        let h = surface_height(world, px, pz);
        if h < 0 || !is_liquid(world.get(px, h, pz)) {
            break;
        }
        water += 1;
    }
    water as f64 / FLORA_EXPOSURE_CAP as f64
}

/// Share of spruce among the leaf columns within `FLORA_TREE_KIND_RADIUS` of
/// `(x, z)` (a circle): columns with a leaf block above `y`, spruce where the
/// topmost leaf is spruce leaves. `FLORA_CONIFER_NONE` if there is none.
pub(crate) fn conifer(world: &World, x: i32, y: i32, z: i32) -> f64 {
    let r = FLORA_TREE_KIND_RADIUS;
    let (mut spruce, mut all) = (0u32, 0u32);
    for dz in -r..=r {
        for dx in -r..=r {
            if dx * dx + dz * dz > r * r {
                continue;
            }
            let (px, pz) = (x + dx, z + dz);
            let top = (y + 1..HEIGHT as i32)
                .rev()
                .map(|y| world.get(px, y, pz))
                .find(|&b| is_leaves(b));
            if let Some(b) = top {
                all += 1;
                spruce += (b == Block::SpruceLeaves as u8) as u32;
            }
        }
    }
    if all == 0 {
        FLORA_CONIFER_NONE
    } else {
        spruce as f64 / all as f64
    }
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
