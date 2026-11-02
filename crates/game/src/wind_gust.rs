//! Gust layer of the wind: gust noise advected by the local front wind.
//! See docs/design/wind.md Phase A.
//!
//! The gust noise is sampled at `(x - wx*t, z - wz*t) / gust_size`, with
//! `(wx, wz)` the front wind at that position and `t` in seconds. There is no
//! separate change over time, so the gusts are pure advection. Only
//! `+ - * /`, `floor` and `sqrt` are used.

use crate::clock::TICK_SECONDS;
use crate::wind::WindNoise;
use crate::wind_config::WindConfig;

/// Raw gust value at `(x, z)`, given the front wind `(wx, wz)` there in
/// blocks per second. `ticks` is `tick + alpha` from the client, or whole
/// ticks from the sim.
pub fn gust_at(
    noise: &WindNoise,
    cfg: &WindConfig,
    front: (f64, f64),
    x: f64,
    z: f64,
    ticks: f64,
) -> f64 {
    let t = ticks * TICK_SECONDS;
    noise.gust(
        (x - front.0 * t) / cfg.gust_size,
        (z - front.1 * t) / cfg.gust_size,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WindConfig {
        WindConfig::default_from_data().unwrap()
    }

    #[test]
    fn gusts_advect_with_the_wind() {
        let (n, c) = (WindNoise::new(12345), cfg());
        let w = (1.7, -0.9);
        for i in 0..30 {
            for j in 0..30 {
                let (x, z) = (i as f64 * 37.0 - 500.0, j as f64 * 29.0 - 400.0);
                let (t, dt) = (i as f64 * 11.0, 73.0);
                let secs = dt * TICK_SECONDS;
                let a = gust_at(&n, &c, w, x, z, t);
                let b = gust_at(&n, &c, w, x + w.0 * secs, z + w.1 * secs, t + dt);
                assert!((a - b).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn same_query_same_answer_in_any_order() {
        let (a, b, c) = (WindNoise::new(7), WindNoise::new(7), cfg());
        let w = (0.4, 1.1);
        let pts = [
            (0.5, 0.5, 0.0),
            (900.0, -300.0, 1234.5),
            (-50.0, 20.0, 99.0),
        ];
        let fwd: Vec<_> = pts
            .iter()
            .map(|&(x, z, t)| gust_at(&a, &c, w, x, z, t))
            .collect();
        let mut rev: Vec<_> = pts
            .iter()
            .rev()
            .map(|&(x, z, t)| gust_at(&b, &c, w, x, z, t))
            .collect();
        rev.reverse();
        assert_eq!(fwd, rev);
        assert_ne!(fwd[0], fwd[1]);
        assert_ne!(gust_at(&a, &c, w, 900.0, -300.0, 0.0), fwd[1]);
    }

    #[test]
    fn zero_wind_is_static_in_time() {
        let (n, c) = (WindNoise::new(3), cfg());
        let v = gust_at(&n, &c, (0.0, 0.0), 12.5, -80.25, 0.0);
        for t in [1.0, 100.0, 5000.5] {
            assert_eq!(gust_at(&n, &c, (0.0, 0.0), 12.5, -80.25, t), v);
        }
        assert_ne!(v, gust_at(&n, &c, (0.0, 0.0), 112.5, -80.25, 0.0));
    }

    #[test]
    fn climate_unchanged_by_gust_queries() {
        use terrain::climate::climate_at;
        let climate = || -> Vec<(f64, f64)> {
            (-3..3)
                .map(|i| climate_at(12345, i * 97, i * -41))
                .collect()
        };
        let before = climate();
        gust_at(&WindNoise::new(12345), &cfg(), (1.0, 2.0), 3.0, 4.0, 5.0);
        assert_eq!(before, climate());
    }
}
