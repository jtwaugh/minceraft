//! Smoothed ground height for terrain-shaped wind.
//! See docs/design/terrain-wind-shelter.md Phase A.
//!
//! A cell holds `H` (box mean of `max(top_y, SEA_LEVEL)` over `radius`) and
//! `H_wide` (the same over `radius_wide`) at the nodes of a lattice spaced
//! `step` apart, including both cell edges, so neighbouring cells share edge
//! nodes and the bilinear lookup is continuous across them. Each node reads a
//! fixed window of the coarse heightmap around itself, so a value depends only
//! on the seed and the node, never on what else is requested or cached.

use terrain::{generate_coarse_heightmap, SEA_LEVEL};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindHeightParams {
    /// Cell edge, blocks.
    pub cell: i32,
    /// Spacing of heightmap samples and cell nodes, blocks. A multiple of 4.
    pub step: i32,
    /// Smoothing half-width for `H`, blocks. A multiple of `step`.
    pub radius: i32,
    /// Smoothing half-width for `H_wide`, blocks. A multiple of `step`, at least `radius`.
    pub radius_wide: i32,
}

impl WindHeightParams {
    fn check(&self) {
        assert!(
            self.step > 0 && self.step % 4 == 0,
            "step must be a multiple of 4"
        );
        assert!(
            self.cell > 0 && self.cell % self.step == 0,
            "cell must be a multiple of step"
        );
        assert!(
            self.radius >= 0 && self.radius % self.step == 0,
            "radius must be a multiple of step"
        );
        assert!(
            self.radius_wide >= self.radius && self.radius_wide % self.step == 0,
            "radius_wide must be a multiple of step and at least radius"
        );
    }

    /// Edge of the heightmap window a cell fill reads, blocks.
    pub fn window(&self) -> i32 {
        self.cell + 2 * self.radius_wide
    }
}

/// `H` and `H_wide` at the nodes of one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightCell {
    x0: i32,
    z0: i32,
    params: WindHeightParams,
    /// Nodes per side, `cell / step + 1`; index `iz * n + ix`.
    n: usize,
    h: Vec<f64>,
    wide: Vec<f64>,
}

/// Cell index containing world coordinate `v`.
pub fn cell_index(params: &WindHeightParams, v: f64) -> i32 {
    (v / params.cell as f64).floor() as i32
}

impl HeightCell {
    pub fn fill(seed: u64, params: WindHeightParams, cx: i32, cz: i32) -> Self {
        Self::fill_padded(seed, params, cx, cz, 0)
    }

    /// Fill from a heightmap request that starts `pad` samples further out.
    /// The result must not depend on `pad`.
    fn fill_padded(seed: u64, p: WindHeightParams, cx: i32, cz: i32, pad: i32) -> Self {
        p.check();
        let (x0, z0) = (cx * p.cell, cz * p.cell);
        let reach = p.radius_wide + pad * p.step;
        let side = ((p.cell + 2 * reach) / p.step + 1) as u32;
        let raw =
            generate_coarse_heightmap(seed, x0 - reach, z0 - reach, p.step as u32, side, side);
        let side = side as usize;
        let at = |gx: usize, gz: usize| u32::from(raw[(gz * side + gx) * 4]).max(SEA_LEVEL as u32);
        let mean = |gx: usize, gz: usize, r: usize| {
            let mut sum = 0u32;
            for z in gz - r..=gz + r {
                for x in gx - r..=gx + r {
                    sum += at(x, z);
                }
            }
            f64::from(sum) / ((2 * r + 1) * (2 * r + 1)) as f64
        };
        let n = (p.cell / p.step) as usize + 1;
        let (r, rw) = (
            (p.radius / p.step) as usize,
            (p.radius_wide / p.step) as usize,
        );
        let base = (reach / p.step) as usize;
        let (mut h, mut wide) = (Vec::with_capacity(n * n), Vec::with_capacity(n * n));
        for iz in 0..n {
            for ix in 0..n {
                h.push(mean(base + ix, base + iz, r));
                wide.push(mean(base + ix, base + iz, rw));
            }
        }
        debug_assert!(base >= rw);
        HeightCell {
            x0,
            z0,
            params: p,
            n,
            h,
            wide,
        }
    }

    /// Bilinear `(H, H_wide)` at world `(x, z)`, which must lie in this cell
    /// (edges included).
    pub fn lookup(&self, x: f64, z: f64) -> (f64, f64) {
        let p = &self.params;
        let top = (self.n - 2) as f64;
        let fx = ((x - self.x0 as f64) / p.step as f64).clamp(0.0, top + 1.0);
        let fz = ((z - self.z0 as f64) / p.step as f64).clamp(0.0, top + 1.0);
        let (ix, iz) = (fx.floor().min(top), fz.floor().min(top));
        let (tx, tz) = (fx - ix, fz - iz);
        let (ix, iz) = (ix as usize, iz as usize);
        let bil = |v: &[f64]| {
            let a = v[iz * self.n + ix];
            let b = v[iz * self.n + ix + 1];
            let c = v[(iz + 1) * self.n + ix];
            let d = v[(iz + 1) * self.n + ix + 1];
            let lo = a + (b - a) * tx;
            let hi = c + (d - c) * tx;
            lo + (hi - lo) * tz
        };
        (bil(&self.h), bil(&self.wide))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: WindHeightParams = WindHeightParams {
        cell: 32,
        step: 8,
        radius: 16,
        radius_wide: 40,
    };

    #[test]
    fn window_is_cell_plus_both_radii() {
        assert_eq!(P.window(), 112);
    }

    #[test]
    fn fill_is_independent_of_request_origin() {
        for (cx, cz) in [(0, 0), (-3, 5), (7, -2)] {
            let a = HeightCell::fill(12345, P, cx, cz);
            for pad in 1..4 {
                assert_eq!(
                    a,
                    HeightCell::fill_padded(12345, P, cx, cz, pad),
                    "pad {pad}"
                );
            }
        }
    }

    #[test]
    fn water_counts_as_sea_level() {
        for c in [
            HeightCell::fill(7, P, -4, -4),
            HeightCell::fill(7, P, 10, 3),
        ] {
            assert!(c.h.iter().chain(&c.wide).all(|&v| v >= SEA_LEVEL as f64));
        }
    }

    #[test]
    fn lookup_is_continuous_across_cell_edges() {
        let (a, b) = (HeightCell::fill(99, P, 2, 1), HeightCell::fill(99, P, 3, 1));
        let (c, d) = (HeightCell::fill(99, P, 2, 2), HeightCell::fill(99, P, 3, 2));
        for k in 0..=8 {
            let z = 32.0 + 4.0 * k as f64;
            assert_eq!(a.lookup(96.0, z), b.lookup(96.0, z));
        }
        for k in 0..=8 {
            let x = 64.0 + 4.0 * k as f64;
            assert_eq!(a.lookup(x, 64.0), c.lookup(x, 64.0));
        }
        assert_eq!(a.lookup(96.0, 64.0), d.lookup(96.0, 64.0));
        // Interior points interpolate between nodes.
        let (h0, _) = a.lookup(64.0, 32.0);
        let (h1, _) = a.lookup(72.0, 32.0);
        let (hm, _) = a.lookup(68.0, 32.0);
        assert!((hm - (h0 + h1) / 2.0).abs() < 1e-9);
    }

    #[test]
    #[should_panic]
    fn cell_must_be_multiple_of_step() {
        HeightCell::fill(1, WindHeightParams { cell: 36, ..P }, 0, 0);
    }
}
