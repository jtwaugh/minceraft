//! Roughness behaviour on a real fixed seed: shipped table, the `R` field and
//! the speed multiplier composed. See docs/design/biome-roughness-wind.md
//! Phase A. Locations are found by scanning the coarse heightmap.

use crate::wind_drag::{mean_multiplier, DragParams};
use crate::wind_roughness::{fill_cell, roughness_at, RoughnessParams};
use crate::wind_roughness_cache::RoughnessCache;
use crate::wind_roughness_config::RoughnessConfig;
use terrain::SEA_LEVEL;

const SEED: u64 = 12345;
/// Scan window: `SCAN` samples a side at `STEP` blocks, from `-ORIGIN`.
const SCAN: usize = 512;
const STEP: i32 = 8;
const ORIGIN: i32 = 2048;

fn config() -> RoughnessConfig {
    RoughnessConfig::default_from_data().unwrap()
}

fn drag(c: &RoughnessConfig) -> DragParams {
    DragParams {
        drag: c.roughness_drag,
        floor: c.roughness_floor,
        blend_height: c.blend_height,
        gust_damping: c.gust_damping,
    }
}

fn scan() -> Vec<u8> {
    terrain::generate_coarse_heightmap(
        SEED,
        -ORIGIN,
        -ORIGIN,
        STEP as u32,
        SCAN as u32,
        SCAN as u32,
    )
}

/// Class of sample `(i, k)` of the scan: its shipped roughness.
fn sample_r(c: &RoughnessConfig, map: &[u8], i: usize, k: usize) -> f64 {
    let s = &map[(k * SCAN + i) * 4..];
    if (s[0] as i32) < SEA_LEVEL {
        c.water
    } else {
        c.biome[s[1] as usize]
    }
}

fn world(i: usize) -> f64 {
    (i as i32 * STEP - ORIGIN) as f64
}

