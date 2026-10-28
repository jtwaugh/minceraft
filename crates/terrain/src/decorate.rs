//! Decoration ("population") pass: trees, small plants and snow (SPEC 10-13,
//! 15), run over a whole region at once.
//!
//! # Cells instead of Beta's +8 offset (SPEC 15)
//!
//! The world is cut into fixed 16x16 *decoration cells* aligned to world
//! multiples of 16. Every feature is anchored inside its cell and reaches at
//! most 7 blocks out of it (writes) and 8 blocks out (reads). Each cell has
//! its own RNG hashed from `(seed, cell x, cell z)`.
//!
//! Cells are processed in four passes by cell parity: `(even, even)`, then
//! `(odd, even)`, `(even, odd)`, `(odd, odd)`. Two cells of the same pass are
//! at least one cell apart, so their read/write areas never overlap and order
//! inside a pass is irrelevant. A cell in pass `p` can only see writes from
//! earlier passes of its 8 neighbours, so the dependency chain of any block
//! is at most 4 cells long. [`generate_decorated`] therefore generates terrain
//! for the region plus 4 cells + 8 blocks, processes pass `p` only for cells
//! within `4 - p` cells of the region, and crops: the output is **exactly**
//! independent of region origin and size.
//!
//! The biome is chosen per feature at its own column (recommended deviation
//! B3): a cell draws the maximum attempt count any biome could use, and each
//! attempt is kept only if its index is below its own biome's count. In a
//! single-biome cell that reproduces the spec's counts exactly.
//!
//! Afterwards a per-column cleanup removes plants that lost their support to
//! a later feature (a leaf replacing the middle of a cane, a canopy touching
//! a cactus), and snow runs last, per column. Both only depend on final
//! blocks in and next to their column, so they keep the exactness.

use crate::climate::biome_for;
use crate::data::{
    blocks_movement, is_liquid, opacity, Biome, BiomeInfo, Block, Plants, TreeKind, BIOME_INFO,
};
use crate::flora::{Flora, FloraShape, FLORA};
use crate::noise::{mix64, Rng};
use crate::{plants, trees, Biases, Generator, Region, AIR, HEIGHT, ICE, STONE};
use alloc::vec;
use alloc::vec::Vec;

const SNOW: u8 = Block::SnowLayer as u8;

/// Cells beyond the region processed by [`generate_decorated`] (see module docs).
pub const MARGIN_CELLS: i32 = 4;
/// Extra terrain beyond the outermost processed cell (feature read reach).
pub const READ_REACH: i32 = 8;

// ===========================================================================
// EXTENSION POINT: protected mask
// ===========================================================================
//
// Decoration never modifies a cell for which `protected(x, y, z)` is true
// (world block coordinates). Trees that would write any protected cell are
// skipped entirely; plants and snow simply aren't placed there. Any
// `Fn(i32, i32, i32) -> bool` closure implements it; [`ColumnMask`] is a
// ready-made per-column bitmask.

/// Cells the decoration pass must leave untouched.
pub trait ProtectedMask {
    fn protected(&self, x: i32, y: i32, z: i32) -> bool;
}

impl<F: Fn(i32, i32, i32) -> bool> ProtectedMask for F {
    #[inline]
    fn protected(&self, x: i32, y: i32, z: i32) -> bool {
        self(x, y, z)
    }
}

/// A protected mask over whole columns `(x, z)` (every y), as a bitmask over
/// the world rectangle `[x0, x0 + w) x [z0, z0 + d)`. Columns outside the
/// rectangle are unprotected.
#[derive(Clone)]
pub struct ColumnMask {
    pub x0: i32,
    pub z0: i32,
    pub w: u32,
    pub d: u32,
    bits: Vec<u64>,
}

impl ColumnMask {
    pub fn new(x0: i32, z0: i32, w: u32, d: u32) -> ColumnMask {
        let n = w as usize * d as usize;
        ColumnMask {
            x0,
            z0,
            w,
            d,
            bits: vec![0; n.div_ceil(64)],
        }
    }

