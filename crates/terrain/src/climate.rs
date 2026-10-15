//! Climate (temperature, humidity) and biome lookup (SPEC 2, 7, 8).
//!
//! This is the **one** climate source: the density field, the surface pass,
//! sea ice, the decoration pass and the snow pass all read [`Climate::at`]
//! (SPEC 7). Temperature bias terms are applied inside it, so every use sees
//! the same biased temperature.

use crate::data::{lookup, Biome};
use crate::noise::{floor, OctaveSimplex, Rng};

// ===========================================================================
// EXTENSION POINT: temperature bias terms
// ===========================================================================
//
// Every bias function receives WORLD block column coordinates (x, z) and
// returns a value added to the column's temperature T (after the SPEC 7
// formula, before clamping to [0, 1]). Biased T is what density, the biome
// lookup, sea ice, decoration and snow all see. Default: no biases.

/// A temperature bias term `f(x, z) -> f64` in world block coordinates.
pub trait TemperatureBias {
    fn bias(&self, x: i32, z: i32) -> f64;
}

impl<F: Fn(i32, i32) -> f64> TemperatureBias for F {
    #[inline]
    fn bias(&self, x: i32, z: i32) -> f64 {
        self(x, z)
    }
}

/// The three climate noise generators for one seed.
pub struct Climate {
    temp: OctaveSimplex,
    humid: OctaveSimplex,
    blend: OctaveSimplex,
}

impl Climate {
    pub fn new(seed: u64) -> Climate {
        let c = |m: u64, n: usize| {
            OctaveSimplex::new(&mut Rng::for_generator(seed.wrapping_mul(m), 0x5EED_C11A), n)
        };
        Climate { temp: c(9871, 4), humid: c(39811, 4), blend: c(543321, 2) }
    }

    /// Unbiased `(T, H)` at world block column (x, z) (SPEC 7).
    pub fn raw(&self, x: i32, z: i32) -> (f64, f64) {
        let (x, z) = (x as f64, z as f64);
        let tn = self.temp.sample(x, z, 0.025, 0.025, 0.25, 0.5);
        let bn = self.blend.sample(x, z, 0.25, 0.25, 1.0 / 1.7, 0.5);
        let hn = self.humid.sample(x, z, 0.05, 0.05, 1.0 / 3.0, 0.5);
        let b = bn * 1.1 + 0.5;
        let mut t = (tn * 0.15 + 0.7) * 0.99 + b * 0.01;
        t = 1.0 - (1.0 - t) * (1.0 - t);
        let h = (hn * 0.15 + 0.5) * 0.998 + b * 0.002;
        (t.clamp(0.0, 1.0), h.clamp(0.0, 1.0))
    }

    /// `(T, H)` with temperature bias terms applied (T clamped to [0, 1]).
    #[inline]
    pub fn at(&self, x: i32, z: i32, tb: &[&dyn TemperatureBias]) -> (f64, f64) {
        let (mut t, h) = self.raw(x, z);
        if !tb.is_empty() {
            for b in tb {
                t += b.bias(x, z);
            }
            t = t.clamp(0.0, 1.0);
        }
        (t, h)
    }
}

/// `(T, H)` at world column (x, z) for `seed` (no biases). Builds the noise
/// generators on each call; use [`Climate`] for bulk queries.
pub fn climate_at(seed: u64, x: i32, z: i32) -> (f64, f64) {
    Climate::new(seed).raw(x, z)
}

/// Biome at world column (x, z) for `seed` (no biases).
pub fn biome_at(seed: u64, x: i32, z: i32) -> Biome {
    let (t, h) = climate_at(seed, x, z);
    biome_for(t, h)
}

/// Biome for an (unquantised) climate: quantise to the 64x64 table (B1).
#[inline]
pub fn biome_for(t: f64, h: f64) -> Biome {
    let i = floor(t * 63.0) as i32;
    let j = floor(h * 63.0) as i32;
    biome_lookup(i.clamp(0, 63) as u32, j.clamp(0, 63) as u32)
}

/// Table entry (i, j), i.e. temperature `i/63`, humidity `j/63` (SPEC 8.2).
/// Thresholds come from `data/biomes.toml`.
pub fn biome_lookup(i: u32, j: u32) -> Biome {
    use lookup::*;
    let t = i as f64 / 63.0;
    let h = j as f64 / 63.0;
    let w = t * h;
    if t < COLD_T {
        Biome::Tundra
    } else if w < DRY_W && t < COOL_T {
        Biome::Tundra
    } else if w < DRY_W && t < HOT_T {
        Biome::Savanna
    } else if w < DRY_W {
        Biome::Desert
    } else if w > SWAMP_W && t < SWAMP_T {
        Biome::Swampland
    } else if t < COOL_T {
        Biome::Taiga
    } else if t < VERY_HOT_T && w < SHRUB_W {
        Biome::Shrubland
    } else if t < VERY_HOT_T {
        Biome::Forest
    } else if w < PLAINS_W {
        Biome::Plains
    } else if w < SEASONAL_W {
        Biome::SeasonalForest
    } else {
        Biome::Rainforest
    }
}
