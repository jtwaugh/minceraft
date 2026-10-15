use super::*;
use terrain::{biome_at, biome_for, generate_region_with_biases, Biases, Climate, HEIGHT};

const RADIAL: ClimateBias = ClimateBias::Radial {
    center: (0.0, 0.0),
    r_inner: 10.0,
    r_outer: 30.0,
    d_temp: 0.5,
    d_humid: -0.25,
};

/// Length 40: (0,0) -> (10,0) -> (10,30).
fn path() -> ClimateBias {
    ClimateBias::AlongPath {
        points: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 30.0)],
        d_temp_start: 0.0,
        d_temp_end: 0.4,
        d_humid_start: 0.2,
        d_humid_end: -0.2,
        half_width: 4.0,
        falloff: 8.0,
    }
}

fn close(a: (f64, f64), b: (f64, f64)) {
    assert!(
        (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12,
        "{a:?} != {b:?}"
    );
}

#[test]
fn radial_inside_edge_outside() {
    close(RADIAL.deltas(0, 0), (0.5, -0.25));
    close(RADIAL.deltas(10, 0), (0.5, -0.25)); // inner edge
    let f = falloff(20.0, 10.0, 30.0);
    close(RADIAL.deltas(20, 0), (0.5 * f, -0.25 * f));
    close(RADIAL.deltas(30, 0), (0.0, 0.0)); // outer edge
    close(RADIAL.deltas(100, 100), (0.0, 0.0));
}

#[test]
fn along_path_start_end_and_off_path() {
    let p = path();
    close(p.deltas(0, 0), (0.0, 0.2)); // start, s = 0
    close(p.deltas(10, 30), (0.4, -0.2)); // end, s = 1
    close(p.deltas(10, 10), (0.4 * 0.5, 0.2 - 0.4 * 0.5)); // arc 20 of 40
                                                           // Off the path: nearest point (5, 0), s = 5/40, d = 4 (inner edge).
    let s = 5.0 / 40.0;
    close(p.deltas(5, 4), (0.4 * s, 0.2 - 0.4 * s));
    // Mid-falloff: d = 8 -> falloff(8, 4, 12) = 0.5.
    close(p.deltas(5, -8), (0.4 * s * 0.5, (0.2 - 0.4 * s) * 0.5));
    close(p.deltas(5, -12), (0.0, 0.0)); // at half_width + falloff
    close(p.deltas(500, 500), (0.0, 0.0));
}

#[test]
fn along_path_degenerate() {
    let mk = |points| ClimateBias::AlongPath {
        points,
        d_temp_start: 0.1,
        d_temp_end: 0.3,
        d_humid_start: 0.0,
        d_humid_end: 0.0,
        half_width: 2.0,
        falloff: 2.0,
    };
    close(mk(vec![]).deltas(0, 0), (0.0, 0.0));
    close(mk(vec![(0.0, 0.0)]).deltas(0, 0), (0.1, 0.0));
}

#[test]
fn adapters_sum_terms() {
    let terms = [RADIAL, path()];
    let (t, h) = (TemperatureTerms(&terms), HumidityTerms(&terms));
    assert!((t.bias(0, 0) - 0.5).abs() < 1e-12);
    assert!((h.bias(0, 0) - (-0.25 + 0.2)).abs() < 1e-12);
}

#[test]
fn radial_temp_changes_biome_at_centre_not_outside() {
    let terms = [ClimateBias::Radial {
        center: (0.0, 0.0),
        r_inner: 16.0,
        r_outer: 48.0,
        d_temp: 1.0,
        d_humid: -1.0,
    }];
    let (t, h) = (TemperatureTerms(&terms), HumidityTerms(&terms));
    let mut changed = 0;
    for seed in 1..=50u64 {
        let c = Climate::new(seed);
        let biased = |x, z| {
            let (bt, bh) = c.at(x, z, &[&t], &[&h]);
            biome_for(bt, bh)
        };
        if biased(0, 0) != biome_at(seed, 0, 0) {
            changed += 1;
        }
        for (x, z) in [(48, 0), (0, -60), (100, 100), (-200, 5)] {
            assert_eq!(biased(x, z), biome_at(seed, x, z), "seed {seed} ({x},{z})");
        }
    }
    assert!(changed > 0);
}

#[test]
fn overlapping_regions_agree_with_both_terms() {
    let terms = [RADIAL, path()];
    let (t, h) = (TemperatureTerms(&terms), HumidityTerms(&terms));
    let (ts, hs): ([&dyn TemperatureBias; 1], [&dyn HumidityBias; 1]) = ([&t], [&h]);
    let b = Biases {
        temperature: &ts,
        humidity: &hs,
        ..Biases::default()
    };
    for decorate in [false, true] {
        let big = generate_region_with_biases(4242, -50, -50, 100, 100, &b, decorate);
        let small = generate_region_with_biases(4242, -13, -7, 40, 50, &b, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let i = small.index(x, 0, z);
                let j = big.index(
                    (small.x0 + x as i32 + 50) as u32,
                    0,
                    (small.z0 + z as i32 + 50) as u32,
                );
                assert_eq!(
                    small.blocks[i..i + HEIGHT],
                    big.blocks[j..j + HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}
