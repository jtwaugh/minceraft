//! Level validation, island checks (docs/design/world-layout.md 6.1, Phase B).
//!
//! Runs on an **undecorated** region. Thresholds come from
//! `data/validation.toml`.
//!
//! Choices made here:
//! - A column is *land* when its topmost non-air block is neither water nor
//!   ice and sits at `y >= SEA_LEVEL`.
//! - The flood fill uses 4-connectivity (shared edges), so diagonal-only
//!   contact does not join two land patches.
//! - The window is the box holding the island centre and the spawn water
//!   disc, grown by `window_margin` on every side. The fill never leaves it,
//!   so an island larger than the window is measured as the part inside.
//! - The island touches the spawn zone if any island column's centre is
//!   within `spawn_water_radius` of the anchorage (distance <= radius).

use std::fmt;

use terrain::{column_summary, Block, Region, HEIGHT, SEA_LEVEL};

use crate::{is_sea_ice, Layout, Level, Structure, StructurePlacement};

/// Thresholds from `data/validation.toml`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "toml-io", derive(serde::Deserialize))]
pub struct Thresholds {
    pub min_island_columns: u32,
    pub window_margin: i32,
    pub min_raft_water_depth: u32,
    pub raft_clearance: i32,
    pub min_flat_grass_tiles: u32,
    /// Decorated terrain (design 6.1, Phase G): least flat grass tiles with
    /// no decoration above them.
    #[cfg_attr(feature = "toml-io", serde(default = "default_min_clear_flat_tiles"))]
    pub min_clear_flat_tiles: u32,
    /// Decorated terrain: least trees on the island (1 if any log stands on
    /// an island column).
    #[cfg_attr(feature = "toml-io", serde(default = "default_min_trees"))]
    pub min_trees: u32,
    /// Navigability (design 6.2): least water depth of a navigable column.
    #[cfg_attr(feature = "toml-io", serde(default = "default_min_navigable_depth"))]
    pub min_navigable_depth: u32,
    /// Navigability: cells from `SEA_LEVEL` up that must be air.
    #[cfg_attr(feature = "toml-io", serde(default = "default_navigable_clearance"))]
    pub navigable_clearance: i32,
    /// Raft half-width in columns: navigable columns closer than this
    /// (Chebyshev) to a blocked one are eroded before pathfinding.
    #[cfg_attr(feature = "toml-io", serde(default = "default_raft_half_width"))]
    pub raft_half_width: u32,
    /// Retry loop cap (`find_level`).
    #[cfg_attr(feature = "toml-io", serde(default = "default_max_attempts"))]
    pub max_attempts: u32,
}

#[cfg(feature = "toml-io")]
fn default_min_clear_flat_tiles() -> u32 {
    6
}

#[cfg(feature = "toml-io")]
fn default_min_trees() -> u32 {
    1
}

#[cfg(feature = "toml-io")]
fn default_min_navigable_depth() -> u32 {
    2
}

#[cfg(feature = "toml-io")]
fn default_navigable_clearance() -> i32 {
    4
}

#[cfg(feature = "toml-io")]
fn default_raft_half_width() -> u32 {
    2
}

#[cfg(feature = "toml-io")]
fn default_max_attempts() -> u32 {
    64
}

include!(concat!(env!("OUT_DIR"), "/validation.rs"));

impl Thresholds {
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The shipped `data/validation.toml`, baked in at build time.
    pub fn shipped() -> Self {
        SHIPPED_THRESHOLDS.clone()
    }
}

/// One check: what was measured, what was required, and the outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckResult {
    pub name: &'static str,
    pub measured: f64,
    /// Human-readable requirement, e.g. `>= 150`.
    pub required: String,
    pub passed: bool,
}

impl fmt::Display for CheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} (need {}) {}",
            self.name,
            self.measured,
            self.required,
            if self.passed { "pass" } else { "FAIL" }
        )
    }
}

/// All checks run for a level.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidationReport {
    pub checks: Vec<CheckResult>,
}

impl ValidationReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    pub fn check(&self, name: &str) -> Option<&CheckResult> {
        self.checks.iter().find(|c| c.name == name)
    }
}

pub const CHECK_ISLAND_AREA: &str = "island_area";
pub const CHECK_SPAWN_CLEAR: &str = "island_clear_of_spawn_zone";
pub const CHECK_RAFT_WATER: &str = "raft_water_depth";
pub const CHECK_RAFT_CLEAR: &str = "raft_footprint_clear";
pub const CHECK_FLAT_GRASS: &str = "flat_grass_tiles";
pub const CHECK_CLEAR_FLAT: &str = "clear_flat_grass_tiles";
pub const CHECK_TREES: &str = "island_trees";

/// World-coordinate window `(x0, z0, w, d)` a level's checks need.
pub fn window(level: &Level, t: &Thresholds) -> (i32, i32, u32, u32) {
    let r = level.spawn_water_radius.ceil() as i32;
    let [ax, az] = level.anchorage;
    let [ix, iz] = level.island_center;
    let m = t.window_margin;
    let x0 = (ax - r).min(ix) - m;
    let z0 = (az - r).min(iz) - m;
    let x1 = (ax + r).max(ix) + m;
    let z1 = (az + r).max(iz) + m;
    (x0, z0, (x1 - x0 + 1) as u32, (z1 - z0 + 1) as u32)
}