    /// From one byte per column (`z * w + x`, non-zero = protected).
    pub fn from_bytes(x0: i32, z0: i32, w: u32, d: u32, bytes: &[u8]) -> ColumnMask {
        let mut m = ColumnMask::new(x0, z0, w, d);
        for (i, &b) in bytes.iter().enumerate().take(w as usize * d as usize) {
            if b != 0 {
                m.bits[i / 64] |= 1 << (i % 64);
            }
        }
        m
    }

    fn idx(&self, x: i32, z: i32) -> Option<usize> {
        let (dx, dz) = (x.wrapping_sub(self.x0), z.wrapping_sub(self.z0));
        if dx < 0 || dz < 0 || dx as u32 >= self.w || dz as u32 >= self.d {
            return None;
        }
        Some(dz as usize * self.w as usize + dx as usize)
    }

    pub fn set(&mut self, x: i32, z: i32, on: bool) {
        if let Some(i) = self.idx(x, z) {
            if on {
                self.bits[i / 64] |= 1 << (i % 64);
            } else {
                self.bits[i / 64] &= !(1 << (i % 64));
            }
        }
    }

    pub fn get(&self, x: i32, z: i32) -> bool {
        self.idx(x, z)
            .is_some_and(|i| self.bits[i / 64] >> (i % 64) & 1 != 0)
    }
}

impl ProtectedMask for ColumnMask {
    fn protected(&self, x: i32, _y: i32, z: i32) -> bool {
        self.get(x, z)
    }
}

// ===========================================================================
// Statistics
// ===========================================================================

/// Per-biome counters. Columns are attributed to their own biome; features to
/// the biome of their anchor column.
#[derive(Clone, Copy, Debug, Default)]
pub struct BiomeStats {
    pub columns: u64,
    pub tree_attempts: u64,
    pub trees: u64,
    pub plants: u64,
    pub snow: u64,
}

/// Decoration statistics for features anchored inside the counted area.
#[derive(Clone, Debug, Default)]
pub struct DecorationStats {
    pub biomes: [BiomeStats; Biome::COUNT],
}

impl DecorationStats {
    /// Tree attempts per 256 columns (one chunk's area) of `b`.
    pub fn attempts_per_chunk(&self, b: Biome) -> f64 {
        let s = &self.biomes[b as usize];
        if s.columns == 0 {
            0.0
        } else {
            s.tree_attempts as f64 * 256.0 / s.columns as f64
        }
    }

    /// Trees actually grown per 256 columns of `b`.
    pub fn trees_per_chunk(&self, b: Biome) -> f64 {
        let s = &self.biomes[b as usize];
        if s.columns == 0 {
            0.0
        } else {
            s.trees as f64 * 256.0 / s.columns as f64
        }
    }
}

// ===========================================================================
// World buffer
// ===========================================================================

/// Mutable block buffer with per-column climate, used by every generator.
pub(crate) struct World<'a> {
    blocks: &'a mut [u8],
    x0: i32,
    z0: i32,
    w: i32,
    d: i32,
    temp: &'a [f64],
    biome: &'a [u8],
    protected: Option<&'a dyn ProtectedMask>,
    pending: Vec<(i32, i32, i32, u8)>,
}

impl<'a> World<'a> {
    #[inline]
    fn col(&self, x: i32, z: i32) -> Option<usize> {
        let (dx, dz) = (x.wrapping_sub(self.x0), z.wrapping_sub(self.z0));
        if dx < 0 || dz < 0 || dx >= self.w || dz >= self.d {
            None
        } else {
            Some(dz as usize * self.w as usize + dx as usize)
        }
    }

