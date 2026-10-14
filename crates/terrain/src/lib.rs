//! Clean-room Beta-style overworld terrain generator (see SPEC.md).
//!
//! `no_std` + `alloc`, no dependencies.
//!
//! # Block index layout
//!
//! [`Region::blocks`] is **column-major with Y innermost**:
//!
//! ```text
//! index(x, y, z) = ((z * width) + x) * 128 + y
//! ```
//!
//! where `x in 0..width`, `z in 0..depth` are offsets from the region origin
//! `(x0, z0)` and `y in 0..128` is the absolute block height. Each column of
//! 128 bytes is contiguous, and columns are stored in rows of constant `z`.
//!
//! # Position determinism
//!
//! The block at a given world coordinate never depends on the region origin
//! or size: lattice points sit at world multiples of 4 (X/Z) and 8 (Y),
//! climate is sampled at fixed world positions, and per-column randomness is
//! a hash of `(seed, x, z[, y])` (SPEC 6.2, quirks Q5, Q6, Q9 not reproduced).

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod noise;

use alloc::vec;
use alloc::vec::Vec;
use noise::{hash_f64, Axis, OctavePerlin, OctaveSimplex, Rng};

/// World height in blocks.
pub const HEIGHT: usize = 128;
/// Sea level: every non-solid block with `y < SEA_LEVEL` is water.
pub const SEA_LEVEL: i32 = 64;
/// Number of vertical lattice levels (spacing 8 blocks).
pub const LEVELS: usize = 17;

/// Block palette (SPEC 6.2). Bedrock/ice/sandstone are substituted by
/// Stone/Water/Sand.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Air = 0,
    Stone = 1,
    Dirt = 2,
    Grass = 3,
    Sand = 4,
    Gravel = 5,
    Water = 6,
}

impl Block {
    pub fn from_u8(v: u8) -> Block {
        match v {
            1 => Block::Stone,
            2 => Block::Dirt,
            3 => Block::Grass,
            4 => Block::Sand,
            5 => Block::Gravel,
            6 => Block::Water,
            _ => Block::Air,
        }
    }
}

const AIR: u8 = Block::Air as u8;
const STONE: u8 = Block::Stone as u8;
const DIRT: u8 = Block::Dirt as u8;
const GRASS: u8 = Block::Grass as u8;
const SAND: u8 = Block::Sand as u8;
const GRAVEL: u8 = Block::Gravel as u8;
const WATER: u8 = Block::Water as u8;
/// Internal marker used only during the surface pass; written out as Sand.
const SANDSTONE: u8 = 0x80;

/// A generated region. See the crate docs for the index layout.
pub struct Region {
    pub blocks: Vec<u8>,
    pub width: u32,
    pub depth: u32,
    pub height: u32,
}

impl Region {
    #[inline]
    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        ((z as usize * self.width as usize) + x as usize) * HEIGHT + y as usize
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> Block {
        Block::from_u8(self.blocks[self.index(x, y, z)])
    }
}

// ===========================================================================
// EXTENSION POINT: density bias terms (SPEC 6.3)
// ===========================================================================
//
// Every bias function receives WORLD block coordinates (x, y, z) and returns a
// value that is summed with the other biases and added to the interpolated
// density immediately before the `density > 0 => stone` test. Positive values
// add material, negative values carve it. Default: no biases.
//
// Intended uses: island bumps, spawn water, route channels, ...
//
// Any `Fn(i32, i32, i32) -> f64` closure implements `DensityBias`.

/// A density bias term `f(x, y, z) -> f64` in world block coordinates.
pub trait DensityBias {
    fn bias(&self, x: i32, y: i32, z: i32) -> f64;
}

impl<F: Fn(i32, i32, i32) -> f64> DensityBias for F {
    #[inline]
    fn bias(&self, x: i32, y: i32, z: i32) -> f64 {
        self(x, y, z)
    }
}

// ===========================================================================
// Public API
// ===========================================================================

