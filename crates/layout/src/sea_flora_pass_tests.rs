use std::collections::BTreeMap;

use terrain::{Region, HEIGHT, SEA_LEVEL};

use crate::{
    build_route, build_route_layout, raft_footprint, ChannelEval, Habitats, Layout, Level,
    SeaFlora, WorldConfig, NO_HABITAT,
};
use terrain::ProtectedMask;

const SEEDS: [u64; 3] = [9, 12345, 31];
/// Area around the first anchorage: spawn water, raft, channel and open sea.
const AREA: (i32, i32, u32, u32) = (-96, -96, 192, 192);

fn route_layout(seed: u64) -> (Layout, Vec<Level>) {
    build_route_layout(&WorldConfig { seed }, &build_route(seed, 2))
}

/// Shellfish block id to entry name.
fn shellfish() -> BTreeMap<u8, String> {
    SeaFlora::shipped()
        .entries
        .iter()
        .map(|e| (e.blocks[0], e.name.clone()))
        .filter(|(_, n)| n != "kelp")
        .collect()
}

/// Every cell `(x, y, z, block)` whose block is a shellfish.
fn placed(r: &Region) -> Vec<(i32, i32, i32, u8)> {
    let ids = shellfish();
    let mut out = Vec::new();
    for (c, col) in r.blocks.chunks_exact(HEIGHT).enumerate() {
        let (x, z) = (
            r.x0 + (c as u32 % r.width) as i32,
            r.z0 + (c as u32 / r.width) as i32,
        );
        for (y, &b) in col.iter().enumerate() {
            if ids.contains_key(&b) {
                out.push((x, y as i32, z, b));
            }
        }
    }
    out
}

#[test]
fn every_shellfish_sits_where_its_entry_allows() {
    let flora = SeaFlora::shipped();
    let habitats = Habitats::shipped();
    let ids = shellfish();
    let mut kinds = BTreeMap::new();
    for seed in SEEDS {
        let (l, _) = route_layout(seed);
        let (x0, z0, w, d) = AREA;
        let r = l.generate_layout_region(x0, z0, w, d, true).region;
        let map = l.habitat_map(x0, z0, w, d);
        for (x, y, z, b) in placed(&r) {
            let e = flora.get(&ids[&b]).unwrap();
            let i = ((z - z0) as u32 * w + (x - x0) as u32) as usize;
            let depth = u32::from(map.derived.water_depth[i]);
            assert_ne!(map.habitat[i], NO_HABITAT);
            let h = &habitats.rules[usize::from(map.habitat[i])].id;
            assert!(
                e.habitats.iter().any(|id| habitats.rules[id.0].id == *h),
                "{h}"
            );
            assert!(e.bottom.contains(&map.derived.bottom_block[i]));
            assert!((e.depth_min..=e.depth_max).contains(&depth), "{depth}");
            assert_eq!(y, SEA_LEVEL - depth as i32, "lowest water cell");
            if let Some(s) = e.shore_distance_max {
                assert!(u32::from(map.derived.shore_distance[i]) <= s);
            }
            *kinds.entry(e.name.clone()).or_insert(0) += 1;
        }
    }
    assert!(kinds.len() == 2, "both shellfish appear: {kinds:?}");
}

#[test]
fn nothing_in_channel_spawn_water_or_raft_footprint() {
    let mut checked = 0;
    for seed in SEEDS {
        let (l, levels) = route_layout(seed);
        let channels: Vec<ChannelEval> = l.density.iter().filter_map(ChannelEval::new).collect();
        assert!(!channels.is_empty() && !l.flora_free.is_empty());
        let raft = raft_footprint(&l);
        let (x0, z0, w, d) = AREA;
        for decorate in [false, true] {
            for (x, _, z, _) in placed(&l.generate_layout_region(x0, z0, w, d, decorate).region) {
                assert!(channels.iter().all(|c| c.weight(x, z) == 0.0), "{x},{z}");
                assert!(!raft.contains(&(x, z)));
                for lv in &levels {
                    let [ax, az] = lv.anchorage;
                    let d2 = f64::from((x - ax).pow(2) + (z - az).pow(2));
                    assert!(d2 > lv.spawn_water_radius.powi(2), "{x},{z}");
                }
                assert!(!l.flora_free.iter().any(|e| e.protected(x, 0, z)));
                checked += 1;
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn only_water_cells_become_flora() {
    let ids = shellfish();
    for seed in SEEDS {
        let (l, _) = route_layout(seed);
        let (x0, z0, w, d) = AREA;
        for decorate in [false, true] {
            let before = l.generate_region(x0, z0, w, d, decorate);
            let after = l.generate_layout_region(x0, z0, w, d, decorate).region;
            let mut changed = 0;
            for (i, (&a, &b)) in before.blocks.iter().zip(&after.blocks).enumerate() {
                if a != b {
                    assert_eq!(a, terrain::Block::Water as u8, "cell {i}");
                    assert!(ids.contains_key(&b), "cell {i}");
                    changed += 1;
                }
            }
            assert_eq!(changed, placed(&after).len());
        }
    }
}

#[test]
fn overlapping_regions_agree() {
    for seed in SEEDS {
        let (l, _) = route_layout(seed);
        for decorate in [false, true] {
            let big = l
                .generate_layout_region(-64, -64, 128, 128, decorate)
                .region;
            for (x0, z0, w, d) in [(-64, -64, 40, 70), (-30, -10, 94, 21), (5, -64, 59, 128)] {
                let part = l.generate_layout_region(x0, z0, w, d, decorate).region;
                for z in 0..d {
                    let (bx, bz) = ((x0 + 64) as u32, (z0 + 64) as u32 + z);
                    let a = big.index(bx, 0, bz);
                    let b = part.index(0, 0, z);
                    let n = w as usize * HEIGHT;
                    assert_eq!(
                        big.blocks[a..a + n],
                        part.blocks[b..b + n],
                        "{x0},{z0} row {z}"
                    );
                }
            }
        }
    }
}

#[test]
fn region_order_does_not_change_flora() {
    let (l, _) = route_layout(12345);
    for decorate in [false, true] {
        let tiles: Vec<(i32, i32)> = (0..3)
            .flat_map(|j| (0..3).map(move |i| (-48 + 32 * i, -48 + 32 * j)))
            .collect();
        let rows: Vec<Region> = tiles
            .iter()
            .map(|&(x, z)| l.generate_layout_region(x, z, 32, 32, decorate).region)
            .collect();
        for &k in &[4usize, 8, 0, 6, 2, 7, 1, 5, 3] {
            let (x, z) = tiles[k];
            let r = l.generate_layout_region(x, z, 32, 32, decorate).region;
            assert_eq!(r.blocks, rows[k].blocks, "tile {k}");
        }
    }
}