/// Index of a sample whose square of `reach` samples all satisfy `want`,
/// nearest to `near` (or the first if none given).
fn find(
    c: &RoughnessConfig,
    map: &[u8],
    reach: usize,
    near: Option<(usize, usize)>,
    want: impl Fn(f64, bool) -> bool,
) -> Option<(usize, usize)> {
    let ok = |i: usize, k: usize| {
        let s = &map[(k * SCAN + i) * 4..];
        want(sample_r(c, map, i, k), (s[0] as i32) < SEA_LEVEL)
    };
    let mut best: Option<((usize, usize), usize)> = None;
    for k in reach..SCAN - reach {
        for i in reach..SCAN - reach {
            // Only cell centres: the cell's whole window is then within `reach`.
            if (world(i) as i32).rem_euclid(64) != 32 || (world(k) as i32).rem_euclid(64) != 32 {
                continue;
            }
            if !(k - reach..=k + reach).all(|kk| (i - reach..=i + reach).all(|ii| ok(ii, kk))) {
                continue;
            }
            let d = near.map_or(0, |(ni, nk)| i.abs_diff(ni).pow(2) + k.abs_diff(nk).pow(2));
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some(((i, k), d));
                if near.is_none() {
                    return Some((i, k));
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

/// Reach, in samples, from a cell centre of the window one fill reads.
fn reach(p: &RoughnessParams) -> usize {
    ((p.cell_size / 2 + p.radius) / p.step) as usize
}

struct Spots {
    forest: (f64, f64),
    plains: (f64, f64),
    sea: (f64, f64),
}

fn spots(c: &RoughnessConfig, map: &[u8]) -> Spots {
    let reach = reach(&c.params());
    let (forest_r, plains_r) = (c.biome[terrain::Biome::Forest as usize], 0.2);
    let forest =
        find(c, map, reach, None, |v, w| !w && v >= forest_r).expect("no deep forest in the scan");
    let plains = find(c, map, reach, Some(forest), |v, w| !w && v <= plains_r)
        .expect("no plains in the scan");
    let sea = find(c, map, reach, Some(plains), |_, w| w).expect("no open sea in the scan");
    let w = |(i, k): (usize, usize)| (world(i), world(k));
    Spots {
        forest: w(forest),
        plains: w(plains),
        sea: w(sea),
    }
}

#[test]
fn forest_is_calmer_than_plains_and_sea_is_windiest() {
    let c = config();
    let (p, d) = (c.params(), drag(&c));
    let s = spots(&c, &scan());
    let f = |(x, z): (f64, f64)| {
        let r = roughness_at(SEED, &p, x, z);
        (r, mean_multiplier(r, 0.0, &d))
    };
    let (rf, mf) = f(s.forest);
    let (rp, mp) = f(s.plains);
    let (rs, ms) = f(s.sea);
    assert!(rf > rp && rp > rs, "R: {rf} {rp} {rs}");
    assert!(mf < mp && mp < ms, "multiplier: {mf} {mp} {ms}");
}

#[test]
fn r_and_multiplier_ease_across_a_forest_edge() {
    let c = config();
    let (p, d) = (c.params(), drag(&c));
    let map = scan();
    let side = (p.radius / p.step) as usize;
    let rough = c.biome[terrain::Biome::Forest as usize];
    let low = 0.45;
    // A band of rows with forest for `side` samples, then plains for `side` samples.
    let band = 2;
    let edge = (side..SCAN - side)
        .flat_map(|i| (side + band..SCAN - side - band).map(move |k| (i, k)))
        .find(|&(i, k)| {
            (k - band..=k + band).all(|kk| {
                (i - side..i).all(|ii| sample_r(&c, &map, ii, kk) >= rough)
                    && (i..i + side).all(|ii| {
                        let v = sample_r(&c, &map, ii, kk);
                        v <= low && v > c.water
                    })
            })
        })
        .expect("no forest-to-plains edge in the scan");
    let (i, k) = edge;
    let z = world(k);
    let xs: Vec<f64> = (-(side as i32) * 2..=side as i32 * 2)
        .map(|n| world(i) + (n * STEP / 2) as f64)
        .collect();
    let rs: Vec<f64> = xs.iter().map(|&x| roughness_at(SEED, &p, x, z)).collect();
    let ms: Vec<f64> = rs.iter().map(|&r| mean_multiplier(r, 0.0, &d)).collect();
    for series in [&rs, &ms] {
        let total = (series[series.len() - 1] - series[0]).abs();
        assert!(total > 0.1, "edge too small to test: {total}");
        let biggest = series
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        assert!(biggest <= 0.25 * total, "step {biggest} of total {total}");
    }
    assert!(rs[0] > *rs.last().unwrap());
}

#[test]
fn lookups_are_reproducible_and_agree_across_cells() {
    let c = config();
    let p = c.params();
    let map = scan();
    let s = spots(&c, &map);
    let size = p.cell_size as f64;
    for (x, z) in [s.forest, s.plains, s.sea] {
        let (cx, cz) = ((x / size).floor() as i32, (z / size).floor() as i32);
        let cold = fill_cell(SEED, &p, cx, cz);
        assert_eq!(cold, fill_cell(SEED, &p, cx, cz));
        assert_eq!(
            roughness_at(SEED, &p, x, z).to_bits(),
            cold.at(x, z).to_bits()
        );
        // The neighbouring cells meet this one bit for bit on shared edges.
        let east = fill_cell(SEED, &p, cx + 1, cz);
        let south = fill_cell(SEED, &p, cx, cz + 1);
        let (ex, sz) = ((cx + 1) as f64 * size, (cz + 1) as f64 * size);
        for t in [0.0, 0.3, 1.0] {
            let (zz, xx) = (cz as f64 * size + t * size, cx as f64 * size + t * size);
            assert_eq!(cold.at(ex, zz).to_bits(), east.at(ex, zz).to_bits());
            assert_eq!(cold.at(xx, sz).to_bits(), south.at(xx, sz).to_bits());
        }
    }
    let line = |_: ()| -> Vec<u64> {
        (0..64)
            .map(|n| roughness_at(SEED, &p, s.forest.0 + n as f64 * 5.5, s.forest.1).to_bits())
            .collect()
    };
    assert_eq!(line(()), line(()));
    assert_eq!(scan(), map);
}

#[test]
fn querying_roughness_leaves_world_output_alone() {
    let c = config();
    let p = c.params();
    let world = || {
        let climate = terrain::generate_climate(SEED, -100, 40, 96, 80, &[], &[]);
        (
            climate.temperature,
            climate.humidity,
            climate.biome,
            terrain::generate_heightmap(SEED, -100, 40, 96, 80),
            terrain::generate_coarse_heightmap(SEED, -100, 40, 8, 40, 40),
        )
    };
    let before = world();
    for n in 0..20 {
        roughness_at(SEED, &p, n as f64 * 37.0 - 300.0, n as f64 * 11.0);
    }
    assert!(before == world());
}

#[test]
fn cached_value_matches_uncached_at_forest_plains_and_sea() {
    let c = config();
    let p = c.params();
    let s = spots(&c, &scan());
    for cap in [1, 2, 64] {
        let mut cache = RoughnessCache::new(SEED, p.clone(), cap);
        for _ in 0..2 {
            for (x, z) in [s.forest, s.plains, s.sea] {
                for (dx, dz) in [(0.0, 0.0), (5.0, -3.0), (-20.0, 17.0)] {
                    let (x, z) = (x + dx, z + dz);
                    assert_eq!(
                        cache.lookup(x, z),
                        roughness_at(SEED, &p, x, z),
                        "cap {cap}"
                    );
                }
            }
        }
    }
}
