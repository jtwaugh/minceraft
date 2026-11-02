//! Terrain-shaped wind: channelling, then shelter, then exposure.
//! See docs/design/terrain-wind-shelter.md Phase A (order of the layers;
//! decided in #1800 and #1801).
//!
//! Channelling turns the incoming vector along the valley contour. Shelter
//! then takes its upwind direction from the channelled vector, and exposure
//! comes from `y - H_wide`. The vector is scaled by shelter and exposure; the
//! shelter factor is also returned so the gust layer can scale gust amplitude
//! by it. The caller applies the speed floor and ceiling afterwards.
//! Only `+ - * /`, `floor` and `sqrt` are used.

use std::cell::RefCell;

use crate::wind_channel::channel;
use crate::wind_config::WindConfig;
use crate::wind_exposure::exposure_factor;
use crate::wind_height_cache::WindHeightCache;
use crate::wind_shelter::shelter_factor;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainWind {
    /// Wind `(x, z)` after channelling, shelter and exposure.
    pub wind: (f64, f64),
    /// Shelter factor in `[floor, 1]`.
    pub shelter: f64,
}

/// Wind at `(x, y, z)` for the `incoming` `(x, z)` vector.
///
/// `incoming` is the fronts sum. The land-sea breeze (#1750) is not built;
/// when it lands it must be added into `incoming` before this call, so that
/// shelter sees it too.
pub fn terrain_wind(
    cache: &mut WindHeightCache,
    params: &WindConfig,
    incoming: (f64, f64),
    x: f64,
    y: f64,
    z: f64,
) -> TerrainWind {
    let cache = RefCell::new(cache);
    terrain_wind_with(
        |px, pz| cache.borrow_mut().lookup(px, pz),
        params,
        incoming,
        x,
        y,
        z,
    )
}

