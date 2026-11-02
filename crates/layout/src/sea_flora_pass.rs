//! Sea flora placement (docs/design/sea-flora-and-shellfish.md sections 3
//! and 4): a pass over the blocks `Layout::generate_layout_region` returns.
//! Both `cluster` (shellfish) and `column` (kelp) entries are placed.
//!
//! Choices made here:
//! - A column is skipped when it is frozen or `deep_water`, within a
//!   channel's reach (weight above 0, which holds every navigable channel
//!   cell), in a `flora_free` zone (spawn water) or under the raft footprint.
//! - Entries are tried in file order; the first that fits the column and
//!   passes its patch noise and per-column hash takes it.
//! - The flora block replaces the lowest water cell, on the bottom block.
//! - A kelp column's height is hashed from the seed and the column; one
//!   that would leave fewer than `surface_gap_min` water cells above it is
//!   shortened to fit, and skipped if that is under `height_min`.

use terrain::noise::{hash_f64, mix64, OctavePerlin, Rng};
use terrain::{Block, ProtectedMask, Region, HEIGHT, SEA_LEVEL};

use crate::{
    raft_footprint, ChannelEval, DerivedMaps, HabitatMap, Habitats, Layout, SeaFlora,
    SeaFloraEntry, SeaFloraShape, NO_HABITAT, SHORE_PAD,
};

/// Habitats nothing grows in (section 3).
const BARREN: [&str; 2] = ["frozen", "deep_water"];

/// Patch noise for one entry, from the seed and its salt.
fn patch_noise(seed: u64, e: &SeaFloraEntry) -> OctavePerlin {
    OctavePerlin::new(&mut Rng::new(mix64(seed ^ mix64(u64::from(e.salt)))), 2)
}

/// Whether `e` grows in column `i` of `map`.
fn fits(e: &SeaFloraEntry, map: &HabitatMap, i: usize) -> bool {
    let m = &map.derived;
    let depth = u32::from(m.water_depth[i]);
    e.habitats
        .iter()
        .any(|h| h.0 == usize::from(map.habitat[i]))
        && e.bottom.contains(&m.bottom_block[i])
        && (e.depth_min..=e.depth_max).contains(&depth)
        && e.shore_distance_max
            .is_none_or(|s| u32::from(m.shore_distance[i]) <= s)
}

/// The block-copy of the `w x d` area at `(x0, z0)` inside `big`.
fn crop(big: &Region, x0: i32, z0: i32, w: u32, d: u32) -> Region {
    let mut blocks = Vec::with_capacity(w as usize * d as usize * HEIGHT);
    for z in 0..d {
        let start = big.index((x0 - big.x0) as u32, 0, (z0 - big.z0) as u32 + z);
        blocks.extend_from_slice(&big.blocks[start..start + w as usize * HEIGHT]);
    }
    Region {
        blocks,
        width: w,
        depth: d,
        x0,
        z0,
        ..*big
    }
}

impl Layout {
    /// [`Layout::generate_region`] plus sea flora. The terrain padded for
    /// shore distance is generated once; undecorated output is cut from it.
    pub(crate) fn generate_with_sea_flora(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        decorate: bool,
    ) -> Region {
        let (pad, w2, d2) = (SHORE_PAD as i32, w + 2 * SHORE_PAD, d + 2 * SHORE_PAD);
        let padded = self.generate_region(x0 - pad, z0 - pad, w2, d2, false);
        let habitats = Habitats::shipped();
        let map = self.habitat_map_from(&habitats, DerivedMaps::from_padded(&padded), x0, z0);
        let mut region = if decorate {
            self.generate_region(x0, z0, w, d, true)
        } else {
            crop(&padded, x0, z0, w, d)
        };
        self.place_sea_flora(&mut region, &map, &SeaFlora::shipped(), &habitats);
        region
    }

    /// Replace water cells of `region` with sea flora; `map` is the habitat
    /// map of the same area.
    pub(crate) fn place_sea_flora(
        &self,
        region: &mut Region,
        map: &HabitatMap,
        flora: &SeaFlora,
        habitats: &Habitats,
    ) {
        let barren: Vec<usize> = BARREN
            .iter()
            .filter_map(|n| habitats.id_of(n))
            .map(|h| h.0)
            .collect();
        let channels: Vec<ChannelEval> = self.density.iter().filter_map(ChannelEval::new).collect();
        let raft = raft_footprint(self);
        let noises: Vec<OctavePerlin> = flora
            .entries
            .iter()
            .map(|e| patch_noise(self.seed, e))
            .collect();
        for lz in 0..region.depth {
            for lx in 0..region.width {
                let i = (lz * region.width + lx) as usize;
                let (x, z) = (region.x0 + lx as i32, region.z0 + lz as i32);
                let h = map.habitat[i];
                if h == NO_HABITAT
                    || map.derived.frozen[i]
                    || barren.contains(&usize::from(h))
                    || channels.iter().any(|c| c.weight(x, z) > 0.0)
                    || self.flora_free.iter().any(|e| e.protected(x, 0, z))
                    || raft.binary_search(&(x, z)).is_ok()
                {
                    continue;
                }
                let y = SEA_LEVEL - i32::from(map.derived.water_depth[i]);
                let cell = region.index(lx, y as u32, lz);
                if region.blocks[cell] != Block::Water as u8 {
                    continue;
                }
                let grows = |(e, n): &(&SeaFloraEntry, &OctavePerlin)| {
                    if !fits(e, map, i) {
                        return false;
                    }
                    let v = n.sample2d(x as f64 / e.cluster_scale, z as f64 / e.cluster_scale);
                    let patch = (v * 0.5 + 0.5).clamp(0.0, 1.0) * 2.0;
                    hash_f64(self.seed, x, z, y, u64::from(e.salt)) < e.chance * patch
                };
                let Some((e, _)) = flora.entries.iter().zip(&noises).find(grows) else {
                    continue;
                };
                let SeaFloraShape::Column(c) = e.shape else {
                    region.blocks[cell] = e.blocks[0];
                    continue;
                };
                let span = c.height_max - c.height_min + 1;
                let r = hash_f64(self.seed, x, z, y, u64::from(e.salt) | 1 << 32);
                let tall = c.height_min + (r * f64::from(span)) as u32;
                let fit = u32::from(map.derived.water_depth[i]).saturating_sub(c.surface_gap_min);
                let height = tall.min(fit).min(c.height_max);
                let top = y + height as i32 - 1;
                if height >= c.height_min
                    && (y..=top).all(|yy| {
                        region.blocks[region.index(lx, yy as u32, lz)] == Block::Water as u8
                    })
                {
                    for yy in y..top {
                        let c = region.index(lx, yy as u32, lz);
                        region.blocks[c] = e.blocks[0];
                    }
                    let c = region.index(lx, top as u32, lz);
                    region.blocks[c] = e.blocks[1];
                }
            }
        }
    }
}
