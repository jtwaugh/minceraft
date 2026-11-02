//! The wind at a point: the front wind with its speed held between the
//! floor and ceiling. See docs/design/wind.md Phase A.
//!
//! Gusts are added later behind the same `at` signature.

use crate::wind::WindNoise;
use crate::wind_config::WindConfig;
use crate::wind_front::front_wind;

pub struct Wind {
    noise: WindNoise,
    cfg: WindConfig,
}

impl Wind {
    pub fn new(seed: u64, cfg: WindConfig) -> Wind {
        Wind {
            noise: WindNoise::new(seed),
            cfg,
        }
    }

    /// Wind `(dx, dz)` in blocks per second at `(x, z)`. `ticks` is
    /// `tick + alpha` from the client, or whole ticks from the sim.
    ///
    /// The speed is clamped to `[speed_floor, speed_ceiling]`, keeping the
    /// direction. A zero-length front wind has no direction to scale, so it
    /// stays the zero vector even when the floor is above zero.
    pub fn at(&self, x: f64, z: f64, ticks: f64) -> (f64, f64) {
        let (wx, wz) = front_wind(&self.noise, &self.cfg, x, z, ticks);
        let speed = (wx * wx + wz * wz).sqrt();
        if speed == 0.0 {
            return (0.0, 0.0);
        }
        let k = speed.clamp(self.cfg.speed_floor, self.cfg.speed_ceiling) / speed;
        (wx * k, wz * k)
    }

    /// `clamp(|wind| / speed_ceiling, 0, 1)`.
    pub fn strength(&self, dx: f64, dz: f64) -> f64 {
        ((dx * dx + dz * dz).sqrt() / self.cfg.speed_ceiling).clamp(0.0, 1.0)
    }
}

/// Unit direction of a wind vector, or zero for zero wind.
pub fn direction(dx: f64, dz: f64) -> (f64, f64) {
    let len = (dx * dx + dz * dz).sqrt();
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        (dx / len, dz / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WindConfig {
        WindConfig::default_from_data().unwrap()
    }

    fn grid() -> Vec<(f64, f64, f64)> {
        let mut v = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                for t in [0.0, 4000.0, 30000.0] {
                    v.push((i as f64 * 211.0 - 4000.0, j as f64 * 173.0 - 3000.0, t));
                }
            }
        }
        v
    }

    #[test]
    fn speeds_stay_in_range_with_calm_and_storm() {
        let w = Wind::new(12345, cfg());
        let c = cfg();
        let (mut calm, mut storm) = (false, false);
        for (x, z, t) in grid() {
            let (dx, dz) = w.at(x, z, t);
            let s = (dx * dx + dz * dz).sqrt();
            assert!(s >= c.speed_floor - 1e-9 && s <= c.speed_ceiling + 1e-9);
            calm |= s <= c.speed_floor + 0.1 * c.speed_ceiling;
            storm |= s >= c.speed_ceiling - 1e-9;
        }
        assert!(calm && storm, "calm {calm} storm {storm}");
    }

    #[test]
    fn replay_and_query_order_agree() {
        let (a, b) = (Wind::new(9, cfg()), Wind::new(9, cfg()));
        let pts = grid();
        let fwd: Vec<_> = pts.iter().map(|&(x, z, t)| a.at(x, z, t)).collect();
        let mut rev: Vec<_> = pts.iter().rev().map(|&(x, z, t)| b.at(x, z, t)).collect();
        rev.reverse();
        assert_eq!(fwd, rev);
    }

    #[test]
    fn floor_raises_calm_and_ceiling_keeps_direction() {
        let raw = Wind::new(3, cfg());
        let mut c = cfg();
        c.speed_floor = 2.0;
        let floored = Wind::new(3, c);
        let mut c = cfg();
        c.speed_ceiling = 0.5;
        let capped = Wind::new(3, c);
        for (x, z, t) in grid().into_iter().step_by(7) {
            let (rx, rz) = front_wind(&raw.noise, &raw.cfg, x, z, t);
            let rs = (rx * rx + rz * rz).sqrt();
            let (fx, fz) = floored.at(x, z, t);
            let fs = (fx * fx + fz * fz).sqrt();
            let (cx, cz) = capped.at(x, z, t);
            let cs = (cx * cx + cz * cz).sqrt();
            if rs > 0.0 {
                assert!(fs >= 2.0 - 1e-9);
                assert!(cs <= 0.5 + 1e-9);
                let (d0, d1, d2) = (direction(rx, rz), direction(fx, fz), direction(cx, cz));
                assert!((d0.0 - d1.0).abs() < 1e-9 && (d0.1 - d1.1).abs() < 1e-9);
                assert!((d0.0 - d2.0).abs() < 1e-9 && (d0.1 - d2.1).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn strength_and_direction_are_bounded() {
        let w = Wind::new(1, cfg());
        for (x, z, t) in grid().into_iter().step_by(11) {
            let (dx, dz) = w.at(x, z, t);
            assert!((0.0..=1.0).contains(&w.strength(dx, dz)));
            let (ux, uz) = direction(dx, dz);
            assert!(((ux * ux + uz * uz).sqrt() - 1.0).abs() < 1e-9 || (ux, uz) == (0.0, 0.0));
        }
        assert_eq!(direction(0.0, 0.0), (0.0, 0.0));
        assert_eq!(w.strength(100.0, 0.0), 1.0);
        assert_eq!(w.strength(0.0, 0.0), 0.0);
    }
}