/// Generate a `w x d` region whose minimum corner is world `(x0, z0)`.
pub fn generate_region(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Region {
    generate_region_with_bias(seed, x0, z0, w, d, &[])
}

/// Like [`generate_region`] with additional density bias terms (SPEC 6.3).
pub fn generate_region_with_bias(
    seed: u64,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &[&dyn DensityBias],
) -> Region {
    let gen = Generator::new(seed);
    let mut blocks = vec![0u8; w as usize * d as usize * HEIGHT];
    gen.for_each_column(x0, z0, w, d, biases, |cx, cz, col| {
        let i = (cz as usize * w as usize + cx as usize) * HEIGHT;
        blocks[i..i + HEIGHT].copy_from_slice(col);
    });
    Region { blocks, width: w, depth: d, height: HEIGHT as u32 }
}

/// Per-column summary without materialising the full block array.
///
/// Returns 4 bytes per column, columns in row order `z * w + x`:
/// `[top_block, top_y, floor_y, floor_block]` where `top_*` is the highest
/// non-air block, and `floor_*` is the first non-water block below it when
/// the top is water (otherwise equal to `top_*`).
pub fn generate_heightmap(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    generate_heightmap_with_bias(seed, x0, z0, w, d, &[])
}

pub fn generate_heightmap_with_bias(
    seed: u64,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &[&dyn DensityBias],
) -> Vec<u8> {
    let gen = Generator::new(seed);
    let mut out = vec![0u8; w as usize * d as usize * 4];
    gen.for_each_column(x0, z0, w, d, biases, |cx, cz, col| {
        let i = (cz as usize * w as usize + cx as usize) * 4;
        let (tb, ty, fy, fb) = column_summary(col);
        out[i] = tb;
        out[i + 1] = ty;
        out[i + 2] = fy;
        out[i + 3] = fb;
    });
    out
}

/// `(top_block, top_y, floor_y, floor_block)` for one 128-block column.
pub fn column_summary(col: &[u8]) -> (u8, u8, u8, u8) {
    let mut y = HEIGHT - 1;
    while y > 0 && col[y] == AIR {
        y -= 1;
    }
    let top = col[y];
    let ty = y;
    if top == WATER {
        while y > 0 && col[y] == WATER {
            y -= 1;
        }
    }
    (top, ty as u8, y as u8, col[y])
}

// ===========================================================================
// Generator
// ===========================================================================

/// Main-noise coordinate scale S (SPEC 3.2).
const S: f64 = 684.412;

/// Generator ids for independent RNG streams (SPEC 1.1).
const ID_LOW: u64 = 1;
const ID_HIGH: u64 = 2;
const ID_SEL: u64 = 3;
const ID_SAND: u64 = 4;
const ID_STONE: u64 = 5;
const ID_SCALE: u64 = 6;
const ID_DEPTH: u64 = 7;

/// All noise generators for one seed.
pub struct Generator {
    pub seed: u64,
    low: OctavePerlin,
    high: OctavePerlin,
    sel: OctavePerlin,
    sand: OctavePerlin,
    stone_depth: OctavePerlin,
    scale: OctavePerlin,
    depth: OctavePerlin,
    temp: OctaveSimplex,
    humid: OctaveSimplex,
    blend: OctaveSimplex,
}

/// Density lattice for a region: `(ix * nz + iz) * LEVELS + ly`.
struct Lattice {
    lx0: i64,
    lz0: i64,
    nz: usize,
    data: Vec<f64>,
}

impl Generator {
    pub fn new(seed: u64) -> Generator {
        let g = |id: u64, n: usize| OctavePerlin::new(&mut Rng::for_generator(seed, id), n);
        let c = |m: u64, n: usize| {
            OctaveSimplex::new(&mut Rng::for_generator(seed.wrapping_mul(m), 0x5EED_C11A), n)
        };
        Generator {
            seed,
            low: g(ID_LOW, 16),
            high: g(ID_HIGH, 16),
            sel: g(ID_SEL, 8),
            sand: g(ID_SAND, 4),
            stone_depth: g(ID_STONE, 4),
            scale: g(ID_SCALE, 10),
            depth: g(ID_DEPTH, 16),
            temp: c(9871, 4),
            humid: c(39811, 4),
            blend: c(543321, 2),
        }
    }

    /// Temperature and humidity at world block column (x, z) (SPEC 2).
    pub fn climate(&self, x: i32, z: i32) -> (f64, f64) {
        let (t, b) = self.temperature(x, z);
        (t, self.humidity(x, z, b))
    }

    /// Returns (T, b) where b is the blend term shared with humidity.
    fn temperature(&self, x: i32, z: i32) -> (f64, f64) {
        let (x, z) = (x as f64, z as f64);
        let tn = self.temp.sample(x, z, 0.025, 0.025, 0.25, 0.5);
        let bn = self.blend.sample(x, z, 0.25, 0.25, 1.0 / 1.7, 0.5);
        let b = bn * 1.1 + 0.5;
        let mut t = (tn * 0.15 + 0.7) * 0.99 + b * 0.01;
        t = 1.0 - (1.0 - t) * (1.0 - t);
        (t.clamp(0.0, 1.0), b)
    }

    fn humidity(&self, x: i32, z: i32, b: f64) -> f64 {
        let hn = self.humid.sample(x as f64, z as f64, 0.05, 0.05, 1.0 / 3.0, 0.5);
        ((hn * 0.15 + 0.5) * 0.998 + b * 0.002).clamp(0.0, 1.0)
    }

    /// Desert test at a block column; humidity is only evaluated when the
    /// temperature already qualifies.
    pub fn is_desert_at(&self, x: i32, z: i32) -> bool {
        let (t, b) = self.temperature(x, z);
        if noise::floor(t * 63.0) / 63.0 < 0.95 {
            return false;
        }
        Self::is_desert(t, self.humidity(x, z, b))
    }

    /// Desert test from the quantised climate (SPEC 2, "Biome lookup").
    pub fn is_desert(t: f64, h: f64) -> bool {
        let tq = noise::floor(t * 63.0) / 63.0;
        let hq = noise::floor(h * 63.0) / 63.0;
        let w = tq * hq;
        tq >= 0.1 && w < 0.2 && tq >= 0.95
    }

    /// Per-lattice-column (center, scale) (SPEC 3.3).
    fn column_terms(&self, lx: i64, lz: i64) -> (f64, f64) {
        let (t, h) = self.climate((lx * 4) as i32, (lz * 4) as i32);
        let th = 1.0 - t * h;
        let a = 1.0 - th * th * th * th;
        let c = self.scale.sample2d(lx as f64 * 1.121, lz as f64 * 1.121);
        let dn = self.depth.sample2d(lx as f64 * 200.0, lz as f64 * 200.0);

        let mut scale = ((c + 256.0) / 512.0 * a).min(1.0);
        let mut d = dn / 8000.0;
        if d < 0.0 {
            d = -d * 0.3;
        }
        d = d * 3.0 - 2.0;
        if d < 0.0 {
            d /= 2.0;
            if d < -1.0 {
                d = -1.0;
            }
            d = d / 1.4 / 2.0;
            scale = 0.0;
        } else {
            if d > 1.0 {
                d = 1.0;
            }
            d /= 8.0;
        }
        let scale = scale.max(0.0) + 0.5;
        let d = d * 17.0 / 16.0;
        let center = 17.0 / 2.0 + d * 4.0;
        (center, scale)
    }

    /// Compute the density lattice covering block columns
    /// `x0..x0+w`, `z0..z0+d` (SPEC 3.1 - 3.4), all at once.
    fn lattice(&self, x0: i32, z0: i32, w: u32, d: u32) -> Lattice {
        let lx0 = (x0 as i64).div_euclid(4);
        let lz0 = (z0 as i64).div_euclid(4);
        let lx1 = (x0 as i64 + w.max(1) as i64 - 1).div_euclid(4) + 1;
        let lz1 = (z0 as i64 + d.max(1) as i64 - 1).div_euclid(4) + 1;
        let nx = (lx1 - lx0 + 1) as usize;
        let nz = (lz1 - lz0 + 1) as usize;
        let n = nx * nz * LEVELS;

        // Base (unscaled) lattice coordinates.
        let lxs: Vec<f64> = (0..nx).map(|i| (lx0 + i as i64) as f64).collect();
        let lzs: Vec<f64> = (0..nz).map(|i| (lz0 + i as i64) as f64).collect();
        let lys: Vec<f64> = (0..LEVELS).map(|i| i as f64).collect();
        let scaled = |v: &[f64], s: f64| -> Vec<f64> { v.iter().map(|&c| c * s).collect() };

        let mut ax: Vec<Axis> = Vec::with_capacity(nx);
        let mut ay: Vec<Axis> = Vec::with_capacity(LEVELS);
        let mut az: Vec<Axis> = Vec::with_capacity(nz);

        // Selector M first, so that L/U can be skipped where irrelevant.
        let mut sel = vec![0.0f64; n];
        self.sel.fill_grid(
            &scaled(&lxs, S / 80.0),
            &scaled(&lys, S / 160.0),
            &scaled(&lzs, S / 80.0),
            &mut sel,
            None,
            &mut ax,
            &mut ay,
            &mut az,
        );
        for s in sel.iter_mut() {
            *s = (*s / 10.0 + 1.0) / 2.0;
        }
        let need_lo: Vec<bool> = sel.iter().map(|&s| s < 1.0).collect();
        let need_hi: Vec<bool> = sel.iter().map(|&s| s > 0.0).collect();

        let (mx, my, mz) = (scaled(&lxs, S), scaled(&lys, S), scaled(&lzs, S));
        let mut lo = vec![0.0f64; n];
        self.low
            .fill_grid(&mx, &my, &mz, &mut lo, Some(&need_lo), &mut ax, &mut ay, &mut az);
        let mut hi = vec![0.0f64; n];
        self.high
            .fill_grid(&mx, &my, &mz, &mut hi, Some(&need_hi), &mut ax, &mut ay, &mut az);

        // Combine into densities, reusing the `sel` buffer as output.
        for ix in 0..nx {
            for iz in 0..nz {
                let (center, scale) = self.column_terms(lx0 + ix as i64, lz0 + iz as i64);
                let base = (ix * nz + iz) * LEVELS;
                for ly in 0..LEVELS {
                    let i = base + ly;
                    let s = sel[i];
                    let l = lo[i] / 512.0;
                    let u = hi[i] / 512.0;
                    let nval = if s < 0.0 {
                        l
                    } else if s > 1.0 {
                        u
                    } else {
                        l + (u - l) * s
                    };
                    let mut fall = (ly as f64 - center) * 12.0 / scale;
                    if fall < 0.0 {
                        fall *= 4.0;
                    }
                    let mut dens = nval - fall;
                    if ly > 13 {
                        let t = (ly as f64 - 13.0) / 3.0;
                        dens = dens * (1.0 - t) + -10.0 * t;
                    }
                    sel[i] = dens;
                }
            }
        }
        Lattice { lx0, lz0, nz, data: sel }
    }

    /// Generate every column of the region and hand it to `f(cx, cz, col)`
    /// where `(cx, cz)` are region-relative offsets.
    pub fn for_each_column<F: FnMut(u32, u32, &[u8; HEIGHT])>(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        biases: &[&dyn DensityBias],
        mut f: F,
    ) {
        if w == 0 || d == 0 {
            return;
        }
        let lat = self.lattice(x0, z0, w, d);
        let mut col = [0u8; HEIGHT];
        let mut lev = [0.0f64; LEVELS];
        // Per-row surface noise buffers (SPEC 4), computed with the grid path
        // so per-axis floor/fade work is shared along the row.
        let wu = w as usize;
        let xs32: Vec<f64> = (0..wu).map(|i| x0.wrapping_add(i as i32) as f64 / 32.0).collect();
        let xs16: Vec<f64> = (0..wu).map(|i| x0.wrapping_add(i as i32) as f64 / 16.0).collect();
        let (mut sn, mut gn, mut dn) = (vec![0.0f64; wu], vec![0.0f64; wu], vec![0.0f64; wu]);
        let (mut ax, mut ay, mut az) = (Vec::new(), Vec::new(), Vec::new());
        for cz in 0..d {
            let wz = z0.wrapping_add(cz as i32);
            let zf = wz as f64;
            for v in sn.iter_mut().chain(gn.iter_mut()).chain(dn.iter_mut()) {
                *v = 0.0;
            }
            // Sn: 3D noise on the plane (x, z, 0) - world Z on the noise Y axis.
            self.sand.fill_grid(&xs32, &[zf / 32.0], &[0.0], &mut sn, None, &mut ax, &mut ay, &mut az);
            // Gn: 2D path, i.e. 3D at (x, 0, z).
            self.sand.fill_grid(&xs32, &[0.0], &[zf / 32.0], &mut gn, None, &mut ax, &mut ay, &mut az);
            // Dn: stone-depth, 2D path at block scale 1/16.
            self.stone_depth
                .fill_grid(&xs16, &[0.0], &[zf / 16.0], &mut dn, None, &mut ax, &mut ay, &mut az);
            let lz = (wz as i64).div_euclid(4);
            let tz = (wz as i64).rem_euclid(4) as f64 * 0.25;
            let iz = (lz - lat.lz0) as usize;
            for cx in 0..w {
                let wx = x0.wrapping_add(cx as i32);
                let lx = (wx as i64).div_euclid(4);
                let tx = (wx as i64).rem_euclid(4) as f64 * 0.25;
                let ix = (lx - lat.lx0) as usize;

                // Bilinear in X/Z for each level, then linear in Y (= trilinear).
                let b00 = (ix * lat.nz + iz) * LEVELS;
                let b01 = (ix * lat.nz + iz + 1) * LEVELS;
                let b10 = ((ix + 1) * lat.nz + iz) * LEVELS;
                let b11 = ((ix + 1) * lat.nz + iz + 1) * LEVELS;
                let dd = &lat.data;
                for ly in 0..LEVELS {
                    let a = dd[b00 + ly] + (dd[b10 + ly] - dd[b00 + ly]) * tx;
                    let b = dd[b01 + ly] + (dd[b11 + ly] - dd[b01 + ly]) * tx;
                    lev[ly] = a + (b - a) * tz;
                }

                // Solid / water / air (SPEC 3.6).
                for y in 0..HEIGHT {
                    let l = y >> 3;
                    let ty = (y & 7) as f64 * 0.125;
                    let mut rho = lev[l] + (lev[l + 1] - lev[l]) * ty;
                    // ---- density bias extension point (SPEC 6.3) ----
                    for b in biases {
                        rho += b.bias(wx, y as i32, wz);
                    }
                    col[y] = if rho > 0.0 {
                        STONE
                    } else if (y as i32) < SEA_LEVEL {
                        WATER // (ice at y = 63 when T < 0.5 -> Water)
                    } else {
                        AIR
                    };
                }

                let ci = cx as usize;
                self.surface(wx, wz, sn[ci], gn[ci], dn[ci], &mut col);
                f(cx, cz, &col);
            }
        }
    }

    /// Surface pass for one column (SPEC 4).
    fn surface(&self, x: i32, z: i32, sn: f64, gn: f64, dn: f64, col: &mut [u8; HEIGHT]) {
        let seed = self.seed;
        let r1 = hash_f64(seed, x, z, 0, 1);
        let r2 = hash_f64(seed, x, z, 0, 2);
        let r3 = hash_f64(seed, x, z, 0, 3);
        let sand = sn + r1 * 0.2 > 0.0;
        let gravel = gn + r2 * 0.2 > 3.0;
        // Q8: truncation toward zero (reproduced).
        let soil = (dn / 3.0 + 3.0 + r3 * 0.25) as i32;

        let (btop, bfill) = if self.is_desert_at(x, z) { (SAND, SAND) } else { (GRASS, DIRT) };
        let mut top = btop;
        let mut filler = bfill;
        let mut run: i32 = -1;

        for y in (0..HEIGHT as i32).rev() {
            let yi = y as usize;
            // Bedrock (-> Stone) at y <= rand_int(0..=4).
            if y <= 4 && y <= (hash_f64(seed, x, z, y, 4) * 5.0) as i32 {
                col[yi] = STONE;
                continue;
            }
            match col[yi] {
                AIR => run = -1,
                WATER => {}
                _ => {
                    if run == -1 {
                        if soil <= 0 {
                            top = AIR;
                            filler = STONE;
                        } else if (60..=65).contains(&y) {
                            top = btop;
                            filler = bfill;
                            if gravel {
                                top = AIR;
                                filler = GRAVEL;
                            }
                            if sand {
                                top = SAND;
                                filler = SAND;
                            }
                        }
                        if y < SEA_LEVEL && top == AIR {
                            top = WATER;
                        }
                        // Spec says `run = soil`; a negative soil would set
                        // run back to -1 and re-trigger the surface rule on
                        // every block below, so clamp at 0.
                        run = soil.max(0);
                        col[yi] = if y >= SEA_LEVEL - 1 { top } else { filler };
                    } else if run > 0 {
                        run -= 1;
                        col[yi] = filler;
                        if run == 0 && filler == SAND {
                            run = (hash_f64(seed, x, z, y, 5) * 4.0) as i32;
                            filler = SANDSTONE;
                        }
                    }
                }
            }
        }
        for b in col.iter_mut() {
            if *b == SANDSTONE {
                *b = SAND;
            }
        }
    }
}

#[cfg(test)]
mod tests;