    /// Block at a world position; anything outside the buffer (horizontally
    /// or vertically) reads as stone, i.e. "occupied".
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if !(0..HEIGHT as i32).contains(&y) {
            return STONE;
        }
        match self.col(x, z) {
            Some(c) => self.blocks[c * HEIGHT + y as usize],
            None => STONE,
        }
    }

    #[inline]
    fn is_protected(&self, x: i32, y: i32, z: i32) -> bool {
        self.protected.is_some_and(|m| m.protected(x, y, z))
    }

    /// Write one block unless out of range or protected.
    pub fn set(&mut self, x: i32, y: i32, z: i32, b: u8) -> bool {
        if !(0..HEIGHT as i32).contains(&y) || self.is_protected(x, y, z) {
            return false;
        }
        match self.col(x, z) {
            Some(c) => {
                self.blocks[c * HEIGHT + y as usize] = b;
                true
            }
            None => false,
        }
    }

    /// Queue a write for the current tree (see [`World::commit`]).
    #[inline]
    pub fn stage(&mut self, x: i32, y: i32, z: i32, b: u8) {
        self.pending.push((x, y, z, b));
    }

    /// Apply the queued writes in order, or none of them if any targets a
    /// protected cell (deterministic skip). Returns whether they were applied.
    pub fn commit(&mut self) -> bool {
        let mut pending = core::mem::take(&mut self.pending);
        let blocked = pending
            .iter()
            .any(|&(x, y, z, _)| self.is_protected(x, y, z));
        if !blocked {
            for &(x, y, z, b) in &pending {
                self.set(x, y, z, b);
            }
        }
        pending.clear();
        self.pending = pending;
        !blocked
    }

    #[cfg(test)]
    pub fn pending(&self) -> &[(i32, i32, i32, u8)] {
        &self.pending
    }

    pub fn discard(&mut self) {
        self.pending.clear();
    }

    /// Unquantised (biased) temperature of a column.
    pub fn temperature(&self, x: i32, z: i32) -> f64 {
        self.col(x, z).map_or(1.0, |c| self.temp[c])
    }

    pub fn biome(&self, x: i32, z: i32) -> Biome {
        self.col(x, z)
            .map_or(Biome::Plains, |c| Biome::from_u8(self.biome[c]))
    }

    /// Lowest y such that y and everything above has light opacity 0.
    pub fn heightmap(&self, x: i32, z: i32) -> i32 {
        let mut y = HEIGHT as i32 - 1;
        while y >= 0 && opacity(self.get(x, y, z)) == 0 {
            y -= 1;
        }
        y + 1
    }

    /// Vertical sky light at a position (SPEC 12): 15 minus the opacities
    /// above, floored at 0.
    pub fn light(&self, x: i32, y: i32, z: i32) -> i32 {
        let mut sum = 0i32;
        for yy in (y + 1)..HEIGHT as i32 {
            sum += opacity(self.get(x, yy, z)) as i32;
            if sum >= 15 {
                return 0;
            }
        }
        15 - sum
    }
}

// ===========================================================================
// Driver
// ===========================================================================

/// Region of interest for statistics: `[x0, x1) x [z0, z1)`.
#[derive(Clone, Copy)]
struct Area {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
}

impl Area {
    fn contains(&self, x: i32, z: i32) -> bool {
        x >= self.x0 && x < self.x1 && z >= self.z0 && z < self.z1
    }
}

/// Decorate an existing region in place: every cell overlapping it, all four
/// passes, then snow on every column. Writes are clipped to the region.
///
/// Only features fully inside the region see all their neighbours, so the
/// result near the edges depends on the region bounds; use
/// [`crate::generate_region_with_biases`] (which adds a margin and crops) when
/// position-determinism matters.
pub fn decorate(region: &mut Region, gen: &Generator, biases: &Biases) -> DecorationStats {
    let (w, d) = (region.width as i32, region.depth as i32);
    let n = (w * d) as usize;
    let mut temp = Vec::with_capacity(n);
    let mut biome = Vec::with_capacity(n);
    for z in 0..d {
        for x in 0..w {
            let (t, h) = gen.climate.at(
                region.x0 + x,
                region.z0 + z,
                biases.temperature,
                biases.humidity,
            );
            temp.push(t);
            biome.push(biome_for(t, h) as u8);
        }
    }
    let area = Area {
        x0: region.x0,
        z0: region.z0,
        x1: region.x0 + w,
        z1: region.z0 + d,
    };
    let mut world = World {
        blocks: &mut region.blocks,
        x0: area.x0,
        z0: area.z0,
        w,
        d,
        temp: &temp,
        biome: &biome,
        protected: biases.protected,
        pending: Vec::new(),
    };
    run(&mut world, gen, area, 0, &FLORA)
}

/// Generate terrain for the region plus margin, decorate, crop (SPEC 15).
pub(crate) fn generate_decorated(
    gen: &Generator,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &Biases,
) -> (Region, DecorationStats) {
    generate_decorated_with(gen, x0, z0, w, d, biases, &FLORA)
}

