//! Sea-ice floe band (docs/design/sea-ice-floes.md Phase A).

use super::*;
use crate::data::SEA_ICE_T_ICE;

const SEED: u64 = 12345;
const AREA: (i32, i32, u32, u32) = (-200, 50, 160, 160);

/// Temperature sweeping from -0.6 to +0.4 across x, so the sea crosses the band.
fn ramp(x: i32, _z: i32) -> f64 {
    -0.6 + (x + 200) as f64 * (1.0 / 160.0)
}

fn col(r: &Region, x: u32, z: u32) -> &[u8] {
    let i = r.index(x, 0, z);
    &r.blocks[i..i + HEIGHT]
}

fn run(hard: bool, decorate: bool) -> (Region, Vec<f32>) {
    let (x0, z0, w, d) = AREA;
    let tb: [&dyn TemperatureBias; 1] = [&ramp];
    let b = Biases {
        temperature: &tb,
        hard_ice_edge: hard,
        ..Biases::default()
    };
    let r = generate_region_with_biases(SEED, x0, z0, w, d, &b, decorate);
    let t = generate_climate(SEED, x0, z0, w, d, &tb, &[]).temperature;
    (r, t)
}

#[test]
fn band_places_floes_and_water() {
    let (r, ts) = run(false, false);
    let (mut solid, mut floe, mut gap) = (0, 0, 0);
    for z in 0..r.depth {
        for x in 0..r.width {
            let c = col(&r, x, z);
            if c[62] != WATER {
                continue;
            }
            let t = ts[(z * r.width + x) as usize] as f64;
            let top = c[63];
            if t >= 0.5 + 1e-6 {
                assert_eq!(top, WATER, "({x},{z}) T={t}");
            } else if t < SEA_ICE_T_ICE - 1e-6 {
                assert_eq!(top, ICE, "({x},{z}) T={t}");
                solid += 1;
            } else if t > SEA_ICE_T_ICE + 1e-6 && t < 0.5 - 1e-6 {
                assert!(top == FLOE_ICE || top == WATER, "({x},{z}) T={t}");
                floe += (top == FLOE_ICE) as usize;
                gap += (top == WATER) as usize;
            }
        }
    }
    assert!(
        solid > 100 && floe > 100 && gap > 100,
        "{solid} {floe} {gap}"
    );
}

#[test]
fn band_ice_is_subset_of_hard_edge_ice() {
    let (band, _) = run(false, false);
    let (hard, ts) = run(true, false);
    let mut differing = 0;
    for z in 0..band.depth {
        for x in 0..band.width {
            let (a, h) = (col(&band, x, z), col(&hard, x, z));
            assert!(h[63] != FLOE_ICE);
            if is_sea_ice(a[63]) {
                assert_eq!(h[63], ICE, "({x},{z})");
            }
            if a[63] != h[63] {
                let t = ts[(z * band.width + x) as usize] as f64;
                assert!((SEA_ICE_T_ICE - 1e-6..0.5).contains(&t), "({x},{z}) T={t}");
                differing += 1;
            }
        }
    }
    assert!(differing > 100);
}

#[test]
fn floes_clump() {
    let (r, ts) = run(false, false);
    let in_band = |x: u32, z: u32| {
        let t = ts[(z * r.width + x) as usize] as f64;
        col(&r, x, z)[62] == WATER && (SEA_ICE_T_ICE..0.5).contains(&t)
    };
    let floe = |x: u32, z: u32| col(&r, x, z)[63] == FLOE_ICE;
    let (mut cols, mut floes, mut nb, mut nb_floe) = (0u32, 0u32, 0u32, 0u32);
    for z in 1..r.depth - 1 {
        for x in 1..r.width - 1 {
            if !in_band(x, z) {
                continue;
            }
            cols += 1;
            if !floe(x, z) {
                continue;
            }
            floes += 1;
            for (nx, nz) in [(x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1)] {
                if in_band(nx, nz) {
                    nb += 1;
                    nb_floe += floe(nx, nz) as u32;
                }
            }
        }
    }
    let p = floes as f64 / cols as f64;
    let q = nb_floe as f64 / nb as f64;
    // Independent placement would give q ~ p (the mean floe density).
    assert!(q > p + 0.1, "neighbour floe rate {q} vs density {p}");
}

#[test]
fn overlapping_regions_agree() {
    let (x0, z0, _, _) = AREA;
    let tb: [&dyn TemperatureBias; 1] = [&ramp];
    let b = Biases {
        temperature: &tb,
        ..Biases::default()
    };
    for decorate in [false, true] {
        let a = generate_region_with_biases(SEED, x0, z0, 100, 100, &b, decorate);
        let c = generate_region_with_biases(SEED, x0 + 37, z0 + 21, 100, 100, &b, decorate);
        for z in 0..79 {
            for x in 0..63 {
                assert_eq!(
                    col(&a, x + 37, z + 21),
                    col(&c, x, z),
                    "({x},{z}) {decorate}"
                );
            }
        }
    }
}

#[test]
fn no_snow_on_floes_and_open_water_stays_water() {
    let (x0, z0, w, d) = AREA;
    let tb: [&dyn TemperatureBias; 1] = [&ramp];
    let open = |x: i32, z: i32| (x + z) % 3 == 0;
    let b = Biases {
        temperature: &tb,
        open_water: Some(&open),
        ..Biases::default()
    };
    let r = generate_region_with_biases(SEED, x0, z0, w, d, &b, true);
    let mut floes = 0;
    for z in 0..d {
        for x in 0..w {
            let c = col(&r, x, z);
            if c[63] == FLOE_ICE {
                floes += 1;
                assert_ne!(c[64], Block::SnowLayer as u8, "({x},{z})");
            }
            if open(x0 + x as i32, z0 + z as i32) {
                assert!(!is_sea_ice(c[63]), "({x},{z})");
            }
        }
    }
    assert!(floes > 50);
}
