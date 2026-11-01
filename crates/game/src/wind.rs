//! Wind noise layers, seeded apart from terrain and climate.
//! See docs/design/wind.md Phase A (Reconciliation notes, "Wind-specific
//! seed"); the Simplex primitive is SPEC 1.4.

use terrain::noise::{Rng, Simplex};

/// XORed into the world seed for the pressure layer.
const PRESSURE_SEED: u64 = 0x57_49_4e_44_50_52_45_53; // "WINDPRES"
/// XORed into the world seed for the gust layer.
const GUST_SEED: u64 = 0x57_49_4e_44_47_55_53_54; // "WINDGUST"

/// The two noise fields the wind is built from.
pub struct WindNoise {
    pressure: Simplex,
    gust: Simplex,
}

impl WindNoise {
    pub fn new(seed: u64) -> WindNoise {
        WindNoise {
            pressure: Simplex::new(&mut Rng::new(seed ^ PRESSURE_SEED)),
            gust: Simplex::new(&mut Rng::new(seed ^ GUST_SEED)),
        }
    }

    pub fn pressure(&self, x: f64, z: f64) -> f64 {
        self.pressure.noise(x, z)
    }

    pub fn gust(&self, x: f64, z: f64) -> f64 {
        self.gust.noise(x, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain::climate::climate_at;

    const POINTS: [(f64, f64); 4] = [(0.3, 0.7), (12.5, -4.2), (-80.1, 33.3), (1000.9, 250.4)];

    fn samples(w: &WindNoise) -> Vec<(f64, f64)> {
        POINTS
            .iter()
            .map(|&(x, z)| (w.pressure(x, z), w.gust(x, z)))
            .collect()
    }

    #[test]
    fn same_seed_same_samples() {
        assert_eq!(
            samples(&WindNoise::new(12345)),
            samples(&WindNoise::new(12345))
        );
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(
            samples(&WindNoise::new(12345)),
            samples(&WindNoise::new(54321))
        );
    }

    #[test]
    fn layers_differ() {
        let w = WindNoise::new(12345);
        assert!(POINTS
            .iter()
            .any(|&(x, z)| w.pressure(x, z) != w.gust(x, z)));
    }

    #[test]
    fn climate_unchanged_by_wind() {
        let climate = |seed| -> Vec<(f64, f64)> {
            (-3..3).map(|i| climate_at(seed, i * 97, i * -41)).collect()
        };
        let before = climate(12345);
        let w = WindNoise::new(12345);
        samples(&w);
        assert_eq!(before, climate(12345));
    }
}
