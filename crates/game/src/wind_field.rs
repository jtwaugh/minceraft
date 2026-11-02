//! The wind at a point: the front wind with gusts and direction wobble, its
//! speed held between the floor and ceiling. See docs/design/wind.md Phase A.

use std::cell::RefCell;

use crate::wind::WindNoise;
use crate::wind_config::WindConfig;
use crate::wind_drag::{self, DragParams};
use crate::wind_front::front_wind;
use crate::wind_gust::gust_at;
use crate::wind_roughness_cache::RoughnessCache;
use crate::wind_roughness_config::RoughnessConfig;

/// Roughness cells kept at once; eviction only costs a refill.
const ROUGHNESS_CELLS: usize = 64;
/// Height above ground the Phase A drag is evaluated at: full effect.
const GROUND: f64 = 0.0;

pub struct Wind {
    noise: WindNoise,
    cfg: WindConfig,
    drag: DragParams,
    /// A memo only; `at` stays `&self` and no value depends on its state.
    roughness: RefCell<RoughnessCache>,
}

impl Wind {
    pub fn new(seed: u64, cfg: WindConfig, rough: &RoughnessConfig) -> Wind {
        Wind {
            noise: WindNoise::new(seed),
            cfg,
            drag: DragParams {
                drag: rough.roughness_drag,
                floor: rough.roughness_floor,
                blend_height: rough.blend_height,
                gust_damping: rough.gust_damping,
            },
            roughness: RefCell::new(RoughnessCache::new(seed, rough.params(), ROUGHNESS_CELLS)),
        }
    }

    /// Surface roughness `R` in `[0, 1]` at `(x, z)`.
    pub fn roughness_at(&self, x: f64, z: f64) -> f64 {
        self.roughness.borrow_mut().lookup(x, z)
    }

    /// Speed multiplier roughness gives the mean wind at the ground.
    pub fn ground_multiplier(&self, x: f64, z: f64) -> f64 {
        wind_drag::mean_multiplier(self.roughness_at(x, z), GROUND, &self.drag)
    }

    /// Wind `(dx, dz)` in blocks per second at `(x, z)`. `ticks` is
    /// `tick + alpha` from the client, or whole ticks from the sim.
    ///
    /// Gusts scale the front speed by `max(0, 1 + gust_strength * gust)` and
    /// push it sideways (the wind rotated 90 degrees) by
    /// `direction_wobble * gust * speed`. Roughness drag scales the front wind
    /// and the gust term, then the speed is clamped to `[speed_floor, speed_ceiling]`, keeping the
    /// direction. A zero-length front wind has no direction to scale, so it
    /// stays the zero vector even when the floor is above zero.
    pub fn at(&self, x: f64, z: f64, ticks: f64) -> (f64, f64) {
        let front = front_wind(&self.noise, &self.cfg, x, z, ticks);
        let gust = gust_at(&self.noise, &self.cfg, front, x, z, ticks);
        let scale = (1.0 + self.cfg.gust_strength * gust).max(0.0) - 1.0;
        let wob = self.cfg.direction_wobble * gust;
        let ([mx, mz], [gx, gz]) = wind_drag::apply(
            [front.0, front.1],
            [
                front.0 * scale - front.1 * wob,
                front.1 * scale + front.0 * wob,
            ],
            self.roughness_at(x, z),
            GROUND,
            &self.drag,
        );
        let (wx, wz) = (mx + gx, mz + gz);
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

    fn rc() -> RoughnessConfig {
        RoughnessConfig::default_from_data().unwrap()
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
        let w = Wind::new(12345, cfg(), &rc());
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
    fn no_gusts_means_front_only_wind() {
        let mut c = cfg();
        c.gust_strength = 0.0;
        c.direction_wobble = 0.0;
        let w = Wind::new(5, c.clone(), &rc());
        for (x, z, t) in grid().into_iter().step_by(13) {
            let f = front_wind(&w.noise, &c, x, z, t);
            let m = w.ground_multiplier(x, z);
            let (dx, dz) = w.at(x, z, t);
            let s = (f.0 * f.0 + f.1 * f.1).sqrt() * m;
            let k = if s > 0.0 {
                s.clamp(c.speed_floor, c.speed_ceiling) / s
            } else {
                0.0
            };
            assert!((dx - f.0 * m * k).abs() < 1e-9 && (dz - f.1 * m * k).abs() < 1e-9);
        }
    }

    #[test]
    fn gusts_vary_by_place_and_drift_with_the_wind() {
        let w = Wind::new(12345, cfg(), &rc());
        let mut c = cfg();
        c.gust_strength = 0.0;
        c.direction_wobble = 0.0;
        let calm = Wind::new(12345, c, &rc());
        let (mut differs, mut drifts) = (0, 0);
        for i in 0..20 {
            let (x, z, t) = (i as f64 * 53.0, 17.0 * i as f64, 100.0);
            let g = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0, p.1 - q.1);
            let a = g(w.at(x, z, t), calm.at(x, z, t));
            let b = g(w.at(x + 40.0, z, t), calm.at(x + 40.0, z, t));
            differs += (a != b) as u32;
            // Advect by the local front wind: the gust offset moves with it.
            let f = front_wind(&w.noise, &w.cfg, x, z, t);
            let secs = 50.0 * crate::clock::TICK_SECONDS;
            let ga = gust_at(&w.noise, &w.cfg, f, x, z, t);
            let gb = gust_at(
                &w.noise,
                &w.cfg,
                f,
                x + f.0 * secs,
                z + f.1 * secs,
                t + 50.0,
            );
            drifts += ((ga - gb).abs() < 1e-9) as u32;
        }
        assert!(differs > 10 && drifts == 20);
    }

    #[test]
    fn replay_and_query_order_agree() {
        let (a, b) = (Wind::new(9, cfg(), &rc()), Wind::new(9, cfg(), &rc()));
        let pts = grid();
        let fwd: Vec<_> = pts.iter().map(|&(x, z, t)| a.at(x, z, t)).collect();
        let mut rev: Vec<_> = pts.iter().rev().map(|&(x, z, t)| b.at(x, z, t)).collect();
        rev.reverse();
        assert_eq!(fwd, rev);
    }

    #[test]
    fn floor_raises_calm_and_ceiling_keeps_direction() {
        let calm = || {
            let mut c = cfg();
            c.gust_strength = 0.0;
            c.direction_wobble = 0.0;
            c
        };
        let raw = Wind::new(3, calm(), &rc());
        let mut c = calm();
        c.speed_floor = 2.0;
        let floored = Wind::new(3, c, &rc());
        let mut c = calm();
        c.speed_ceiling = 0.5;
        let capped = Wind::new(3, c, &rc());
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
        let w = Wind::new(1, cfg(), &rc());
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
