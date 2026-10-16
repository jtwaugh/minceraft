//! Region cache with load and unload radii (design: world-layout Phase D).

use crate::{Layout, LayoutRegion, RegionCoord};
use std::collections::HashMap;

/// Which regions an [`RegionCache::update`] call generated and dropped, each
/// list sorted by `(rx, rz)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheChange {
    pub loaded: Vec<RegionCoord>,
    pub unloaded: Vec<RegionCoord>,
}

/// Generated regions of one layout, keyed by coordinate.
pub struct RegionCache {
    layout: Layout,
    decorate: bool,
    regions: HashMap<RegionCoord, LayoutRegion>,
    generations: usize,
}

/// Chebyshev distance between two region coordinates.
fn distance(a: RegionCoord, b: RegionCoord) -> i64 {
    let dx = (a.rx as i64 - b.rx as i64).abs();
    let dz = (a.rz as i64 - b.rz as i64).abs();
    dx.max(dz)
}

impl RegionCache {
    pub fn new(layout: Layout, decorate: bool) -> RegionCache {
        RegionCache {
            layout,
            decorate,
            regions: HashMap::new(),
            generations: 0,
        }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Number of regions generated so far, counting regenerations after an
    /// unload.
    pub fn generations(&self) -> usize {
        self.generations
    }

    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn contains(&self, coord: RegionCoord) -> bool {
        self.regions.contains_key(&coord)
    }

    /// The cached region, if loaded.
    pub fn get(&self, coord: RegionCoord) -> Option<&LayoutRegion> {
        self.regions.get(&coord)
    }

    /// The region at `coord`, generated on first use.
    pub fn get_or_generate(&mut self, coord: RegionCoord) -> &LayoutRegion {
        let (layout, decorate) = (&self.layout, self.decorate);
        let generations = &mut self.generations;
        self.regions.entry(coord).or_insert_with(|| {
            *generations += 1;
            layout.generate_layout_region_at(coord, decorate)
        })
    }

    /// Loads every region within `load_radius` (Chebyshev) of `centre` and
    /// drops those beyond `unload_radius`. Negative `load_radius` counts as 0
    /// and `unload_radius` is raised to at least `load_radius`, so a region
    /// at the edge is never loaded and dropped in the same call.
    pub fn update(
        &mut self,
        centre: RegionCoord,
        load_radius: i32,
        unload_radius: i32,
    ) -> CacheChange {
        let load = load_radius.max(0) as i64;
        let unload = (unload_radius as i64).max(load);

        let mut unloaded: Vec<RegionCoord> = self
            .regions
            .keys()
            .copied()
            .filter(|&c| distance(c, centre) > unload)
            .collect();
        unloaded.sort();
        for c in &unloaded {
            self.regions.remove(c);
        }

        let mut loaded = Vec::new();
        for dx in -load..=load {
            for dz in -load..=load {
                let c = RegionCoord::new(
                    (centre.rx as i64 + dx) as i32,
                    (centre.rz as i64 + dz) as i32,
                );
                if !self.regions.contains_key(&c) {
                    self.get_or_generate(c);
                    loaded.push(c);
                }
            }
        }
        loaded.sort();
        CacheChange { loaded, unloaded }
    }
}
