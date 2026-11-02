//! Wind from drifting fronts: the wind runs along contours of a pressure
//! field `P` that drifts across the world. See docs/design/wind.md Phase A.
//!
//! `P` is sampled at `(x - u*t, z - v*t) / front_size`, with `(u, v)` the
//! drift and `t` in seconds. The gradient is taken in those sample units by
//! central differences and rotated 90 degrees by swapping components, so no
//! trig is needed. Only `+ - * /`, `floor` and `sqrt` are used.

use crate::clock::TICK_SECONDS;
use crate::wind::WindNoise;
use crate::wind_config::WindConfig;

/// Half-width of the finite difference, in sample units (a fraction of
/// `front_size`). Small against a feature, large against rounding error.
const GRADIENT_STEP: f64 = 1.0 / 64.0;

/// Raw front wind `(wx, wz)` in blocks per second at `(x, z)`, before any
/// floor or ceiling. `ticks` is `tick + alpha` from the client, or whole
/// ticks from the sim.
pub fn front_wind(noise: &WindNoise, cfg: &WindConfig, x: f64, z: f64, ticks: f64) -> (f64, f64) {
    front_wind_of(|sx, sz| noise.pressure(sx, sz), cfg, x, z, ticks)
}

fn front_wind_of(
    pressure: impl Fn(f64, f64) -> f64,
    cfg: &WindConfig,
    x: f64,
    z: f64,
    ticks: f64,
) -> (f64, f64) {
    let t = ticks * TICK_SECONDS;
    let sx = (x - cfg.front_drift[0] * t) / cfg.front_size;
    let sz = (z - cfg.front_drift[1] * t) / cfg.front_size;
    let h = GRADIENT_STEP;
    let gx = (pressure(sx + h, sz) - pressure(sx - h, sz)) / (2.0 * h);
    let gz = (pressure(sx, sz + h) - pressure(sx, sz - h)) / (2.0 * h);
    (-gz * cfg.speed_scale, gx * cfg.speed_scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WindConfig {
        WindConfig::default_from_data().unwrap()
    }

    #[test]
    fn same_query_same_answer_in_any_order() {
        let (a, b, c) = (WindNoise::new(7), WindNoise::new(7), cfg());
        let pts = [
            (0.5, 0.5, 0.0),
            (900.0, -300.0, 1234.5),
            (-50.0, 20.0, 99.0),
        ];
        let fwd: Vec<_> = pts
            .iter()
            .map(|&(x, z, t)| front_wind(&a, &c, x, z, t))
            .collect();
        let mut rev: Vec<_> = pts
            .iter()
            .rev()
            .map(|&(x, z, t)| front_wind(&b, &c, x, z, t))
            .collect();
        rev.reverse();
        assert_eq!(fwd, rev);
        assert_ne!(front_wind(&a, &c, 900.0, -300.0, 0.0), fwd[1]);
    }

    #[test]
    fn wind_is_perpendicular_to_gradient_and_scales_with_it() {
        let mut c = cfg();
        c.front_drift = [0.0, 0.0];
        let (size, scale) = (c.front_size, c.speed_scale);
        // P = 3 sx + 4 sz: gradient (3, 4), length 5.
        let (wx, wz) = front_wind_of(|sx, sz| 3.0 * sx + 4.0 * sz, &c, 10.0, 20.0, 0.0);
        assert!((wx * 3.0 + wz * 4.0).abs() < 1e-9);
        assert!(((wx * wx + wz * wz).sqrt() - 5.0 * scale).abs() < 1e-9);
        let (wx2, wz2) = front_wind_of(|sx, sz| 6.0 * sx + 8.0 * sz, &c, 10.0, 20.0, 0.0);
        assert!((wx2 - 2.0 * wx).abs() < 1e-9 && (wz2 - 2.0 * wz).abs() < 1e-9);
        // The gradient is in sample units: stretching the field halves it.
        c.front_size = size * 2.0;
        let (wx3, wz3) = front_wind_of(|sx, sz| 3.0 * sx + 4.0 * sz, &c, 10.0, 20.0, 0.0);
        assert_eq!((wx3, wz3), (wx, wz));
    }

    #[test]
    fn fronts_drift_slowly_and_advect() {
        let (n, c) = (WindNoise::new(12345), cfg());
        let p = |x: f64, z: f64, secs: f64| {
            let t = secs * 20.0 * TICK_SECONDS;
            n.pressure(
                (x - c.front_drift[0] * t) / c.front_size,
                (z - c.front_drift[1] * t) / c.front_size,
            )
        };
        let (mut lo, mut hi, mut worst) = (f64::MAX, f64::MIN, 0.0f64);
        for i in 0..40 {
            for j in 0..40 {
                let (x, z) = (i as f64 * 211.0 - 4000.0, j as f64 * 173.0 - 3000.0);
                let v = p(x, z, 0.0);
                lo = lo.min(v);
                hi = hi.max(v);
                worst = worst.max((p(x, z, 10.0) - v).abs());
                // Advection: the field at p + drift*dt, t + dt equals p, t.
                let dt = 37.0;
                let moved = p(x + c.front_drift[0] * dt, z + c.front_drift[1] * dt, dt);
                assert!((moved - v).abs() < 1e-9);
            }
        }
        assert!(worst < 0.1 * (hi - lo), "{worst} vs range {}", hi - lo);
    }

    #[test]
    fn time_moves_the_wind_by_drift() {
        let (n, c) = (WindNoise::new(5), cfg());
        let dt_ticks = 400.0; // 20 s
        let a = front_wind(&n, &c, 100.0, 50.0, 0.0);
        let b = front_wind(
            &n,
            &c,
            100.0 + c.front_drift[0] * 20.0,
            50.0 + c.front_drift[1] * 20.0,
            dt_ticks,
        );
        assert!((a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
    }

    #[test]
    fn climate_unchanged_by_front_queries() {
        use terrain::climate::climate_at;
        let climate = || -> Vec<(f64, f64)> {
            (-3..3)
                .map(|i| climate_at(12345, i * 97, i * -41))
                .collect()
        };
        let before = climate();
        front_wind(&WindNoise::new(12345), &cfg(), 3.0, 4.0, 5.0);
        assert_eq!(before, climate());
    }
}
