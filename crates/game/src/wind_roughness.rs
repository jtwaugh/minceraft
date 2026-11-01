//! Smoothed roughness field `R(x, z)` for wind, filled one cell at a time.
//! See docs/design/biome-roughness-wind.md Phase A.
//!
//! `R` is the mean of per-sample roughness over a square neighbourhood of
//! half-width `radius`, sampled from `terrain::generate_coarse_heightmap`.
//! A cell stores `R` at its four corners; the lookup is bilinear between
//! them, so it is continuous across cell edges. A corner value depends only
//! on the seed and the corner, because every sample sits on the world lattice
//! of multiples of `step`. There is no cache here.

use terrain::SEA_LEVEL;

/// Roughness values and field geometry, all in blocks where applicable.
#[derive(Clone, Debug, PartialEq)]
pub struct RoughnessParams {
    /// Roughness per biome byte, indexed like `terrain::BIOME_INFO`.
    pub biome: Vec<f64>,
    /// Roughness of any sample with `top_y < SEA_LEVEL`, whatever its biome.
    pub water: f64,
    /// Side of a cell; a multiple of `step`.
    pub cell_size: i32,
    /// Spacing of heightmap samples; a multiple of 4 (the coarse lattice).
    pub step: i32,
    /// Half-width of the smoothing neighbourhood; a multiple of `step`.
    pub radius: i32,
}

impl RoughnessParams {
    fn check(&self) {
        assert!(
            self.step > 0 && self.step % 4 == 0,
            "step must be a positive multiple of 4"
        );
        assert!(
            self.cell_size > 0 && self.cell_size % self.step == 0,
            "cell size must be a multiple of the sample step"
        );
        assert!(
            self.radius >= 0 && self.radius % self.step == 0,
            "smoothing radius must be a multiple of the sample step"
        );
    }

    /// Side of the square window one fill reads: the cell plus the radius on
    /// every side, in blocks. The heightmap request has `window_samples()` per side.
    pub fn window_size(&self) -> i32 {
        self.cell_size + 2 * self.radius
    }

    pub fn window_samples(&self) -> u32 {
        (self.window_size() / self.step + 1) as u32
    }

    fn sample(&self, top_y: u8, biome: u8) -> f64 {
        if (top_y as i32) < SEA_LEVEL {
            self.water
        } else {
            self.biome[biome as usize]
        }
    }
}

/// `R` at the four corners of one cell: `[x0z0, x1z0, x0z1, x1z1]`.
#[derive(Clone, Debug, PartialEq)]
pub struct RoughnessCell {
    pub cx: i32,
    pub cz: i32,
    pub cell_size: i32,
    pub corners: [f64; 4],
}

impl RoughnessCell {
    /// Bilinear `R` at world `(x, z)`, clamped to the cell.
    pub fn at(&self, x: f64, z: f64) -> f64 {
        let size = self.cell_size as f64;
        let tx = ((x - (self.cx * self.cell_size) as f64) / size).clamp(0.0, 1.0);
        let tz = ((z - (self.cz * self.cell_size) as f64) / size).clamp(0.0, 1.0);
        let [a, b, c, d] = self.corners;
        let top = a + (b - a) * tx;
        let bottom = c + (d - c) * tx;
        top + (bottom - top) * tz
    }
}

/// Mean roughness of the samples within `n` indices of `(i, k)` in a
/// `nx`-wide coarse heightmap `map` (4 bytes per sample).
fn mean_around(
    params: &RoughnessParams,
    map: &[u8],
    nx: usize,
    i: usize,
    k: usize,
    n: usize,
) -> f64 {
    let (mut sum, mut count) = (0.0, 0u32);
    for kk in k - n..=k + n {
        for ii in i - n..=i + n {
            let s = &map[(kk * nx + ii) * 4..];
            sum += params.sample(s[0], s[1]);
            count += 1;
        }
    }
    sum / count as f64
}

/// Fills cell `(cx, cz)`, covering world `[cx*size, (cx+1)*size)` on each axis.
pub fn fill_cell(seed: u64, params: &RoughnessParams, cx: i32, cz: i32) -> RoughnessCell {
    params.check();
    let size = params.cell_size;
    let (x0, z0) = (cx * size - params.radius, cz * size - params.radius);
    let n = params.window_samples();
    let map = terrain::generate_coarse_heightmap(seed, x0, z0, params.step as u32, n, n);
    let nx = n as usize;
    let span = (size / params.step) as usize;
    let r = (params.radius / params.step) as usize;
    let at = |dx: usize, dz: usize| mean_around(params, &map, nx, r + dx * span, r + dz * span, r);
    RoughnessCell {
        cx,
        cz,
        cell_size: size,
        corners: [at(0, 0), at(1, 0), at(0, 1), at(1, 1)],
    }
}

