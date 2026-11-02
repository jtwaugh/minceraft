//! Bounded cache of roughness cells.
//! See docs/design/biome-roughness-wind.md Phase A.
//!
//! A pure memo over `fill_cell`, keyed by `(cx, cz)` for one seed. It is not
//! sim state: eviction (oldest fill first) only decides what is recomputed;
//! a miss refills exactly the same numbers, so no value depends on cache
//! state. This is the place the land-sea breeze's land fraction (#1750) joins:
//! it adds its field to the same fill, not a second cache.

use crate::wind_roughness::{fill_cell, RoughnessCell, RoughnessParams};

pub struct RoughnessCache {
    seed: u64,
    params: RoughnessParams,
    capacity: usize,
    /// Oldest first.
    cells: Vec<((i32, i32), RoughnessCell)>,
}

impl RoughnessCache {
    /// A cache holding at most `capacity` cells (at least one).
    pub fn new(seed: u64, params: RoughnessParams, capacity: usize) -> Self {
        RoughnessCache {
            seed,
            params,
            capacity: capacity.max(1),
            cells: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Bilinear `R` at world `(x, z)`, filling the cell on a miss.
    pub fn lookup(&mut self, x: f64, z: f64) -> f64 {
        let size = self.params.cell_size as f64;
        let key = ((x / size).floor() as i32, (z / size).floor() as i32);
        let at = match self.cells.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                if self.cells.len() == self.capacity {
                    self.cells.remove(0);
                }
                let cell = fill_cell(self.seed, &self.params, key.0, key.1);
                self.cells.push((key, cell));
                self.cells.len() - 1
            }
        };
        self.cells[at].1.at(x, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wind_roughness::roughness_at;

    const SEED: u64 = 4242;

    fn params() -> RoughnessParams {
        RoughnessParams {
            biome: (0..terrain::BIOME_INFO.len())
                .map(|i| i as f64 / 10.0)
                .collect(),
            water: 0.05,
            cell_size: 16,
            step: 4,
            radius: 8,
        }
    }

    fn points() -> Vec<(f64, f64)> {
        let mut v = Vec::new();
        for i in -5..6 {
            for j in -4..5 {
                v.push((i as f64 * 13.7, j as f64 * 11.9));
            }
        }
        // Exactly on cell edges and corners.
        v.extend([(0.0, 0.0), (16.0, 16.0), (-16.0, 32.0), (16.0, 10.0)]);
        v
    }

    fn cache(cap: usize) -> RoughnessCache {
        RoughnessCache::new(SEED, params(), cap)
    }

    #[test]
    fn cold_warm_and_evicting_caches_match_uncached() {
        let pts = points();
        let p = params();
        let mut big = cache(10_000);
        let cold: Vec<_> = pts.iter().map(|&(x, z)| big.lookup(x, z)).collect();
        let warm: Vec<_> = pts.iter().map(|&(x, z)| big.lookup(x, z)).collect();
        assert_eq!(cold, warm);
        for (i, &(x, z)) in pts.iter().enumerate() {
            assert_eq!(cold[i], roughness_at(SEED, &p, x, z));
        }
        for cap in [1, 2] {
            let mut small = cache(cap);
            for (i, &(x, z)) in pts.iter().enumerate() {
                assert_eq!(small.lookup(x, z), cold[i], "cap {cap}");
                assert!(small.len() <= cap);
            }
            for (i, &(x, z)) in pts.iter().enumerate().rev() {
                assert_eq!(small.lookup(x, z), cold[i], "cap {cap} reverse");
            }
        }
    }

    #[test]
    fn capacity_is_respected_and_zero_means_one() {
        let mut c = cache(3);
        assert!(c.is_empty());
        for &(x, z) in &points() {
            c.lookup(x, z);
            assert!(c.len() <= 3);
        }
        assert_eq!(c.len(), 3);
        let mut z = cache(0);
        z.lookup(0.0, 0.0);
        z.lookup(500.0, 0.0);
        assert_eq!(z.len(), 1);
    }
}
