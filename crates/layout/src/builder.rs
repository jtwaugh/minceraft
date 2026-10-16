//! `build_layout`: the single-level layout (docs/design/world-layout.md
//! Phase B, 4.1, 4.2).
//!
//! Per level: an island bump at `island_center`, a spawn-water dip at the
//! anchorage, an exclusion circle around the raft, and the `raft_start`
//! placement with its deck at `SEA_LEVEL`.
//!
//! Tuned numbers were measured with `cargo run -p layout --release --example
//! layout_stats` over seeds 1..=200 (see the PR for the distribution).

use crate::{DensityTerm, Exclusion, Layout, Level, Structure, StructurePlacement, WorldConfig};
use terrain::noise::mix64;
use terrain::SEA_LEVEL;

/// Island bump: full effect inside `ISLAND_R_INNER`, none beyond
/// `ISLAND_R_OUTER` (design doc defaults).
pub const ISLAND_R_INNER: f64 = 8.0;
pub const ISLAND_R_OUTER: f64 = 20.0;
/// Base-height delta of the island bump. Tuned so the centre column's top
/// has a median of about `SEA_LEVEL + 6` over seeds 1..=200.
pub const ISLAND_DELTA: f64 = 9.0;

/// The spawn-water dip fades out over this many blocks beyond
/// `spawn_water_radius`.
pub const SPAWN_WATER_FALLOFF: f64 = 12.0;
/// Base-height delta (negative) of the spawn-water dip. Tuned so water under
/// the raft is at least 3 deep for most seeds in 1..=200.
pub const SPAWN_WATER_DELTA: f64 = -40.0;

/// Blocks of exclusion beyond the raft footprint.
pub const RAFT_EXCLUSION_MARGIN: f64 = 2.0;

/// Default v0 placement: island centre distance range from the anchorage.
pub const V0_ISLAND_DIST_MIN: f64 = 24.0;
pub const V0_ISLAND_DIST_MAX: f64 = 32.0;
/// Default v0 spawn water radius.
pub const V0_SPAWN_WATER_RADIUS: f64 = 8.0;

/// SplitMix64 step: a stateless hash, so placement is a pure function of
/// the seed.
fn mix(z: u64) -> u64 {
    mix64(z.wrapping_add(0x9E37_79B9_7F4A_7C15))
}

/// Unit float in [0, 1) from the hash of `seed` and a stream number.
fn unit(seed: u64, stream: u64) -> f64 {
    (mix(seed ^ mix(stream)) >> 11) as f64 / (1u64 << 53) as f64
}

/// The default v0 level: anchorage at the world origin, island centre 24 to
/// 32 blocks away in a direction (and distance) hashed from `seed`.
pub fn default_v0_level(seed: u64) -> Level {
    let angle = unit(seed, 1) * std::f64::consts::TAU;
    let dist = V0_ISLAND_DIST_MIN + unit(seed, 2) * (V0_ISLAND_DIST_MAX - V0_ISLAND_DIST_MIN);
    Level {
        id: "v0".into(),
        week: 1,
        anchorage: [0, 0],
        island_center: [
            (dist * angle.cos()).round() as i32,
            (dist * angle.sin()).round() as i32,
        ],
        spawn_water_radius: V0_SPAWN_WATER_RADIUS,
    }
}

/// Radius of the raft exclusion: the farthest raft cell from the origin plus
/// the margin (the footprint turns about the origin, so any rotation fits).
fn raft_exclusion_radius() -> f64 {
    Structure::raft_start()
        .solid_cells([0, 0, 0], 0)
        .iter()
        .map(|&(x, _, z, _)| (x as f64).hypot(z as f64))
        .fold(0.0, f64::max)
        + RAFT_EXCLUSION_MARGIN
}

/// Build the layout for `levels` under `world`.
pub fn build_layout(world: &WorldConfig, levels: &[Level]) -> Layout {
    let mut layout = Layout::new(world.seed);
    let radius = raft_exclusion_radius();
    for l in levels {
        let anchor = (l.anchorage[0] as f64, l.anchorage[1] as f64);
        layout.density.push(DensityTerm::RadialBaseHeight {
            center: (l.island_center[0] as f64, l.island_center[1] as f64),
            r_inner: ISLAND_R_INNER,
            r_outer: ISLAND_R_OUTER,
            delta: ISLAND_DELTA,
        });
        layout.density.push(DensityTerm::RadialBaseHeight {
            center: anchor,
            r_inner: l.spawn_water_radius,
            r_outer: l.spawn_water_radius + SPAWN_WATER_FALLOFF,
            delta: SPAWN_WATER_DELTA,
        });
        layout.exclusions.push(Exclusion::Circle {
            center: anchor,
            radius,
        });
        layout.structures.push(StructurePlacement {
            structure: "raft_start".into(),
            origin: [l.anchorage[0], SEA_LEVEL, l.anchorage[1]],
            rotation: 0,
        });
    }
    layout
}
