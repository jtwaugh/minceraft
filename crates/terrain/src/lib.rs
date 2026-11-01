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

pub mod climate;
pub mod data;
pub mod decorate;
pub mod flora;
pub mod niche;
pub mod noise;
mod plants;
mod trees;

use alloc::vec;
use alloc::vec::Vec;
pub use climate::{
    biome_at, biome_for, biome_lookup, climate_at, Climate, HumidityBias, TemperatureBias,
};
pub use data::{
    block_id, Biome, BiomeInfo, Block, BlockInfo, Collision, TreeKind, BIOME_INFO, BLOCK_INFO,
    LEGEND_JSON,
};
pub use decorate::{decorate, ColumnMask, DecorationStats, ProtectedMask};
use noise::{hash_f64, Axis, OctavePerlin, Rng};

/// World height in blocks.
pub const HEIGHT: usize = 128;
/// Sea level: every non-solid block with `y < SEA_LEVEL` is water.
pub const SEA_LEVEL: i32 = 64;
/// Number of vertical lattice levels (spacing 8 blocks).
pub const LEVELS: usize = 17;

pub(crate) const AIR: u8 = Block::Air as u8;
pub(crate) const STONE: u8 = Block::Stone as u8;
pub(crate) const DIRT: u8 = Block::Dirt as u8;
pub(crate) const GRASS: u8 = Block::Grass as u8;
pub(crate) const SAND: u8 = Block::Sand as u8;
pub(crate) const GRAVEL: u8 = Block::Gravel as u8;
pub(crate) const WATER: u8 = Block::Water as u8;
pub(crate) const ICE: u8 = Block::Ice as u8;
pub(crate) const FLOE_ICE: u8 = Block::FloeIce as u8;

/// Sea ice: plain ice or floe ice (sea-ice-floes.md A).
#[inline]
pub(crate) fn is_sea_ice(b: u8) -> bool {
    b == ICE || b == FLOE_ICE
}
/// Internal marker used only during the surface pass; written out as Sand.
const SANDSTONE: u8 = 0x80;