pub(crate) fn is_land(region: &Region, x: u32, z: u32) -> bool {
    let start = region.index(x, 0, z);
    let (top, top_y, _, _) = column_summary(&region.blocks[start..start + HEIGHT]);
    top != Block::Air as u8
        && top != Block::Water as u8
        && !is_sea_ice(top)
        && top_y as i32 >= SEA_LEVEL
}

/// World columns `(x, z)` of the island: land 4-connected to the island
/// centre inside `region`. Empty if the centre is not land (or outside).
pub(crate) fn island_columns(region: &Region, centre: [i32; 2]) -> Vec<(i32, i32)> {
    island_columns_with(region, centre, is_land)
}

/// [`island_columns`] with a custom land test on local `(x, z)`.
pub(crate) fn island_columns_with(
    region: &Region,
    centre: [i32; 2],
    is_land: impl Fn(&Region, u32, u32) -> bool,
) -> Vec<(i32, i32)> {
    let (w, d) = (region.width as i64, region.depth as i64);
    let local = |x: i32, z: i32| (x as i64 - region.x0 as i64, z as i64 - region.z0 as i64);
    let (cx, cz) = local(centre[0], centre[1]);
    if cx < 0 || cz < 0 || cx >= w || cz >= d || !is_land(region, cx as u32, cz as u32) {
        return Vec::new();
    }
    let mut seen = vec![false; (w * d) as usize];
    let mut stack = vec![(cx, cz)];
    seen[(cz * w + cx) as usize] = true;
    let mut out = Vec::new();
    while let Some((x, z)) = stack.pop() {
        out.push((x as i32 + region.x0, z as i32 + region.z0));
        for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
            if nx < 0 || nz < 0 || nx >= w || nz >= d {
                continue;
            }
            let i = (nz * w + nx) as usize;
            if !seen[i] && is_land(region, nx as u32, nz as u32) {
                seen[i] = true;
                stack.push((nx, nz));
            }
        }
    }
    out.sort_unstable();
    out
}

/// Run the island checks for `level` on an undecorated `region`. The region
/// should cover [`window`]; land outside it is not seen.
pub fn validate_island(region: &Region, level: &Level, t: &Thresholds) -> ValidationReport {
    let island = island_columns(region, level.island_center);
    let area = island.len();
    let [ax, az] = level.anchorage;
    let r2 = level.spawn_water_radius * level.spawn_water_radius;
    let inside = island
        .iter()
        .filter(|&&(x, z)| {
            let (dx, dz) = ((x - ax) as f64, (z - az) as f64);
            dx * dx + dz * dz <= r2
        })
        .count();
    ValidationReport {
        checks: vec![
            CheckResult {
                name: CHECK_ISLAND_AREA,
                measured: area as f64,
                required: format!(">= {}", t.min_island_columns),
                passed: area >= t.min_island_columns as usize,
            },
            CheckResult {
                name: CHECK_SPAWN_CLEAR,
                measured: inside as f64,
                required: "== 0 island columns in spawn zone".into(),
                passed: inside == 0,
            },
        ],
    }
}

/// Block at world `(x, y, z)`, or `None` outside the region.
pub(crate) fn block_at(region: &Region, x: i32, y: i32, z: i32) -> Option<u8> {
    let (lx, lz) = (x as i64 - region.x0 as i64, z as i64 - region.z0 as i64);
    if lx < 0 || lz < 0 || lx >= region.width as i64 || lz >= region.depth as i64 {
        return None;
    }
    if !(0..HEIGHT as i32).contains(&y) {
        return None;
    }
    Some(region.blocks[region.index(lx as u32, 0, lz as u32) + y as usize])
}

/// Distinct world columns `(x, z)` under the non-air cells of every
/// `raft = true` placement in `layout`: the raft footprint.
pub fn raft_footprint(layout: &Layout) -> Vec<(i32, i32)> {
    footprint_where(layout, |_| true)
}

/// The raft footprint of the placements at `anchorage` only, so a level in a
/// multi-level layout is not judged on another level's raft.
pub(crate) fn raft_footprint_at(layout: &Layout, anchorage: [i32; 2]) -> Vec<(i32, i32)> {
    footprint_where(layout, |p| [p.origin[0], p.origin[2]] == anchorage)
}

fn footprint_where(layout: &Layout, keep: impl Fn(&StructurePlacement) -> bool) -> Vec<(i32, i32)> {
    let mut cols = Vec::new();
    for p in layout.structures.iter().filter(|p| keep(p)) {
        let Some(s) = Structure::named(&p.structure) else {
            continue;
        };
        if s.raft {
            cols.extend(
                s.solid_cells(p.origin, p.rotation)
                    .iter()
                    .map(|c| (c.0, c.2)),
            );
        }
    }
    cols.sort_unstable();
    cols.dedup();
    cols
}

