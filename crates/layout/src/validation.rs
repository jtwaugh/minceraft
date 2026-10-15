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

use serde::Deserialize;
use terrain::{column_summary, Block, Region, HEIGHT, SEA_LEVEL};

use crate::{Layout, Level};

const DEFAULT_THRESHOLDS: &str = include_str!("../../../data/validation.toml");

/// Thresholds from `data/validation.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Thresholds {
    pub min_island_columns: u32,
    pub window_margin: i32,
}

impl Thresholds {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The shipped `data/validation.toml`.
    pub fn shipped() -> Self {
        Self::from_toml(DEFAULT_THRESHOLDS).expect("data/validation.toml is valid")
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

fn is_land(region: &Region, x: u32, z: u32) -> bool {
    let start = region.index(x, 0, z);
    let (top, top_y, _, _) = column_summary(&region.blocks[start..start + HEIGHT]);
    top != Block::Air as u8
        && top != Block::Water as u8
        && top != Block::Ice as u8
        && top_y as i32 >= SEA_LEVEL
}

/// World columns `(x, z)` of the island: land 4-connected to the island
/// centre inside `region`. Empty if the centre is not land (or outside).
fn island_columns(region: &Region, centre: [i32; 2]) -> Vec<(i32, i32)> {
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

/// Generate the undecorated window for `level` under `layout` and validate.
pub fn validate_level(layout: &Layout, level: &Level, t: &Thresholds) -> ValidationReport {
    let (x0, z0, w, d) = window(level, t);
    let region = layout.generate_region(x0, z0, w, d, false);
    validate_island(&region, level, t)
}
