//! Sea flora validation (docs/design/sea-flora-and-shellfish.md section 4,
//! world-layout 6.1): no sea flora in a channel's reach, a level's spawn
//! water radius or the raft footprint. Measured on a region generated with
//! the sea flora pass, so it needs the `toml-io` feature.

use std::collections::BTreeSet;

use terrain::{ProtectedMask, Region, HEIGHT};

use crate::validation::{raft_footprint_at, CheckResult, ValidationReport};
use crate::{ChannelEval, Layout, Level, SeaFlora};

pub const CHECK_SEA_FLORA_CLEAR: &str = "sea_flora_clear";

/// Count the columns of `region` holding a sea flora block that lie in a
/// channel's reach (weight above 0, which holds the navigable set), within
/// `level.spawn_water_radius` of its anchorage, in a `flora_free` zone or
/// under the level's raft footprint. The check passes at 0.
pub fn validate_sea_flora(region: &Region, layout: &Layout, level: &Level) -> ValidationReport {
    let ids: BTreeSet<u8> = SeaFlora::shipped()
        .entries
        .iter()
        .flat_map(|e| e.blocks.iter().copied())
        .collect();
    let channels: Vec<ChannelEval> = layout.density.iter().filter_map(ChannelEval::new).collect();
    let raft = raft_footprint_at(layout, level.anchorage);
    let [ax, az] = level.anchorage;
    let r2 = level.spawn_water_radius.powi(2);
    let bad = region
        .blocks
        .chunks_exact(HEIGHT)
        .enumerate()
        .filter(|(c, col)| {
            let x = region.x0 + (*c as u32 % region.width) as i32;
            let z = region.z0 + (*c as u32 / region.width) as i32;
            col.iter().any(|b| ids.contains(b))
                && (channels.iter().any(|ch| ch.weight(x, z) > 0.0)
                    || f64::from((x - ax).pow(2) + (z - az).pow(2)) <= r2
                    || layout.flora_free.iter().any(|e| e.protected(x, 0, z))
                    || raft.contains(&(x, z)))
        })
        .count();
    ValidationReport {
        checks: vec![CheckResult {
            name: CHECK_SEA_FLORA_CLEAR,
            measured: bad as f64,
            required: "== 0 flora columns in the channel, spawn water or raft footprint".into(),
            passed: bad == 0,
        }],
    }
}
