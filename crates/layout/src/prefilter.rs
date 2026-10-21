//! Seed prefilter (docs/design/world-layout.md Phase F, stage 1; 6.1): reject
//! a candidate seed from a coarse density survey before the full-resolution
//! validation.
//!
//! Choices made here:
//! - The survey is `generate_coarse_density` (every 4 blocks, `y` from
//!   `SEA_LEVEL - 8` to `SEA_LEVEL + 8`) with the layout's biases.
//! - Only corridor regions that a rule reads are sampled: those holding an
//!   anchorage-area sample or an island-centre column. Every one of them is
//!   inside the `Corridor`, and a rejection costs a handful of regions
//!   instead of the whole corridor.
//! - "Anchorage area" is the spawn water zone: the samples within
//!   `spawn_water_radius` of the anchorage, plus the sample nearest the
//!   anchorage. "Solid" is a solid sample at `y >= SEA_LEVEL` in that column.
//! - "Island centre underwater" is no solid sample at `y >= SEA_LEVEL` in the
//!   sample column nearest the centre (ties round up).
//! - Samples are on multiples of 4, so a one-sample feature can be missed;
//!   the filter only rejects, it never accepts a seed for validation.

use std::collections::{BTreeMap, BTreeSet};

use terrain::{generate_coarse_density, SEA_LEVEL};

use crate::{build_route, build_route_layout, Corridor, Layout, Level, RegionCoord, Route};
use crate::{RouteParams, WorldConfig, REGION_SIZE};

const STEP: i32 = 4;
const BAND: i32 = 8;

/// Why a seed was rejected; `level` is the index into the levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefilterReject {
    /// Land at or above sea level in the anchorage's spawn water zone.
    AnchorageSolid { level: usize },
    /// No land at or above sea level at the island centre.
    IslandUnderwater { level: usize },
}

/// The nearest multiple of 4 to `v`, ties up.
fn snap(v: i32) -> i32 {
    (v + STEP / 2).div_euclid(STEP) * STEP
}

fn region_of(x: i32, z: i32) -> RegionCoord {
    RegionCoord::containing(x, z)
}

/// Sample columns `(x, z)` the anchorage rule reads for `level`.
fn anchorage_columns(level: &Level) -> Vec<(i32, i32)> {
    let [ax, az] = level.anchorage;
    let r = level.spawn_water_radius;
    let reach = r.ceil() as i32 + STEP;
    let mut cols = vec![(snap(ax), snap(az))];
    for x in ((ax - reach).div_euclid(STEP)..=(ax + reach).div_euclid(STEP)).map(|i| i * STEP) {
        for z in ((az - reach).div_euclid(STEP)..=(az + reach).div_euclid(STEP)).map(|i| i * STEP) {
            let (dx, dz) = ((x - ax) as f64, (z - az) as f64);
            if dx * dx + dz * dz <= r * r {
                cols.push((x, z));
            }
        }
    }
    cols.sort_unstable();
    cols.dedup();
    cols
}

fn centre_column(level: &Level) -> (i32, i32) {
    (snap(level.island_center[0]), snap(level.island_center[1]))
}

/// The regions [`prefilter_layout`] samples for `levels`, all inside
/// `corridor`, sorted.
pub fn prefilter_regions(levels: &[Level], corridor: &Corridor) -> Vec<RegionCoord> {
    let mut set = BTreeSet::new();
    for l in levels {
        let (cx, cz) = centre_column(l);
        let cols = anchorage_columns(l).into_iter().chain([(cx, cz)]);
        set.extend(cols.map(|(x, z)| region_of(x, z)));
    }
    set.into_iter().filter(|c| corridor.contains(*c)).collect()
}

/// Per sampled region, whether each sample column has a solid sample at
/// `y >= SEA_LEVEL`.
struct Survey {
    land: BTreeMap<RegionCoord, Vec<bool>>,
}

impl Survey {
    fn take(layout: &Layout, regions: &[RegionCoord]) -> Survey {
        let n = REGION_SIZE / STEP as u32;
        let land = regions
            .iter()
            .map(|&c| {
                let (ox, oz) = c.origin();
                let (flags, ny) = layout.with_biases(
                    |_| true,
                    |b| {
                        generate_coarse_density(
                            layout.seed,
                            ox,
                            oz,
                            n,
                            n,
                            (SEA_LEVEL - BAND, SEA_LEVEL + BAND),
                            b,
                        )
                    },
                );
                let n = n as usize;
                // Sample `j` is at y = lowest + 4j; skip those below sea level.
                let lowest = (SEA_LEVEL - BAND + STEP - 1).div_euclid(STEP) * STEP;
                let first_above = ((SEA_LEVEL - lowest + STEP - 1).div_euclid(STEP).max(0)
                    as usize)
                    .min(ny as usize);
                let mut column = vec![false; n * n];
                for j in first_above..ny as usize {
                    for (i, c) in column.iter_mut().enumerate() {
                        *c |= flags[j * n * n + i];
                    }
                }
                (c, column)
            })
            .collect();
        Survey { land }
    }

    /// Whether the sample column at world `(x, z)` (multiples of 4) has land.
    fn land(&self, x: i32, z: i32) -> bool {
        let c = region_of(x, z);
        let (ox, oz) = c.origin();
        let n = REGION_SIZE as i32 / STEP;
        let i = (x - ox) / STEP;
        let k = (z - oz) / STEP;
        self.land[&c][(k * n + i) as usize]
    }
}

/// Run the rules on an already built `layout` for `levels` along `route`.
pub fn prefilter_layout(
    layout: &Layout,
    levels: &[Level],
    route: &Route,
) -> Result<(), PrefilterReject> {
    let corridor = Corridor::new(route, RouteParams::shipped().corridor_margin);
    let survey = Survey::take(layout, &prefilter_regions(levels, &corridor));
    for (level, l) in levels.iter().enumerate() {
        if anchorage_columns(l).iter().any(|&(x, z)| survey.land(x, z)) {
            return Err(PrefilterReject::AnchorageSolid { level });
        }
        let (x, z) = centre_column(l);
        if !survey.land(x, z) {
            return Err(PrefilterReject::IslandUnderwater { level });
        }
    }
    Ok(())
}

/// Prefilter candidate `seed` for a route of `k` further anchorages.
pub fn prefilter(seed: u64, k: usize) -> Result<(), PrefilterReject> {
    let route = build_route(seed, k);
    let (layout, levels) = build_route_layout(&WorldConfig { seed }, &route);
    prefilter_layout(&layout, &levels, &route)
}