/// A generated region. See the crate docs for the index layout.
#[derive(Clone)]
pub struct Region {
    pub blocks: Vec<u8>,
    pub width: u32,
    pub depth: u32,
    pub height: u32,
    /// World coordinates of the region's minimum corner.
    pub x0: i32,
    pub z0: i32,
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
// EXTENSION POINT: per-column base-height terms (SPEC 3.3, 6.3)
// ===========================================================================
//
// Every term receives WORLD block coordinates (x, z) and the column's
// unbiased base height `base` in blocks, and returns a height offset in
// blocks. The sum of all terms, divided by 8 (blocks per vertical
// lattice level), is added to `center` in SPEC 3.3 step 6, evaluated at each
// lattice column's world block coordinates (lattice x * 4, lattice z * 4).
// Positive values raise the terrain, negative values lower it. Default: none.
//
// Any `Fn(i32, i32, f64) -> f64` closure implements `BaseHeightBias`.

/// A base-height bias term `f(x, z, base) -> f64` in world block
/// coordinates. `base` is the column's unbiased base height in blocks (the
/// column `center` of SPEC 3.3 step 6 times 8); the result is in blocks.
pub trait BaseHeightBias {
    fn bias(&self, x: i32, z: i32, base: f64) -> f64;
}

impl<F: Fn(i32, i32, f64) -> f64> BaseHeightBias for F {
    #[inline]
    fn bias(&self, x: i32, z: i32, base: f64) -> f64 {
        self(x, z, base)
    }
}

// ===========================================================================
// EXTENSION POINT: density clamps (SPEC 6.3)
// ===========================================================================
//
// Every clamp receives WORLD block coordinates and the interpolated density
// (after the additive `DensityBias` terms) and returns the new density. The
// clamps run in list order, immediately before the `rho > 0` test. Default:
// none.
//
// Any `Fn(i32, i32, i32, f64) -> f64` closure implements `DensityClamp`.

/// A density clamp `f(x, y, z, density) -> density` in world block
/// coordinates.
pub trait DensityClamp {
    fn clamp(&self, x: i32, y: i32, z: i32, density: f64) -> f64;
}

impl<F: Fn(i32, i32, i32, f64) -> f64> DensityClamp for F {
    #[inline]
    fn clamp(&self, x: i32, y: i32, z: i32, density: f64) -> f64 {
        self(x, y, z, density)
    }
}

/// Columns where sea ice is skipped: where `open(x, z)` is true the sea-ice
/// step (the fill at `SEA_LEVEL - 1` and the surface pass's frozen water top)
/// places water. Climate, biomes and the snow pass are unaffected.
pub trait OpenWater {
    fn open(&self, x: i32, z: i32) -> bool;
}

impl<F: Fn(i32, i32) -> bool> OpenWater for F {
    #[inline]
    fn open(&self, x: i32, z: i32) -> bool {
        self(x, z)
    }
}

// ===========================================================================
// Public API
// ===========================================================================

/// All optional generation hooks, bundled. `Biases::default()` is "none".
///
/// - `density`: density bias terms (SPEC 6.3), world `(x, y, z)`.
/// - `temperature`: temperature bias terms, world `(x, z)`; added to T
///   (then clamped to [0, 1]) before T is used anywhere: density, biome
///   lookup, sea ice, decoration and snow.
/// - `humidity`: humidity bias terms, world `(x, z)`; added to H (then
///   clamped to [0, 1]) before H is used anywhere.
/// - `base_height`: per-column base-height terms, world `(x, z)` plus the
///   unbiased base height, in blocks; their sum / 8 is added to the column
///   `center` (SPEC 3.3 step 6).
/// - `clamp`: density clamps, world `(x, y, z)` plus the density, applied in
///   order after `density` and before the `rho > 0` test.
/// - `protected`: cells decoration must never modify (raft mask, spawn
///   water). Terrain itself is not affected by the mask.
/// - `open_water`: columns kept free of sea ice (water is placed instead).
/// - `hard_ice_edge`: `true` turns the sea-ice floe band off, so sea ice ends
///   on the hard `T < 0.5` edge; `false` (the default) keeps the band
///   `[SEA_ICE_T_ICE, 0.5)` from `data/sea_ice.toml`.
#[derive(Clone, Copy, Default)]
pub struct Biases<'a> {
    pub density: &'a [&'a dyn DensityBias],
    pub temperature: &'a [&'a dyn TemperatureBias],
    pub humidity: &'a [&'a dyn HumidityBias],
    pub base_height: &'a [&'a dyn BaseHeightBias],
    pub clamp: &'a [&'a dyn DensityClamp],
    pub protected: Option<&'a dyn ProtectedMask>,
    pub open_water: Option<&'a dyn OpenWater>,
    pub hard_ice_edge: bool,
}

/// Generate a `w x d` region of **terrain only** (Part I plus biome surfaces
/// and sea ice; no vegetation) whose minimum corner is world `(x0, z0)`.
///
/// Chunk boundaries (SPEC 15): there are none to handle here. Terrain is a
/// function of world coordinates alone, so no margin or cropping is needed
/// and any two overlapping regions agree.
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
    let b = Biases {
        density: biases,
        ..Biases::default()
    };
    generate_region_with_biases(seed, x0, z0, w, d, &b, false)
}

/// Terrain plus vegetation, snow and ice (SPEC Part II), no biases.
///
/// Chunk boundaries (SPEC 15): see [`generate_region_with_biases`].
pub fn generate_decorated(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Region {
    generate_region_with_biases(seed, x0, z0, w, d, &Biases::default(), true)
}

/// The full-control entry point: density and temperature biases, an optional
/// protected mask, and whether to run the decoration pass.
///
/// With `decorate = true` the terrain is generated for the region plus a
/// margin, decorated in world-aligned 16x16 cells and cropped, so the result
/// is identical for any region origin and size (see [`decorate`]).
///
/// # Chunk boundaries (SPEC 15)
///
/// The original populates a chunk only once its +x/+z neighbours have terrain
/// and anchors features in a square shifted by +8 on both axes. That scheme
/// is deliberately not reproduced. Instead:
///
/// - terrain runs over the region plus a margin on every side;
/// - decoration uses fixed 16x16 cells aligned to world multiples of 16
///   (the cell is the anchor area, so no +8 shift), each with an RNG derived
///   from `(seed, cell x, cell z)`;
/// - the result is cropped to the requested region.
///
/// Hence overlapping regions agree. Without decoration, no margin is needed.
pub fn generate_region_with_biases(
    seed: u64,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &Biases,
    decorate: bool,
) -> Region {
    let gen = Generator::new(seed);
    if decorate {
        return decorate::generate_decorated(&gen, x0, z0, w, d, biases).0;
    }
    let mut blocks = vec![0u8; w as usize * d as usize * HEIGHT];
    gen.for_each_column(x0, z0, w, d, biases, |cx, cz, col, _, _| {
        let i = (cz as usize * w as usize + cx as usize) * HEIGHT;
        blocks[i..i + HEIGHT].copy_from_slice(col);
    });
    Region {
        blocks,
        width: w,
        depth: d,
        height: HEIGHT as u32,
        x0,
        z0,
    }
}

/// Decorated region plus per-biome decoration statistics for features
/// anchored inside the region.
///
/// Chunk boundaries (SPEC 15): see [`generate_region_with_biases`].
pub fn generate_decorated_with_stats(
    seed: u64,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &Biases,
) -> (Region, DecorationStats) {
    decorate::generate_decorated(&Generator::new(seed), x0, z0, w, d, biases)
}

/// Per-column summary of the terrain without materialising the block array.
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
    let b = Biases {
        density: biases,
        ..Biases::default()
    };
    generate_heightmap_with_biases(seed, x0, z0, w, d, &b, false)
}