/// Run the raft checks on terrain **before** the raft is stamped. For each
/// footprint column: water depth is the run of water blocks down from
/// `SEA_LEVEL - 1`, and the column is clear when no solid (non-air,
/// non-water) block lies in `SEA_LEVEL..=SEA_LEVEL + raft_clearance`. The
/// water measure is the shallowest column, the clearance measure the number
/// of blocked columns. A column outside the region fails both, as does an
/// empty footprint.
pub fn validate_raft(
    region: &Region,
    footprint: &[(i32, i32)],
    t: &Thresholds,
) -> ValidationReport {
    let water = Block::Water as u8;
    let air = Block::Air as u8;
    let mut min_depth = u32::MAX;
    let mut blocked = 0u32;
    for &(x, z) in footprint {
        let mut depth = 0;
        while block_at(region, x, SEA_LEVEL - 1 - depth, z) == Some(water) {
            depth += 1;
        }
        min_depth = min_depth.min(depth as u32);
        let clear = (SEA_LEVEL..=SEA_LEVEL + t.raft_clearance)
            .all(|y| matches!(block_at(region, x, y, z), Some(b) if b == air || b == water));
        if !clear {
            blocked += 1;
        }
    }
    if footprint.is_empty() {
        min_depth = 0;
    }
    ValidationReport {
        checks: vec![
            CheckResult {
                name: CHECK_RAFT_WATER,
                measured: min_depth as f64,
                required: format!(">= {} under every footprint column", t.min_raft_water_depth),
                passed: !footprint.is_empty() && min_depth >= t.min_raft_water_depth,
            },
            CheckResult {
                name: CHECK_RAFT_CLEAR,
                measured: blocked as f64,
                required: format!(
                    "== 0 footprint columns with solid blocks in SEA_LEVEL..=SEA_LEVEL+{}",
                    t.raft_clearance
                ),
                passed: !footprint.is_empty() && blocked == 0,
            },
        ],
    }
}

/// Top solid block `(block, y)` of a column, ignoring water and ice.
fn ground(region: &Region, x: i32, z: i32) -> Option<(u8, i32)> {
    (0..HEIGHT as i32).rev().find_map(|y| {
        let b = block_at(region, x, y, z)?;
        (b != Block::Air as u8 && b != Block::Water as u8 && !is_sea_ice(b)).then_some((b, y))
    })
}

/// Count flat grass tiles on the island of an **undecorated** region
/// (Decision 4): island columns whose top block is grass and whose 4
/// neighbours' ground heights are each within 1 block. A neighbour outside
/// the region makes the tile not flat.
pub fn validate_flat_grass(region: &Region, level: &Level, t: &Thresholds) -> ValidationReport {
    let flat = flat_grass_tiles(region, &island_columns(region, level.island_center));
    ValidationReport {
        checks: vec![CheckResult {
            name: CHECK_FLAT_GRASS,
            measured: flat as f64,
            required: format!(">= {}", t.min_flat_grass_tiles),
            passed: flat >= t.min_flat_grass_tiles as usize,
        }],
    }
}

/// Flat grass tiles among `island` columns (see [`validate_flat_grass`]).
pub(crate) fn flat_grass_tiles(region: &Region, island: &[(i32, i32)]) -> usize {
    flat_grass_tiles_with(region, island, ground, |_, _| true)
}

/// [`flat_grass_tiles`] with the ground finder chosen by the caller:
/// `ground_of` finds a column's ground block (the decorated check looks
/// through decoration); `keep` drops tiles, but not their neighbours.
pub(crate) fn flat_grass_tiles_with(
    region: &Region,
    island: &[(i32, i32)],
    ground_of: impl Fn(&Region, i32, i32) -> Option<(u8, i32)>,
    keep: impl Fn(i32, i32) -> bool,
) -> usize {
    island
        .iter()
        .filter(|&&(x, z)| {
            if !keep(x, z) {
                return false;
            }
            let Some((b, y)) = ground_of(region, x, z) else {
                return false;
            };
            b == Block::Grass as u8
                && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dz)| {
                    ground_of(region, x + dx, z + dz).is_some_and(|(_, ny)| (ny - y).abs() <= 1)
                })
        })
        .count()
}

/// Generate the undecorated window for `level` under `layout` and validate.
/// The layout's structures are left out, so the raft checks see the terrain
/// before the raft is stamped.
pub fn validate_level(layout: &Layout, level: &Level, t: &Thresholds) -> ValidationReport {
    let (x0, z0, w, d) = window(level, t);
    let bare = Layout {
        structures: Vec::new(),
        ..layout.clone()
    };
    let region = bare.generate_region(x0, z0, w, d, false);
    let mut report = validate_island(&region, level, t);
    report
        .checks
        .extend(validate_raft(&region, &raft_footprint_at(layout, level.anchorage), t).checks);
    report
        .checks
        .extend(validate_flat_grass(&region, level, t).checks);
    report
}
