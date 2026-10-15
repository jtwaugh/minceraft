use super::*;
use terrain::{
    column_summary, generate_region_with_biases, BaseHeightBias, Biases, Block, HEIGHT, SEA_LEVEL,
};

const ISLAND: DensityTerm = DensityTerm::RadialBaseHeight {
    center: (0.0, 0.0),
    r_inner: 24.0,
    r_outer: 64.0,
    delta: 40.0,
};

#[test]
fn falloff_shape() {
    assert_eq!(falloff(0.0, 10.0, 20.0), 1.0);
    assert_eq!(falloff(10.0, 10.0, 20.0), 1.0);
    assert_eq!(falloff(20.0, 10.0, 20.0), 0.0);
    assert_eq!(falloff(500.0, 10.0, 20.0), 0.0);
    assert!((falloff(15.0, 10.0, 20.0) - 0.5).abs() < 1e-12);
    let mut prev = 1.0;
    for i in 0..=100 {
        let v = falloff(10.0 + i as f64 / 10.0, 10.0, 20.0);
        assert!((0.0..=1.0).contains(&v) && v <= prev);
        prev = v;
    }
}

#[test]
fn radial_returns_delta_times_falloff() {
    assert_eq!(ISLAND.bias(0, 0, 68.0), 40.0);
    assert_eq!(ISLAND.bias(100, 0, 68.0), 0.0);
    assert_eq!(ISLAND.bias(40, 0, 68.0), 40.0 * falloff(40.0, 24.0, 64.0));
}

#[test]
fn centre_is_land_for_95_percent_of_seeds() {
    let terms: [&dyn BaseHeightBias; 1] = [&ISLAND];
    let b = Biases {
        base_height: &terms,
        ..Biases::default()
    };
    let land = (1..=200u64)
        .filter(|&seed| {
            let r = generate_region_with_biases(seed, 0, 0, 1, 1, &b, false);
            let (top, ty, _, _) = column_summary(&r.blocks[r.index(0, 0, 0)..][..HEIGHT]);
            top != Block::Water as u8 && top != Block::Ice as u8 && ty as i32 >= SEA_LEVEL
        })
        .count();
    assert!(land >= 190, "only {land}/200 seeds have land at the centre");
}

#[test]
fn overlapping_regions_agree_with_term() {
    let terms: [&dyn BaseHeightBias; 1] = [&ISLAND];
    let b = Biases {
        base_height: &terms,
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
