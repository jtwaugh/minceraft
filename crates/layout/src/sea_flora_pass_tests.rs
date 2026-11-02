use std::collections::BTreeMap;

use terrain::{Region, HEIGHT, SEA_LEVEL};

use crate::{
    build_route, build_route_layout, raft_footprint, ChannelEval, Exclusion, Habitats, Layout,
    Level, SeaFlora, SeaFloraShape, WorldConfig, NO_HABITAT,
};
use terrain::ProtectedMask;

const SEEDS: [u64; 3] = [9, 12345, 31];
/// Area around the first anchorage: spawn water, raft, channel and open sea.
const AREA: (i32, i32, u32, u32) = (-160, -160, 320, 320);

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

/// `(seed, x0, z0)` of `KELP_SIZE` squares with kelp (bottoms are mostly dirt,
/// so kelp is rare near the start).
const KELP_AREAS: [(u64, i32, i32); 3] = [(3, -256, 256), (1, 256, 256), (31, 256, 256)];
const KELP_SIZE: u32 = 256;

/// Kelp block ids to `true` for the top segment.
fn kelp_ids() -> BTreeMap<u8, bool> {
    let k = SeaFlora::shipped().get("kelp").unwrap().blocks.clone();
    BTreeMap::from([(k[0], false), (k[1], true)])
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
    let (mut checked, mut spawn_water) = (0, 0);
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
        // The route's spawn water lies inside channel reach and is deepened
        // to `deep_water`, so nothing grows there anyway. Move a spawn-water
        // zone onto shellfish placed above: only `flora_free` clears it.
        let r = l.generate_layout_region(x0, z0, w, d, false).region;
        let &(fx, _, fz, _) = placed(&r).first().unwrap();
        let mut zoned = l.clone();
        zoned.flora_free = vec![Exclusion::Circle {
            center: (f64::from(fx), f64::from(fz)),
            radius: levels[0].spawn_water_radius,
        }];
        let in_zone = |&(x, _, z, _): &(i32, i32, i32, u8)| zoned.flora_free[0].protected(x, 0, z);
        for decorate in [false, true] {
            let r = l.generate_layout_region(x0, z0, w, d, decorate).region;
            spawn_water += placed(&r).into_iter().filter(in_zone).count();
            let r = zoned.generate_layout_region(x0, z0, w, d, decorate).region;
            assert_eq!(placed(&r).into_iter().filter(in_zone).count(), 0);
        }
    }
    assert!(checked > 0);
    assert!(spawn_water > 0, "shellfish would grow in the moved zone");
}

#[test]
fn only_water_cells_become_flora() {
    let mut ids = shellfish();
    let kelp = kelp_ids();
    ids.extend(kelp.keys().map(|&k| (k, String::new())));
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
            let kelp_cells = after.blocks.iter().filter(|b| kelp.contains_key(b)).count();
            assert_eq!(changed, placed(&after).len() + kelp_cells);
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

/// Kelp columns `(x, z, bottom y, height)` of `r`, checking each is a stack
/// of middle cells under one top cell.
fn kelp_columns(r: &Region) -> Vec<(i32, i32, i32, u32)> {
    let ids = kelp_ids();
    let mut out = Vec::new();
    for (c, col) in r.blocks.chunks_exact(HEIGHT).enumerate() {
        let (x, z) = (
            r.x0 + (c as u32 % r.width) as i32,
            r.z0 + (c as u32 / r.width) as i32,
        );
        let ys: Vec<usize> = (0..HEIGHT).filter(|&y| ids.contains_key(&col[y])).collect();
        let Some(&y0) = ys.first() else { continue };
        let n = ys.len();
        assert_eq!(ys, (y0..y0 + n).collect::<Vec<_>>(), "{x},{z} contiguous");
        assert!(ids[&col[y0 + n - 1]], "{x},{z} top");
        assert!(ys[..n - 1].iter().all(|&y| !ids[&col[y]]), "{x},{z} middle");
        out.push((x, z, y0 as i32, n as u32));
    }
    out
}

#[test]
fn kelp_columns_follow_their_entry() {
    let flora = SeaFlora::shipped();
    let e = flora.get("kelp").unwrap();
    let SeaFloraShape::Column(c) = e.shape else {
        panic!()
    };
    let habitats = Habitats::shipped();
    let (mut total, mut tall) = (0, 0);
    for (seed, x0, z0) in KELP_AREAS {
        let (l, _) = route_layout(seed);
        let (w, d) = (KELP_SIZE, KELP_SIZE);
        let r = l.generate_layout_region(x0, z0, w, d, false).region;
        let map = l.habitat_map(x0, z0, w, d);
        for (x, z, y, h) in kelp_columns(&r) {
            let i = ((z - z0) as u32 * w + (x - x0) as u32) as usize;
            let depth = u32::from(map.derived.water_depth[i]);
            let hab = &habitats.rules[usize::from(map.habitat[i])].id;
            assert!(e.habitats.iter().any(|id| habitats.rules[id.0].id == *hab));
            assert!(e.bottom.contains(&map.derived.bottom_block[i]));
            assert!((e.depth_min..=e.depth_max).contains(&depth));
            assert_eq!(y, SEA_LEVEL - depth as i32, "bottom");
            assert!((c.height_min..=c.height_max).contains(&h), "{h}");
            assert!(depth - h >= c.surface_gap_min, "top rule {depth} {h}");
            total += 1;
            tall += usize::from(h > c.height_min);
        }
    }
    assert!(total > 0, "kelp appears");
    assert!(tall > 0, "heights vary");
}

#[test]
fn kelp_is_the_same_across_a_split_column_border() {
    let mut seen = 0;
    for (seed, x0, z0) in KELP_AREAS {
        let (l, _) = route_layout(seed);
        let (w, d) = (KELP_SIZE, KELP_SIZE);
        for decorate in [false, true] {
            let whole = kelp_columns(&l.generate_layout_region(x0, z0, w, d, decorate).region);
            let half = (w / 2) as i32;
            let left = l.generate_layout_region(x0, z0, w / 2, d, decorate).region;
            let right = l
                .generate_layout_region(x0 + half, z0, w - w / 2, d, decorate)
                .region;
            let mut parts = kelp_columns(&left);
            parts.extend(kelp_columns(&right));
            parts.sort_by_key(|&(x, z, ..)| (z, x));
            seen += whole.len();
            assert_eq!(whole, parts);
        }
    }
    assert!(seen > 0);
}
