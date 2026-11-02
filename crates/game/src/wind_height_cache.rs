//! Bounded cache of wind height cells and the surface-height helper.
//! See docs/design/terrain-wind-shelter.md Phase A.
//!
//! A pure memo over `HeightCell::fill`, keyed by `(cx, cz)` for one seed.
//! Eviction (oldest fill first) only decides what is recomputed; a miss
//! refills exactly the same numbers, so no value depends on cache state.
//! This is the place the land-sea breeze's land fraction (#1750) joins: it
//! adds its field to the same fill, not a second cache.

use crate::wind_config::WindConfig;
use crate::wind_height::{cell_index, HeightCell, WindHeightParams};

pub struct WindHeightCache {
    seed: u64,
    params: WindHeightParams,
    surface_offset: f64,
    capacity: usize,
    /// Oldest first.
    cells: Vec<((i32, i32), HeightCell)>,
}

impl WindHeightCache {
    /// A cache holding at most `capacity` cells (at least one).
    pub fn new(seed: u64, config: &WindConfig, capacity: usize) -> Self {
        WindHeightCache {
            seed,
            params: config.height,
            surface_offset: config.surface_offset,
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

    /// `(H, H_wide)` at world `(x, z)`, filling the cell on a miss.
    pub fn lookup(&mut self, x: f64, z: f64) -> (f64, f64) {
        let key = (cell_index(&self.params, x), cell_index(&self.params, z));
        let at = match self.cells.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                if self.cells.len() == self.capacity {
                    self.cells.remove(0);
                }
                let cell = HeightCell::fill(self.seed, self.params, key.0, key.1);
                self.cells.push((key, cell));
                self.cells.len() - 1
            }
        };
        self.cells[at].1.lookup(x, z)
    }

    /// `H(x, z)` plus the configured offset: the height to use for a caller
    /// that has none.
    pub fn surface_height(&mut self, x: f64, z: f64) -> f64 {
        self.lookup(x, z).0 + self.surface_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points() -> Vec<(f64, f64)> {
        let mut v = Vec::new();
        for i in -6..7 {
            for j in -5..6 {
                v.push((i as f64 * 37.3, j as f64 * 41.9));
            }
        }
        v
    }

    fn cache(cap: usize) -> WindHeightCache {
        WindHeightCache::new(4242, &WindConfig::default_from_data().unwrap(), cap)
    }

    #[test]
    fn cold_warm_and_evicting_caches_agree() {
        let pts = points();
        let mut big = cache(10_000);
        let cold: Vec<_> = pts.iter().map(|&(x, z)| big.lookup(x, z)).collect();
        let warm: Vec<_> = pts.iter().map(|&(x, z)| big.lookup(x, z)).collect();
        assert_eq!(cold, warm);
        for cap in [1, 2] {
            let mut small = cache(cap);
            for (i, &(x, z)) in pts.iter().enumerate() {
                assert_eq!(small.lookup(x, z), cold[i], "cap {cap}");
                assert!(small.len() <= cap);
            }
            // Revisit in reverse so every cell has been evicted since.
            for (i, &(x, z)) in pts.iter().enumerate().rev() {
                assert_eq!(small.lookup(x, z), cold[i], "cap {cap} reverse");
            }
        }
    }

    #[test]
    fn overlapping_cells_agree_on_shared_edge() {
        let mut c = cache(4);
        let edge = c.params.cell as f64;
        // x = edge lies in cell 1 but is also the right edge of cell 0.
        let on_edge = c.lookup(edge, 10.0);
        let fresh = HeightCell::fill(4242, c.params, 0, 0).lookup(edge, 10.0);
        assert_eq!(on_edge, fresh);
    }

    #[test]
    fn capacity_is_respected_and_zero_means_one() {
        let mut c = cache(3);
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

    #[test]
    fn surface_height_is_h_plus_offset() {
        let mut c = cache(2);
        let offset = WindConfig::default_from_data().unwrap().surface_offset;
        for &(x, z) in points().iter().take(20) {
            let h = c.lookup(x, z).0;
            assert_eq!(c.surface_height(x, z), h + offset);
        }
    }
}