/// [`generate_decorated`] with an explicit flora table (tests pass their own).
pub(crate) fn generate_decorated_with(
    gen: &Generator,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    biases: &Biases,
    flora: &[Flora],
) -> (Region, DecorationStats) {
    let mut out = Region {
        blocks: vec![0u8; w as usize * d as usize * HEIGHT],
        width: w,
        depth: d,
        height: HEIGHT as u32,
        x0,
        z0,
    };
    if w == 0 || d == 0 {
        return (out, DecorationStats::default());
    }
    let area = Area {
        x0,
        z0,
        x1: x0 + w as i32,
        z1: z0 + d as i32,
    };
    let pad = MARGIN_CELLS * 16 + READ_REACH;
    let bx0 = x0.div_euclid(16) * 16 - pad;
    let bz0 = z0.div_euclid(16) * 16 - pad;
    let bx1 = (area.x1 - 1).div_euclid(16) * 16 + 16 + pad;
    let bz1 = (area.z1 - 1).div_euclid(16) * 16 + 16 + pad;
    let (bw, bd) = (bx1 - bx0, bz1 - bz0);
    let n = (bw * bd) as usize;
    let mut blocks = vec![0u8; n * HEIGHT];
    let mut temp = vec![0.0f64; n];
    let mut biome = vec![0u8; n];
    gen.for_each_column(
        bx0,
        bz0,
        bw as u32,
        bd as u32,
        biases,
        |cx, cz, col, t, h| {
            let i = cz as usize * bw as usize + cx as usize;
            blocks[i * HEIGHT..(i + 1) * HEIGHT].copy_from_slice(col);
            temp[i] = t;
            biome[i] = biome_for(t, h) as u8;
        },
    );
    let mut world = World {
        blocks: &mut blocks,
        x0: bx0,
        z0: bz0,
        w: bw,
        d: bd,
        temp: &temp,
        biome: &biome,
        protected: biases.protected,
        pending: Vec::new(),
    };
    let stats = run(&mut world, gen, area, MARGIN_CELLS, flora);
    for z in 0..d as usize {
        for x in 0..w as usize {
            let src = ((z + (z0 - bz0) as usize) * bw as usize + x + (x0 - bx0) as usize) * HEIGHT;
            let dst = (z * w as usize + x) * HEIGHT;
            out.blocks[dst..dst + HEIGHT].copy_from_slice(&blocks[src..src + HEIGHT]);
        }
    }
    (out, stats)
}

/// Four parity passes over the cells overlapping `area` (plus `margin`
/// cells, shrinking by one per pass), then the column-local flora pass and the
/// snow pass over `area`.
fn run(
    world: &mut World,
    gen: &Generator,
    area: Area,
    margin: i32,
    flora: &[Flora],
) -> DecorationStats {
    let mut stats = DecorationStats::default();
    let (cx0, cz0) = (cell_index(area.x0), cell_index(area.z0));
    let (cx1, cz1) = (cell_index(area.x1 - 1), cell_index(area.z1 - 1));
    let maxima = Maxima::new();
    for pass in 0..4 {
        let (px, pz) = (pass & 1, pass >> 1);
        let lim = (margin - pass).max(0);
        for cz in (cz0 - lim)..=(cz1 + lim) {
            if cz & 1 != pz {
                continue;
            }
            for cx in (cx0 - lim)..=(cx1 + lim) {
                if cx & 1 != px {
                    continue;
                }
                decorate_cell(world, gen, cx, cz, area, &maxima, &mut stats);
            }
        }
    }
    for z in area.z0..area.z1 {
        for x in area.x0..area.x1 {
            plants::cleanup_column(world, x, z);
        }
    }
    let bounds = (area.x0, area.z0, area.x1, area.z1);
    crate::flora::place(world, gen.seed, bounds, flora);
    for z in area.z0..area.z1 {
        for x in area.x0..area.x1 {
            let b = world.biome(x, z) as usize;
            stats.biomes[b].columns += 1;
            if snow_column(world, x, z, flora) {
                stats.biomes[b].snow += 1;
            }
        }
    }
    stats
}

