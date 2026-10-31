//! Habitat, temperature and water depth per column over the streamed
//! regions. See docs/design/fish-and-fishing.md 3.1 and reconciliation note 4.

use layout::{
    FishColumn, HabitatId, HabitatMap, RegionCache, RegionCoord, NO_HABITAT, REGION_SIZE,
};
use std::collections::HashMap;

/// One [`HabitatMap`] per loaded region, built on first query.
#[derive(Default)]
pub struct FishColumns {
    maps: HashMap<RegionCoord, HabitatMap>,
}

impl FishColumns {
    /// The column at world `(x, z)`, or `None` on land or outside the
    /// regions loaded in `cache`. Depends only on the seed and `(x, z)`.
    pub fn column(&mut self, cache: &RegionCache, x: i32, z: i32) -> Option<FishColumn> {
        let coord = RegionCoord::containing(x, z);
        if !cache.contains(coord) {
            return None;
        }
        let map = self.maps.entry(coord).or_insert_with(|| {
            let (x0, z0) = coord.origin();
            cache.layout().habitat_map(x0, z0, REGION_SIZE, REGION_SIZE)
        });
        let i = ((z - map.z0) as u32 * map.width + (x - map.x0) as u32) as usize;
        let habitat = map.habitat[i];
        (habitat != NO_HABITAT).then(|| FishColumn {
            habitat: HabitatId(usize::from(habitat)),
            temperature: f64::from(map.temperature[i]),
            depth: u32::from(map.derived.water_depth[i]),
        })
    }

    /// Drops maps of regions no longer loaded in `cache`.
    pub fn retain_loaded(&mut self, cache: &RegionCache) {
        self.maps.retain(|c, _| cache.contains(*c));
    }

    /// Number of cached region maps.
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout::Layout;

    fn layout() -> Layout {
        Layout {
            seed: 25,
            ..Layout::default()
        }
    }

    /// A cache with the given regions loaded.
    fn cache_with(coords: &[RegionCoord]) -> RegionCache {
        let mut cache = RegionCache::new(layout(), false);
        for &c in coords {
            cache.get_or_generate(c);
        }
        cache
    }

    #[test]
    fn columns_match_a_direct_query_and_land_or_unloaded_is_none() {
        let cache = cache_with(&[RegionCoord::new(0, 0)]);
        let mut cols = FishColumns::default();
        let direct = layout().habitat_map(-20, -20, 200, 200);
        let (mut water, mut land) = (0, 0);
        for z in (0..128).step_by(3) {
            for x in (0..128).step_by(3) {
                let i = ((z + 20) * 200 + (x + 20)) as usize;
                let got = cols.column(&cache, x, z);
                if direct.habitat[i] == NO_HABITAT {
                    land += 1;
                    assert_eq!(got, None, "({x}, {z})");
                    continue;
                }
                water += 1;
                let want = FishColumn {
                    habitat: HabitatId(usize::from(direct.habitat[i])),
                    temperature: f64::from(direct.temperature[i]),
                    depth: u32::from(direct.derived.water_depth[i]),
                };
                assert_eq!(got, Some(want), "({x}, {z})");
            }
        }
        assert!(water > 0 && land > 0, "{water} water, {land} land");
        assert_eq!(cols.column(&cache, -1, 5), None, "unloaded");
        assert_eq!(cols.column(&cache, 128, 5), None, "unloaded");
        assert_eq!(cols.len(), 1);
    }

    #[test]
    fn overlapping_region_sets_agree() {
        let a = cache_with(&[RegionCoord::new(0, 0), RegionCoord::new(1, 0)]);
        let b = cache_with(&[RegionCoord::new(1, 0), RegionCoord::new(1, 1)]);
        let (mut ca, mut cb) = (FishColumns::default(), FishColumns::default());
        let mut water = 0;
        for z in (0..128).step_by(2) {
            for x in (128..256).step_by(2) {
                let got = ca.column(&a, x, z);
                assert_eq!(got, cb.column(&b, x, z), "({x}, {z})");
                water += usize::from(got.is_some());
            }
        }
        assert!(water > 0);
        assert_eq!(cb.column(&b, 5, 5), None, "not loaded in b");
    }

    #[test]
    fn unloaded_regions_are_dropped() {
        let mut cache = cache_with(&[RegionCoord::new(0, 0), RegionCoord::new(3, 0)]);
        let mut cols = FishColumns::default();
        cols.column(&cache, 1, 1);
        cols.column(&cache, 3 * 128 + 1, 1);
        assert_eq!(cols.len(), 2);
        cache.update(RegionCoord::new(0, 0), 0, 0);
        cols.retain_loaded(&cache);
        assert_eq!(cols.len(), 1);
    }
}
