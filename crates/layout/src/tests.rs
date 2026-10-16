use super::*;
use terrain::BaseHeightBias;

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