/// Largest per-cell attempt count over all biomes, per feature.
struct Maxima {
    tree_term: i32,
    plants: [u32; 9],
}

const PLANT_GETTERS: [fn(&Plants) -> f64; 9] = [
    |p| p.dandelion,
    |p| p.tall_grass,
    |p| p.dead_bush,
    |p| p.rose,
    |p| p.brown_mushroom,
    |p| p.red_mushroom,
    |p| p.sugar_cane,
    |p| p.pumpkin,
    |p| p.cactus,
];

impl Maxima {
    fn new() -> Maxima {
        let mut plants = [0u32; 9];
        for (i, g) in PLANT_GETTERS.iter().enumerate() {
            let m = BIOME_INFO
                .iter()
                .map(|b| g(&b.plants))
                .fold(0.0f64, f64::max);
            plants[i] = libm_ceil(m) as u32;
        }
        let tree_term = BIOME_INFO.iter().map(|b| b.tree_bonus).max().unwrap_or(0);
        Maxima { tree_term, plants }
    }
}

fn libm_ceil(x: f64) -> f64 {
    let f = crate::noise::floor(x);
    if f < x {
        f + 1.0
    } else {
        f
    }
}

/// Decoration cell containing world column `x` (cells are 16 wide, aligned
/// to multiples of 16, floor division so negatives round down).
#[inline]
fn cell_index(x: i32) -> i32 {
    x.div_euclid(16)
}

fn cell_rng(seed: u64, cx: i32, cz: i32) -> Rng {
    Rng::new(mix64(
        seed ^ mix64((cx as u32 as u64) | ((cz as u32 as u64) << 32)) ^ 0xDEC0_4A7E_0F_CE11,
    ))
}

/// Whether attempt `k` is kept for a per-cell count `c` (fractions are a
/// final attempt with probability `frac(c)`), given a uniform `u`.
#[inline]
fn keep(k: u32, c: f64, u: f64) -> bool {
    let need = c - k as f64;
    need >= 1.0 || (need > 0.0 && u < need)
}

fn pick_tree(rng: &mut Rng, info: &BiomeInfo) -> TreeKind {
    let total: u32 = info.tree_weights.iter().sum();
    let mut r = rng.below(total.max(1));
    for k in TreeKind::ALL {
        let wgt = info.tree_weights[k as usize];
        if r < wgt {
            return k;
        }
        r -= wgt;
    }
    TreeKind::Oak
}

/// SPEC 10.3 steps 13-22 for one cell.
fn decorate_cell(
    world: &mut World,
    gen: &Generator,
    cx: i32,
    cz: i32,
    area: Area,
    maxima: &Maxima,
    stats: &mut DecorationStats,
) {
    let mut rng = cell_rng(gen.seed, cx, cz);
    let (bx, bz) = (cx * 16, cz * 16);

    // Step 13: trees. Density noise at the cell centre (B4).
    let dn = gen
        .tree
        .sample2d((bx + 8) as f64 * 0.5, (bz + 8) as f64 * 0.5);
    let n = ((dn / 8.0 + 4.0 * rng.next_f64() + 4.0) / 3.0) as i32; // trunc
    let extra = (rng.below(10) == 0) as i32;
    let count = |b: &BiomeInfo| extra + if b.tree_noise { n } else { 0 } + b.tree_bonus;
    let max = extra + n.max(0) + maxima.tree_term;
    for k in 0..max.max(0) {
        let x = bx + rng.below(16) as i32;
        let z = bz + rng.below(16) as i32;
        let biome = world.biome(x, z);
        let info = biome.info();
        if k >= count(info) {
            continue;
        }
        let kind = pick_tree(&mut rng, info);
        let y = world.heightmap(x, z);
        let ok = trees::grow(world, &mut rng, kind, x, y, z);
        if area.contains(x, z) {
            let s = &mut stats.biomes[biome as usize];
            s.tree_attempts += 1;
            s.trees += ok as u64;
        }
    }

    // Steps 14-22: small plants, in spec order.
    for (fi, getter) in PLANT_GETTERS.iter().enumerate() {
        for k in 0..maxima.plants[fi] {
            let u = rng.next_f64();
            let x = bx + rng.below(16) as i32;
            let y = rng.below(HEIGHT as u32) as i32;
            let z = bz + rng.below(16) as i32;
            let biome = world.biome(x, z);
            let info = biome.info();
            if !keep(k, getter(&info.plants), u) {
                continue;
            }
            let placed = match fi {
                0 => plants::flower_patch(world, &mut rng, x, y, z, Block::Dandelion as u8),
                1 => {
                    let fern = rng.next_f64() < info.plants.fern_share;
                    let b = if fern { Block::Fern } else { Block::TallGrass };
                    plants::grass_patch(world, &mut rng, x, y, z, b as u8, 128)
                }
                2 => plants::grass_patch(world, &mut rng, x, y, z, Block::DeadBush as u8, 4),
                3 => plants::flower_patch(world, &mut rng, x, y, z, Block::Rose as u8),
                4 => plants::flower_patch(world, &mut rng, x, y, z, Block::BrownMushroom as u8),
                5 => plants::flower_patch(world, &mut rng, x, y, z, Block::RedMushroom as u8),
                6 => plants::sugar_cane(world, &mut rng, x, y, z),
                7 => plants::pumpkin_patch(world, &mut rng, x, y, z),
                _ => plants::cactus(world, &mut rng, x, y, z),
            };
            if area.contains(x, z) {
                stats.biomes[biome as usize].plants += placed as u64;
            }
        }
    }
}