/// [`terrain_wind`] over any `(H, H_wide)` source.
pub fn terrain_wind_with(
    height: impl Fn(f64, f64) -> (f64, f64),
    params: &WindConfig,
    incoming: (f64, f64),
    x: f64,
    y: f64,
    z: f64,
) -> TerrainWind {
    let (cx, cz) = channel(&params.channel, &height, x, z, incoming.0, incoming.1);
    let shelter = shelter_factor(&params.shelter, |px, pz| height(px, pz).0, x, y, z, cx, cz);
    let exposure = exposure_factor(y - height(x, z).1, &params.exposure);
    let scale = shelter * exposure;
    TerrainWind {
        wind: (cx * scale, cz * scale),
        shelter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> WindConfig {
        WindConfig::default_from_data().unwrap()
    }

    fn norm(a: (f64, f64)) -> f64 {
        (a.0 * a.0 + a.1 * a.1).sqrt()
    }

    fn flat(_x: f64, _z: f64) -> (f64, f64) {
        (64.0, 64.0)
    }

    /// North-south valley floor at x = 0, 0.5 rise per block, mean 80.
    fn valley(x: f64, _z: f64) -> (f64, f64) {
        (64.0 + 0.5 * x.abs(), 80.0)
    }

    /// A wall at x in [-30, -20] rising 40 blocks above flat ground.
    fn wall(x: f64, _z: f64) -> (f64, f64) {
        let h = if (-30.0..-20.0).contains(&x) {
            104.0
        } else {
            64.0
        };
        (h, 64.0)
    }

    #[test]
    fn flat_ground_at_the_mean_passes_wind_through() {
        let out = terrain_wind_with(flat, &config(), (3.0, 1.0), 0.0, 64.0, 0.0);
        assert_eq!(out.wind, (3.0, 1.0));
        assert_eq!(out.shelter, 1.0);
    }

    #[test]
    fn channelling_turns_the_wind_in_a_valley() {
        let c = config();
        let out = terrain_wind_with(valley, &c, (5.0, 1.0), 5.0, 66.5, 0.0);
        assert!(out.wind.1 / out.wind.0 > 1.0 / 5.0, "{out:?}");
    }

    #[test]
    fn shelter_slows_wind_behind_a_wall() {
        let c = config();
        let lee = terrain_wind_with(wall, &c, (5.0, 0.0), -10.0, 64.0, 0.0);
        let open = terrain_wind_with(wall, &c, (-5.0, 0.0), -10.0, 64.0, 0.0);
        assert!(lee.shelter < 1.0 && open.shelter == 1.0, "{lee:?} {open:?}");
        assert!(norm(lee.wind) < norm(open.wind));
        assert!((lee.wind.0 / 5.0 - lee.shelter).abs() < 1e-12);
    }

    #[test]
    fn exposure_speeds_wind_on_a_hilltop() {
        let c = config();
        let top = terrain_wind_with(flat, &c, (5.0, 0.0), 0.0, 84.0, 0.0);
        let low = terrain_wind_with(flat, &c, (5.0, 0.0), 0.0, 54.0, 0.0);
        assert!(top.wind.0 > 5.0 && low.wind.0 < 5.0);
    }

    #[test]
    fn channelled_direction_feeds_shelter() {
        // A wall to the west of a N-S valley. Raw wind from the north-east
        // looks past the wall; channelled to run along the valley it still
        // does, but a raw westerly with strong channelling is turned away
        // from the wall, so shelter must differ from the unchannelled value.
        let mut c = config();
        c.channel.strength = 1.0;
        let h = |x: f64, z: f64| {
            let (a, b) = valley(x, z);
            (a + wall(x, z).0 - 64.0, b)
        };
        let with = terrain_wind_with(h, &c, (5.0, 0.5), -10.0, 80.0, 0.0);
        c.channel.strength = 0.0;
        let without = terrain_wind_with(h, &c, (5.0, 0.5), -10.0, 80.0, 0.0);
        assert!(with.shelter > without.shelter, "{with:?} {without:?}");
    }

    #[test]
    fn calm_wind_is_finite_and_continuous() {
        let c = config();
        let mut prev = None;
        for i in 0..=40 {
            let w = (i as f64 * 1e-6, 0.0);
            let out = terrain_wind_with(wall, &c, w, -10.0, 64.0, 0.0);
            assert!(out.wind.0.is_finite() && out.wind.1.is_finite());
            if let Some(p) = prev {
                assert!(norm((out.wind.0 - p, 0.0)) < 1e-5);
            }
            prev = Some(out.wind.0);
        }
        let zero = terrain_wind_with(wall, &c, (0.0, 0.0), -10.0, 64.0, 0.0);
        assert_eq!(zero.wind, (0.0, 0.0));
    }

    #[test]
    fn small_steps_give_small_steps() {
        let c = config();
        let bowl = |x: f64, z: f64| (60.0 + 0.02 * x * x + 0.01 * z * z, 70.0);
        let e = 1e-5;
        // Per-layer slopes derived from the data keys: exposure per block of
        // y, shelter per block of height and per unit of rise, times speed.
        let speed = 6.0;
        let ybound = speed * (c.exposure.step_bound(1.0) + c.shelter.height_gain());
        let a = terrain_wind_with(bowl, &c, (5.0, 2.0), 3.0, 75.0, 2.0);
        let b = terrain_wind_with(bowl, &c, (5.0, 2.0), 3.0, 75.0 + e, 2.0);
        assert!(norm((a.wind.0 - b.wind.0, a.wind.1 - b.wind.1)) <= ybound * e * 1.001);
        for (dx, dz, dwx, dwz) in [(e, 0.0, 0.0, 0.0), (0.0, e, 0.0, 0.0), (0.0, 0.0, e, -e)] {
            let b = terrain_wind_with(bowl, &c, (5.0 + dwx, 2.0 + dwz), 3.0 + dx, 75.0, 2.0 + dz);
            let d = norm((a.wind.0 - b.wind.0, a.wind.1 - b.wind.1));
            assert!(d < 1e-3, "{d}");
        }
    }
}
