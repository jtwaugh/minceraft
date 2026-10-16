use super::*;

fn layout() -> Layout {
    Layout {
        seed: 7,
        density: vec![DensityTerm::RadialBaseHeight {
            center: (0.0, 0.0),
            r_inner: 24.0,
            r_outer: 64.0,
            delta: 30.0,
        }],
        structures: vec![StructurePlacement {
            structure: "raft_start".to_string(),
            origin: [-4, 70, 5],
            rotation: 0,
        }],
        ..Layout::default()
    }
}

fn c(rx: i32, rz: i32) -> RegionCoord {
    RegionCoord::new(rx, rz)
}

fn same(a: &LayoutRegion, b: &LayoutRegion) -> bool {
    a.region.blocks == b.region.blocks && a.raft_cells == b.raft_cells
}

#[test]
fn second_get_does_not_regenerate() {
    let mut cache = RegionCache::new(layout(), false);
    let first = cache.get_or_generate(c(-1, 0)).region.blocks.clone();
    assert_eq!(cache.generations(), 1);
    let second = &cache.get_or_generate(c(-1, 0)).region.blocks;
    assert_eq!(&first, second);
    assert_eq!(cache.generations(), 1);
    assert_eq!(cache.len(), 1);
}

#[test]
fn cached_region_matches_direct_generation() {
    let l = layout();
    let direct = l.generate_layout_region_at(c(0, -1), false);
    let mut cache = RegionCache::new(l, false);
    assert!(same(cache.get_or_generate(c(0, -1)), &direct));
}

#[test]
fn update_loads_the_square_and_only_the_difference_on_move() {
    let mut cache = RegionCache::new(Layout::new(3), false);
    let ch = cache.update(c(0, 0), 1, 1);
    assert_eq!(ch.loaded.len(), 9);
    assert!(ch.unloaded.is_empty());
    assert_eq!(cache.len(), 9);
    assert_eq!(cache.generations(), 9);

    let ch = cache.update(c(1, 0), 1, 1);
    assert_eq!(ch.loaded, vec![c(2, -1), c(2, 0), c(2, 1)]);
    assert_eq!(ch.unloaded, vec![c(-1, -1), c(-1, 0), c(-1, 1)]);
    assert_eq!(cache.len(), 9);
    assert_eq!(cache.generations(), 12);

    let ch = cache.update(c(1, 0), 1, 1);
    assert_eq!(ch, CacheChange::default());
}

#[test]
fn regions_between_radii_stay_loaded() {
    let mut cache = RegionCache::new(Layout::new(3), false);
    cache.update(c(0, 0), 1, 2);
    let ch = cache.update(c(2, 0), 1, 2);
    // x = -1 is at distance 3 (dropped); x = 0 is at distance 2 (kept).
    assert!(cache.contains(c(0, 0)));
    assert!(!cache.contains(c(-1, 0)));
    assert!(ch.unloaded.iter().all(|u| u.rx == -1));
    assert_eq!(ch.unloaded.len(), 3);
    assert!(ch.loaded.iter().all(|l| l.rx >= 2));
    assert_eq!(ch.loaded.len(), 6);
}

#[test]
fn unload_radius_below_load_radius_is_raised() {
    let mut cache = RegionCache::new(Layout::new(3), false);
    let ch = cache.update(c(0, 0), 1, 0);
    assert_eq!(ch.loaded.len(), 9);
    assert!(ch.unloaded.is_empty());
    assert_eq!(cache.update(c(0, 0), 1, 0), CacheChange::default());
}

#[test]
fn negative_load_radius_counts_as_zero() {
    let mut cache = RegionCache::new(Layout::new(3), false);
    assert_eq!(cache.update(c(4, 4), -2, 0).loaded, vec![c(4, 4)]);
}

#[test]
fn reloaded_region_is_identical_after_eviction() {
    for decorate in [false, true] {
        let mut cache = RegionCache::new(layout(), decorate);
        cache.update(c(0, 0), 0, 0);
        let first = cache
            .get(c(0, 0))
            .map(|r| (r.region.blocks.clone(), r.raft_cells.clone()));
        let first = first.unwrap();

        let ch = cache.update(c(5, 5), 0, 0);
        assert_eq!(ch.unloaded, vec![c(0, 0)]);
        assert!(!cache.contains(c(0, 0)));

        let ch = cache.update(c(0, 0), 0, 0);
        assert_eq!(ch.loaded, vec![c(0, 0)]);
        let again = cache.get(c(0, 0)).unwrap();
        assert_eq!(first.0, again.region.blocks, "decorate {decorate}");
        assert_eq!(first.1, again.raft_cells, "decorate {decorate}");
        assert_eq!(cache.generations(), 3);
    }
}