/// SPEC 13 snow pass for one column. Returns whether snow was placed.
pub(crate) fn snow_column(world: &mut World, x: i32, z: i32, flora: &[Flora]) -> bool {
    let mut top = -1;
    for y in (1..HEIGHT as i32).rev() {
        let b = world.get(x, y, z);
        if blocks_movement(b) || is_liquid(b) {
            top = y + 1;
            break;
        }
    }
    if top <= 0 || top >= HEIGHT as i32 {
        return false;
    }
    let t = world.temperature(x, z) - (top - 64) as f64 / 64.0 * 0.3;
    if t >= 0.5 {
        return false;
    }
    // Snow replaces `cross` and `tall_cross` plants (both halves) unless
    // `snow_stays`; other flora keep their cell and get no snow.
    let here = world.get(x, top, z);
    if here != AIR {
        let Some(f) = flora.iter().find(|f| f.block as u8 == here) else {
            return false;
        };
        if f.snow_stays {
            return false;
        }
        match f.shape {
            FloraShape::Cross => {}
            FloraShape::TallCross => {
                if let Some(up) = f.upper {
                    if world.get(x, top + 1, z) == up as u8 {
                        world.set(x, top + 1, z, AIR);
                    }
                }
            }
            _ => return false,
        }
        world.set(x, top, z, AIR);
    }
    let below = world.get(x, top - 1, z);
    if !blocks_movement(below) || below == ICE {
        return false;
    }
    world.set(x, top, z, SNOW)
}

#[cfg(test)]
mod tests_cells {
    use super::*;

    fn stream(seed: u64, cx: i32, cz: i32) -> [u64; 8] {
        let mut r = cell_rng(seed, cx, cz);
        core::array::from_fn(|_| r.next_u64())
    }

    #[test]
    fn cell_rng_is_pure() {
        assert_eq!(stream(12345, 3, -7), stream(12345, 3, -7));
        assert_eq!(stream(0, 0, 0), stream(0, 0, 0));
    }

    #[test]
    fn cell_rng_differs_by_cell_and_seed() {
        let base = stream(12345, 1, 2);
        assert_ne!(base, stream(12345, 2, 1));
        assert_ne!(stream(12345, -1, 0), stream(12345, 0, -1));
        assert_ne!(stream(12345, 1, 0), stream(12345, -1, 0));
        assert_ne!(stream(12345, 0, 1), stream(12345, 0, -1));
        assert_ne!(base, stream(12346, 1, 2));
        assert_ne!(base, stream(12345, 1, 3));
    }

    #[test]
    fn cell_index_is_floor_division() {
        assert_eq!(cell_index(-1), -1);
        assert_eq!(cell_index(-16), -1);
        assert_eq!(cell_index(-17), -2);
        assert_eq!(cell_index(0), 0);
        assert_eq!(cell_index(15), 0);
        assert_eq!(cell_index(16), 1);
    }
}