/// Heightmap (same 4-byte format as [`generate_heightmap`]) with biases and
/// optional decoration; when decorated the top block may be a log, leaves,
/// a plant or a snow layer.
///
/// Chunk boundaries (SPEC 15): with `decorate` this crops the result of
/// [`generate_region_with_biases`], so the same margin-and-cells scheme
/// applies; without it the summary is a pure function of world coordinates.
pub fn generate_heightmap_with_biases(
    seed: u64,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &Biases,
    decorate: bool,
) -> Vec<u8> {
    let mut out = vec![0u8; w as usize * d as usize * 4];
    let mut put = |cx: u32, cz: u32, col: &[u8]| {
        let i = (cz as usize * w as usize + cx as usize) * 4;
        let (tb, ty, fy, fb) = column_summary(col);
        out[i] = tb;
        out[i + 1] = ty;
        out[i + 2] = fy;
        out[i + 3] = fb;
    };
    let gen = Generator::new(seed);
    if decorate {
        let (r, _) = decorate::generate_decorated(&gen, x0, z0, w, d, biases);
        for cz in 0..d {
            for cx in 0..w {
                let i = r.index(cx, 0, cz);
                put(cx, cz, &r.blocks[i..i + HEIGHT]);
            }
        }
    } else {
        gen.for_each_column(x0, z0, w, d, biases, |cx, cz, col, _, _| put(cx, cz, col));
    }
    out
}

/// Per-column climate for a region, row order `z * w + x`.
pub struct ClimateMap {
    pub temperature: Vec<f32>,
    pub humidity: Vec<f32>,
    /// [`Biome`] ids.
    pub biome: Vec<u8>,
}

/// Temperature and humidity (both biased) and biome id for every column of a region.
///
/// Chunk boundaries (SPEC 15): climate is sampled per column at fixed world
/// positions, so there is no margin and no cropping.
pub fn generate_climate(
    seed: u64,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    temperature: &[&dyn TemperatureBias],
    humidity: &[&dyn HumidityBias],
) -> ClimateMap {
    let c = Climate::new(seed);
    let n = w as usize * d as usize;
    let mut m = ClimateMap {
        temperature: Vec::with_capacity(n),
        humidity: Vec::with_capacity(n),
        biome: Vec::with_capacity(n),
    };
    for cz in 0..d {
        for cx in 0..w {
            let (t, h) = c.at(
                x0.wrapping_add(cx as i32),
                z0.wrapping_add(cz as i32),
                temperature,
                humidity,
            );
            m.temperature.push(t as f32);
            m.humidity.push(h as f32);
            m.biome.push(biome_for(t, h) as u8);
        }
    }
    m
}