/// Bilinear `R` at any world `(x, z)`; fills the containing cell each call.
pub fn roughness_at(seed: u64, params: &RoughnessParams, x: f64, z: f64) -> f64 {
    params.check();
    let size = params.cell_size as f64;
    let cx = (x / size).floor() as i32;
    let cz = (z / size).floor() as i32;
    fill_cell(seed, params, cx, cz).at(x, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(biome: Vec<f64>, water: f64) -> RoughnessParams {
        RoughnessParams {
            biome,
            water,
            cell_size: 16,
            step: 4,
            radius: 8,
        }
    }

    fn table(v: f64) -> Vec<f64> {
        vec![v; terrain::BIOME_INFO.len()]
    }

    #[test]
    fn constant_table_gives_constant_r() {
        let p = params(table(0.4), 0.4);
        for (cx, cz) in [(0, 0), (-3, 5), (40, -17)] {
            let c = fill_cell(7, &p, cx, cz);
            assert!(c.corners.iter().all(|v| (v - 0.4).abs() < 1e-12));
            assert!((c.at((cx * 16 + 5) as f64, (cz * 16 + 9) as f64) - 0.4).abs() < 1e-12);
        }
    }

    #[test]
    fn water_ignores_biome_and_land_uses_biome() {
        let p = params(
            (0..terrain::BIOME_INFO.len()).map(|i| i as f64).collect(),
            -1.0,
        );
        assert_eq!(p.sample(SEA_LEVEL as u8 - 1, 3), -1.0);
        assert_eq!(p.sample(0, 9), -1.0);
        assert_eq!(p.sample(SEA_LEVEL as u8, 3), 3.0);
    }

    #[test]
    fn fills_agree_with_a_different_request_origin() {
        let p = params(
            (0..terrain::BIOME_INFO.len())
                .map(|i| i as f64 / 10.0)
                .collect(),
            0.05,
        );
        let cell = fill_cell(99, &p, 2, -3);
        // Same corners read from a larger window with another origin.
        let (ox, oz) = (2 * 16 - 8 - 20, -3 * 16 - 8 - 12);
        let n = 20usize;
        let map = terrain::generate_coarse_heightmap(99, ox, oz, 4, n as u32, n as u32);
        let at = |dx: usize, dz: usize| mean_around(&p, &map, n, 5 + 2 + dx * 4, 3 + 2 + dz * 4, 2);
        assert_eq!(cell.corners, [at(0, 0), at(1, 0), at(0, 1), at(1, 1)]);
    }

    #[test]
    fn neighbouring_cells_agree_on_the_shared_edge() {
        let p = params(
            (0..terrain::BIOME_INFO.len())
                .map(|i| (i * i % 7) as f64 / 7.0)
                .collect(),
            0.0,
        );
        let a = fill_cell(5, &p, 0, 0);
        let b = fill_cell(5, &p, 1, 0);
        let c = fill_cell(5, &p, 0, 1);
        for z in [0.0, 3.5, 16.0] {
            assert_eq!(a.at(16.0, z), b.at(16.0, z));
        }
        for x in [0.0, 7.25, 16.0] {
            assert_eq!(a.at(x, 16.0), c.at(x, 16.0));
        }
        assert_eq!(roughness_at(5, &p, 16.0, 8.0), a.at(16.0, 8.0));
    }

    #[test]
    fn border_smooths_over_the_radius() {
        // Left half roughness 0, right half 1; border between sample columns.
        let mut p = params(vec![0.0, 1.0], 0.0);
        p.radius = 16;
        let (nx, nz) = (40usize, 9usize);
        let mut map = vec![0u8; nx * nz * 4];
        for k in 0..nz {
            for i in 0..nx {
                map[(k * nx + i) * 4] = 100;
                map[(k * nx + i) * 4 + 1] = (i >= 20) as u8;
            }
        }
        let n = 4; // radius in samples
        let v = |i: usize| mean_around(&p, &map, nx, i, 4, n);
        assert_eq!(v(10), 0.0);
        assert_eq!(v(15), 0.0);
        assert!(v(19) > 0.0 && v(19) < 0.5);
        assert!(v(20) > v(19) && v(20) < 1.0);
        assert!(v(23) < 1.0);
        assert_eq!(v(24), 1.0);
        assert_eq!(v(30), 1.0);
    }
}
