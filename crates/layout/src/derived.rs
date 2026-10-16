//! Derived maps: water depth, bottom block, frozen flag, shore distance
//! (design: world-layout Phase C, 4.5).

use crate::Layout;
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

/// Per-column maps derived from a generated region. Every vector has
/// `width * depth` entries in row order `z * width + x`, like the region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedMaps {
    pub width: u32,
    pub depth: u32,
    /// Contiguous water/ice cells counted down from `SEA_LEVEL - 1`; 0 on land.
    pub water_depth: Vec<u8>,
    /// The block under the water run. Land columns, and a run that reaches
    /// y = 0, hold [`NO_BOTTOM`].
    pub bottom_block: Vec<u8>,
    /// True where the block at `SEA_LEVEL - 1` is ice (sea ice, SPEC 13).
    pub frozen: Vec<bool>,
    /// Euclidean distance between column centres to the nearest land column
    /// (`water_depth == 0`), rounded to the nearest integer and capped at
    /// [`SHORE_DISTANCE_CAP`]. Land is 0 (design: world-layout Phase C, #91).
    ///
    /// [`DerivedMaps::from_region`] only sees its own region, so columns
    /// near its edge may read too high; [`DerivedMaps::generate`] is exact.
    pub shore_distance: Vec<u8>,
}

/// Largest value stored in `shore_distance`.
pub const SHORE_DISTANCE_CAP: u8 = 64;

/// Blocks of extra terrain generated on each side of the requested area so
/// that `shore_distance` does not depend on region origin or size. Land
/// farther than the cap cannot change a capped value.
pub const SHORE_PAD: u32 = SHORE_DISTANCE_CAP as u32;

/// Sentinel for `bottom_block` on columns without a water run.
pub const NO_BOTTOM: u8 = Block::Air as u8;

fn is_water(b: u8) -> bool {
    b == Block::Water as u8 || b == Block::Ice as u8
}

impl DerivedMaps {
    /// Computes the maps for every column of `region`.
    pub fn from_region(region: &Region) -> DerivedMaps {
        let n = region.width as usize * region.depth as usize;
        let mut maps = DerivedMaps {
            width: region.width,
            depth: region.depth,
            water_depth: vec![0; n],
            bottom_block: vec![NO_BOTTOM; n],
            frozen: vec![false; n],
            shore_distance: Vec::new(),
        };
        let surface = (SEA_LEVEL - 1) as usize;
        for (i, col) in region.blocks.chunks_exact(HEIGHT).enumerate() {
            if !is_water(col[surface]) {
                continue;
            }
            maps.frozen[i] = col[surface] == Block::Ice as u8;
            let run = col[..=surface]
                .iter()
                .rev()
                .take_while(|&&b| is_water(b))
                .count();
            maps.water_depth[i] = run as u8;
            if run <= surface {
                maps.bottom_block[i] = col[surface - run];
            }
        }
        maps.shore_distance = shore_distance(&maps.water_depth, region.width, region.depth);
        maps
    }

    /// Maps for the `w x d` area at `(x0, z0)`. Terrain is generated over
    /// the area padded by [`SHORE_PAD`] so every map, shore distance
    /// included, depends only on the seed and world coordinate.
    pub fn generate(layout: &Layout, x0: i32, z0: i32, w: u32, d: u32, decorate: bool) -> Self {
        let pad = SHORE_PAD as i32;
        let region = layout.generate_region(
            x0 - pad,
            z0 - pad,
            w + 2 * SHORE_PAD,
            d + 2 * SHORE_PAD,
            decorate,
        );
        let big = Self::from_region(&region);
        let mut out = DerivedMaps {
            width: w,
            depth: d,
            water_depth: Vec::with_capacity(w as usize * d as usize),
            bottom_block: Vec::with_capacity(w as usize * d as usize),
            frozen: Vec::with_capacity(w as usize * d as usize),
            shore_distance: Vec::with_capacity(w as usize * d as usize),
        };
        for z in 0..d as usize {
            let row = (z + SHORE_PAD as usize) * big.width as usize + SHORE_PAD as usize;
            let r = row..row + w as usize;
            out.water_depth
                .extend_from_slice(&big.water_depth[r.clone()]);
            out.bottom_block
                .extend_from_slice(&big.bottom_block[r.clone()]);
            out.frozen.extend_from_slice(&big.frozen[r.clone()]);
            out.shore_distance.extend_from_slice(&big.shore_distance[r]);
        }
        out
    }

    /// Index of world column `(x, z)`, or `None` outside the region.
    pub fn index_of(&self, region: &Region, x: i32, z: i32) -> Option<usize> {
        let (dx, dz) = (x.checked_sub(region.x0)?, z.checked_sub(region.z0)?);
        (dx >= 0 && dz >= 0 && (dx as u32) < self.width && (dz as u32) < self.depth)
            .then(|| dz as usize * self.width as usize + dx as usize)
    }
}

/// Exact Euclidean distance transform from land columns (`water_depth == 0`),
/// rounded and capped. Nothing outside the grid counts as land.
pub(crate) fn shore_distance(water_depth: &[u8], w: u32, d: u32) -> Vec<u8> {
    let (w, d) = (w as usize, d as usize);
    let inf = f64::INFINITY;
    let mut sq: Vec<f64> = water_depth
        .iter()
        .map(|&v| if v == 0 { 0.0 } else { inf })
        .collect();
    let mut line = vec![0.0; w.max(d)];
    for x in 0..w {
        for z in 0..d {
            line[z] = sq[z * w + x];
        }
        let out = transform_1d(&line[..d]);
        for z in 0..d {
            sq[z * w + x] = out[z];
        }
    }
    for z in 0..d {
        let out = transform_1d(&sq[z * w..(z + 1) * w]);
        sq[z * w..(z + 1) * w].copy_from_slice(&out);
    }
    sq.iter()
        .map(|&s| {
            if s.is_finite() {
                s.sqrt().round().min(SHORE_DISTANCE_CAP as f64) as u8
            } else {
                SHORE_DISTANCE_CAP
            }
        })
        .collect()
}

/// Lower envelope of parabolas: `out[q] = min_p (q - p)^2 + f[p]`.
fn transform_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut out = vec![f64::INFINITY; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let Some(first) = f.iter().position(|x| x.is_finite()) else {
        return out;
    };
    let mut k = 0;
    v[0] = first;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let cross = |q: usize, p: usize| {
        ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * (q as f64 - p as f64))
    };
    for q in first + 1..n {
        if !f[q].is_finite() {
            continue;
        }
        let mut s = cross(q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = cross(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    let mut k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let dq = q as f64 - v[k] as f64;
        *o = dq * dq + f[v[k]];
    }
    out
}