/// Coarse land survey: the terrain height at every `step`-th block column,
/// read straight off the density lattice (no surface pass, no decoration).
///
/// Samples sit on lattice columns, so `step` is rounded up to a multiple of
/// 4 and the origin down to one. Sample `(i, k)` is world column
/// `(x0' + i * step, z0' + k * step)` where `x0' = x0 & !3`, `z0' = z0 & !3`.
/// Returns 4 bytes per sample in order `k * nx + i`:
/// `[top_y, biome, gaps, gap_h]`, where `top_y` is the highest solid block
/// (land iff `top_y >= SEA_LEVEL`) and matches the full generator's terrain
/// height at that column; `gaps` counts air gaps above sea level with solid
/// both above and below them (overhangs, arches, floating rock) and `gap_h`
/// is the tallest such gap in blocks.
/// Much cheaper per area than [`generate_heightmap`]: one lattice point per
/// sample instead of `step * step` interpolated columns.
///
/// Chunk boundaries (SPEC 15): samples sit on the world-aligned lattice and
/// there is no decoration, so no margin or cropping is involved.
pub fn generate_coarse_heightmap(
    seed: u64,
    x0: i32,
    z0: i32,
    step: u32,
    nx: u32,
    nz: u32,
) -> Vec<u8> {
    let gen = Generator::new(seed);
    let ls = (step.max(1) as i64 + 3) / 4;
    let lx0 = (x0 as i64).div_euclid(4);
    let lz0 = (z0 as i64).div_euclid(4);
    let (nx, nz) = (nx as usize, nz as usize);
    let lat = gen.lattice_grid(lx0, lz0, nx, nz, ls, &Biases::default());
    let mut out = vec![0u8; nx * nz * 4];
    for ix in 0..nx {
        for iz in 0..nz {
            let lev = &lat.data[(ix * lat.nz + iz) * LEVELS..][..LEVELS];
            // Walk down the Y-interpolated density (SPEC 3.5, 3.6): the first
            // solid block is the top; each later air run that ends on solid
            // again at or above sea level is a gap.
            let (mut top, mut gaps, mut gap_h) = (0usize, 0u32, 0usize);
            let mut air_from: Option<usize> = None; // top of the current air run under solid
            let mut seen_solid = false;
            for y in (0..HEIGHT).rev() {
                let l = y >> 3;
                let rho = lev[l] + (lev[l + 1] - lev[l]) * ((y & 7) as f64 * 0.125);
                if rho > 0.0 {
                    if !seen_solid {
                        seen_solid = true;
                        top = y;
                    } else if let Some(a) = air_from.take() {
                        if y + 1 >= SEA_LEVEL as usize {
                            gaps += 1;
                            gap_h = gap_h.max(a - y);
                        }
                    }
                } else if seen_solid && air_from.is_none() {
                    air_from = Some(y);
                }
            }
            let wx = ((lx0 + ix as i64 * ls) * 4) as i32;
            let wz = ((lz0 + iz as i64 * ls) * 4) as i32;
            let o = (iz * nx + ix) * 4;
            out[o] = top as u8;
            out[o + 1] = gen.biome(wx, wz) as u8;
            out[o + 2] = gaps.min(255) as u8;
            out[o + 3] = gap_h.min(255) as u8;
        }
    }
    out
}

/// Solid flags on the world-aligned 4-block grid, for the seed prefilter.
///
/// Sample `(i, j, k)` is world `(x0' + 4i, y0' + 4j, z0' + 4k)` where `x0'`
/// and `z0'` are `x0`, `z0` rounded down to a multiple of 4 and `y0'` is
/// `y_min` rounded up to one; `j` runs over every multiple of 4 in
/// `y_min..=y_max` (the `(y_min, y_max)` pair), clamped to the world. Returns `(flags, ny)` with the flag
/// at index `(j * nz + k) * nx + i`: `rho > 0` after the SPEC 3.5/3.6 Y
/// interpolation, the density bias terms and the clamps (SPEC 6.3), i.e. the
/// terrain pass before the surface pass. Climate, base-height, density and
/// clamp biases are honoured.
///
/// Chunk boundaries (SPEC 15): samples sit on lattice columns at world-aligned
/// positions, so the flag is a function of the seed and the sample coordinate
/// only.
pub fn generate_coarse_density(
    seed: u64,
    x0: i32,
    z0: i32,
    nx: u32,
    nz: u32,
    (y_min, y_max): (i32, i32),
    biases: &Biases,
) -> (Vec<bool>, u32) {
    let (jlo, jhi) = (
        (y_min.max(0) as i64 + 3).div_euclid(4),
        (y_max.min(HEIGHT as i32 - 1) as i64).div_euclid(4),
    );
    let ny = (jhi - jlo + 1).max(0) as usize;
    let (nx, nz) = (nx as usize, nz as usize);
    if ny == 0 || nx == 0 || nz == 0 {
        return (Vec::new(), 0);
    }
    let gen = Generator::new(seed);
    let lx0 = (x0 as i64).div_euclid(4);
    let lz0 = (z0 as i64).div_euclid(4);
    let lat = gen.lattice_grid(lx0, lz0, nx, nz, 1, biases);
    let mut out = vec![false; nx * nz * ny];
    for ix in 0..nx {
        let wx = ((lx0 + ix as i64) * 4) as i32;
        for iz in 0..nz {
            let wz = ((lz0 + iz as i64) * 4) as i32;
            let lev = &lat.data[(ix * lat.nz + iz) * LEVELS..][..LEVELS];
            for j in 0..ny {
                let y = ((jlo + j as i64) * 4) as usize;
                let l = y >> 3;
                let mut rho = lev[l] + (lev[l + 1] - lev[l]) * ((y & 7) as f64 * 0.125);
                for b in biases.density {
                    rho += b.bias(wx, y as i32, wz);
                }
                for c in biases.clamp {
                    rho = c.clamp(wx, y as i32, wz, rho);
                }
                out[(j * nz + iz) * nx + ix] = rho > 0.0;
            }
        }
    }
    (out, ny as u32)
}

/// `(top_block, top_y, floor_y, floor_block)` for one 128-block column.
pub fn column_summary(col: &[u8]) -> (u8, u8, u8, u8) {
    let mut y = HEIGHT - 1;
    while y > 0 && col[y] == AIR {
        y -= 1;
    }
    let top = col[y];
    let ty = y;
    if top == WATER || is_sea_ice(top) {
        while y > 0 && (col[y] == WATER || is_sea_ice(col[y])) {
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
const ID_TREE: u64 = 8;

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
    /// Tree-density noise (SPEC 10.3), 8 octaves.
    pub(crate) tree: OctavePerlin,
    /// The shared climate source (SPEC 7).
    pub climate: Climate,
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
        Generator {
            seed,
            low: g(ID_LOW, 16),
            high: g(ID_HIGH, 16),
            sel: g(ID_SEL, 8),
            sand: g(ID_SAND, 4),
            stone_depth: g(ID_STONE, 4),
            scale: g(ID_SCALE, 10),
            depth: g(ID_DEPTH, 16),
            tree: g(ID_TREE, 8),
            climate: Climate::new(seed),
        }
    }

    /// Temperature and humidity at world block column (x, z) (SPEC 7), no
    /// biases. Same as `self.climate.raw(x, z)`.
    pub fn climate(&self, x: i32, z: i32) -> (f64, f64) {
        self.climate.raw(x, z)
    }

    /// Biome at a block column (no biases).
    pub fn biome(&self, x: i32, z: i32) -> Biome {
        let (t, h) = self.climate.raw(x, z);
        biome_for(t, h)
    }

    /// Desert test at a block column (kept for API compatibility).
    pub fn is_desert_at(&self, x: i32, z: i32) -> bool {
        self.biome(x, z) == Biome::Desert
    }

    /// Desert test from an unquantised climate (SPEC 8 lookup).
    pub fn is_desert(t: f64, h: f64) -> bool {
        biome_for(t, h) == Biome::Desert
    }

    /// Per-lattice-column (center, scale) (SPEC 3.3).
    fn column_terms(&self, lx: i64, lz: i64, biases: &Biases) -> (f64, f64) {
        let (wx, wz) = ((lx * 4) as i32, (lz * 4) as i32);
        let (t, h) = self.climate.at(wx, wz, biases.temperature, biases.humidity);
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
        let mut center = 17.0 / 2.0 + d * 4.0;
        if !biases.base_height.is_empty() {
            let base = center * 8.0;
            let blocks: f64 = biases
                .base_height
                .iter()
                .map(|b| b.bias(wx, wz, base))
                .sum();
            center += blocks / 8.0;
        }
        (center, scale)
    }

    /// Compute the density lattice covering block columns
    /// `x0..x0+w`, `z0..z0+d` (SPEC 3.1 - 3.4), all at once.
    fn lattice(&self, x0: i32, z0: i32, w: u32, d: u32, biases: &Biases) -> Lattice {
        let lx0 = (x0 as i64).div_euclid(4);
        let lz0 = (z0 as i64).div_euclid(4);
        let lx1 = (x0 as i64 + w.max(1) as i64 - 1).div_euclid(4) + 1;
        let lz1 = (z0 as i64 + d.max(1) as i64 - 1).div_euclid(4) + 1;
        let nx = (lx1 - lx0 + 1) as usize;
        let nz = (lz1 - lz0 + 1) as usize;
        self.lattice_grid(lx0, lz0, nx, nz, 1, biases)
    }

    /// Density lattice at lattice columns `(lx0 + ix * step, lz0 + iz * step)`
    /// for `ix < nx`, `iz < nz`. `step == 1` is the full lattice; a larger
    /// step samples a sparse subset (same values at the shared points).
    fn lattice_grid(
        &self,
        lx0: i64,
        lz0: i64,
        nx: usize,
        nz: usize,
        step: i64,
        biases: &Biases,
    ) -> Lattice {
        let n = nx * nz * LEVELS;

        // Base (unscaled) lattice coordinates.
        let lxs: Vec<f64> = (0..nx).map(|i| (lx0 + i as i64 * step) as f64).collect();
        let lzs: Vec<f64> = (0..nz).map(|i| (lz0 + i as i64 * step) as f64).collect();
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
        self.low.fill_grid(
            &mx,
            &my,
            &mz,
            &mut lo,
            Some(&need_lo),
            &mut ax,
            &mut ay,
            &mut az,
        );
        let mut hi = vec![0.0f64; n];
        self.high.fill_grid(
            &mx,
            &my,
            &mz,
            &mut hi,
            Some(&need_hi),
            &mut ax,
            &mut ay,
            &mut az,
        );

        // Combine into densities, reusing the `sel` buffer as output.
        for ix in 0..nx {
            for iz in 0..nz {
                let (center, scale) =
                    self.column_terms(lx0 + ix as i64 * step, lz0 + iz as i64 * step, biases);
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
        Lattice {
            lx0,
            lz0,
            nz,
            data: sel,
        }
    }

    /// Generate every column of the region and hand it to `f(cx, cz, col)`
    /// where `(cx, cz)` are region-relative offsets.
    ///
    /// The callback also receives the column's (biased) climate `(T, H)`.
    pub fn for_each_column<F: FnMut(u32, u32, &[u8; HEIGHT], f64, f64)>(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        biases: &Biases,
        mut f: F,
    ) {
        if w == 0 || d == 0 {
            return;
        }
        let tb = biases.temperature;
        let hb = biases.humidity;
        let db = biases.density;
        let cb = biases.clamp;
        let lat = self.lattice(x0, z0, w, d, biases);
        let mut col = [0u8; HEIGHT];
        let mut lev = [0.0f64; LEVELS];
        // Per-row surface noise buffers (SPEC 4), computed with the grid path
        // so per-axis floor/fade work is shared along the row.
        let wu = w as usize;
        let xs32: Vec<f64> = (0..wu)
            .map(|i| x0.wrapping_add(i as i32) as f64 / 32.0)
            .collect();
        let xs16: Vec<f64> = (0..wu)
            .map(|i| x0.wrapping_add(i as i32) as f64 / 16.0)
            .collect();
        let (mut sn, mut gn, mut dn) = (vec![0.0f64; wu], vec![0.0f64; wu], vec![0.0f64; wu]);
        // Per-row climate, computed ahead of the column loop (keeps the hot
        // fill loop small; noticeably faster in wasm).
        let mut clim = vec![(0.0f64, 0.0f64); wu];
        let (mut ax, mut ay, mut az) = (Vec::new(), Vec::new(), Vec::new());
        for cz in 0..d {
            let wz = z0.wrapping_add(cz as i32);
            let zf = wz as f64;
            for v in sn.iter_mut().chain(gn.iter_mut()).chain(dn.iter_mut()) {
                *v = 0.0;
            }
            // Sn: 3D noise on the plane (x, z, 0) - world Z on the noise Y axis.
            self.sand.fill_grid(
                &xs32,
                &[zf / 32.0],
                &[0.0],
                &mut sn,
                None,
                &mut ax,
                &mut ay,
                &mut az,
            );
            // Gn: 2D path, i.e. 3D at (x, 0, z).
            self.sand.fill_grid(
                &xs32,
                &[0.0],
                &[zf / 32.0],
                &mut gn,
                None,
                &mut ax,
                &mut ay,
                &mut az,
            );
            // Dn: stone-depth, 2D path at block scale 1/16.
            self.stone_depth.fill_grid(
                &xs16,
                &[0.0],
                &[zf / 16.0],
                &mut dn,
                None,
                &mut ax,
                &mut ay,
                &mut az,
            );
            for (i, c) in clim.iter_mut().enumerate() {
                *c = self.climate.at(x0.wrapping_add(i as i32), wz, tb, hb);
            }
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

                let (t, h) = clim[cx as usize];
                let sea_top = self.sea_top(wx, wz, t, biases);
                // Solid / water / air / sea ice (SPEC 3.6, 13).
                for y in 0..HEIGHT {
                    let l = y >> 3;
                    let ty = (y & 7) as f64 * 0.125;
                    let mut rho = lev[l] + (lev[l + 1] - lev[l]) * ty;
                    // ---- density bias extension point (SPEC 6.3) ----
                    for b in db {
                        rho += b.bias(wx, y as i32, wz);
                    }
                    for c in cb {
                        rho = c.clamp(wx, y as i32, wz, rho);
                    }
                    col[y] = if rho > 0.0 {
                        STONE
                    } else if (y as i32) == SEA_LEVEL - 1 {
                        sea_top
                    } else if (y as i32) < SEA_LEVEL {
                        WATER
                    } else {
                        AIR
                    };
                }

                let ci = cx as usize;
                let biome = biome_for(t, h);
                self.surface(wx, wz, sn[ci], gn[ci], dn[ci], biome, sea_top, &mut col);
                f(cx, cz, &col, t, h);
            }
        }
    }

    /// Surface pass for one column (SPEC 4).
    ///
    /// `frozen`: the column's T < 0.5, so a water top placed at y = 63 by the
    /// beach/stone-patch rule becomes ice like the sea fill does.
    #[allow(clippy::too_many_arguments)]
    /// Block at y = `SEA_LEVEL - 1` over open sea in the column (sea-ice-floes.md A):
    /// solid ice below `SEA_ICE_T_ICE`, floe ice or water in the band up to
    /// T = 0.5 (`t + k * n < 0.5`), water elsewhere and on `open_water`.
    fn sea_top(&self, x: i32, z: i32, t: f64, biases: &Biases) -> u8 {
        if t >= 0.5 || biases.open_water.is_some_and(|o| o.open(x, z)) {
            WATER
        } else if biases.hard_ice_edge || t < data::SEA_ICE_T_ICE {
            ICE
        } else if t + data::SEA_ICE_FLOE_STRENGTH * self.floe_noise(x, z) < 0.5 {
            FLOE_ICE
        } else {
            WATER
        }
    }

    /// Smooth value noise in [0, 1) with one lattice cell per `SEA_ICE_FLOE_SCALE` blocks.
    fn floe_noise(&self, x: i32, z: i32) -> f64 {
        let (fx, fz) = (
            x as f64 / data::SEA_ICE_FLOE_SCALE,
            z as f64 / data::SEA_ICE_FLOE_SCALE,
        );
        let (gx, gz) = (noise::floor(fx), noise::floor(fz));
        let sm = |u: f64| u * u * (3.0 - 2.0 * u);
        let (tx, tz) = (sm(fx - gx), sm(fz - gz));
        let corner = |dx: i32, dz: i32| {
            hash_f64(
                self.seed,
                gx as i32 + dx,
                gz as i32 + dz,
                0,
                data::SEA_ICE_FLOE_SALT as u64,
            )
        };
        let a = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * tx;
        let b = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * tx;
        a + (b - a) * tz
    }

    fn surface(
        &self,
        x: i32,
        z: i32,
        sn: f64,
        gn: f64,
        dn: f64,
        biome: Biome,
        sea_top: u8,
        col: &mut [u8; HEIGHT],
    ) {
        let seed = self.seed;
        let r1 = hash_f64(seed, x, z, 0, 1);
        let r2 = hash_f64(seed, x, z, 0, 2);
        let r3 = hash_f64(seed, x, z, 0, 3);
        let sand = sn + r1 * 0.2 > 0.0;
        let gravel = gn + r2 * 0.2 > 3.0;
        // Q8: truncation toward zero (reproduced).
        let soil = (dn / 3.0 + 3.0 + r3 * 0.25) as i32;

        let info = biome.info();
        let (btop, bfill) = (info.top, info.filler);
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
                b if b == WATER || is_sea_ice(b) => {}
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
                            top = if y == SEA_LEVEL - 1 { sea_top } else { WATER };
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

#[cfg(test)]
mod tests_veg;

#[cfg(test)]
mod tests_flora;

#[cfg(test)]
mod tests_ice;
