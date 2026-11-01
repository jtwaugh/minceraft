//! Channelling: in low ground the wind turns toward the contour of `H`.
//! See docs/design/terrain-wind-shelter.md Phase A (Want: channelling).
//!
//! The wind is blended toward its projection onto the contour line,
//! `c * (c . w)`, with `c` the unit contour direction (`grad H` rotated 90
//! degrees). A projection has no sign choice, so it is continuous when the
//! wind is perpendicular to the contour, and the wind keeps whichever way
//! along the valley it already pointed. The blend weight grows with the depth
//! below the local mean and with `|grad H|`, and is zero on flat ground.
//! Only `+ - * /`, `floor` and `sqrt` are used.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelParams {
    /// Largest blend weight, 0 to 1.
    pub strength: f64,
    /// Depth below `H_wide`, blocks, at which the depth ramp reaches 1.
    pub depth_ramp: f64,
    /// `|grad H|` at which the slope ramp reaches 1.
    pub grad_ramp: f64,
    /// Half-width of the finite difference for `grad H`, blocks.
    pub delta: f64,
}

/// Smoothstep of `v / ramp`, clamped to 0 to 1.
fn ease(v: f64, ramp: f64) -> f64 {
    let t = (v / ramp).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Turn wind `(wx, wz)` at `(x, z)` along the valley contour. `height`
/// returns `(H, H_wide)` at a world position.
pub fn channel(
    p: &ChannelParams,
    height: impl Fn(f64, f64) -> (f64, f64),
    x: f64,
    z: f64,
    wx: f64,
    wz: f64,
) -> (f64, f64) {
    let d = p.delta;
    let gx = (height(x + d, z).0 - height(x - d, z).0) / (2.0 * d);
    let gz = (height(x, z + d).0 - height(x, z - d).0) / (2.0 * d);
    let (h, wide) = height(x, z);
    let g2 = gx * gx + gz * gz;
    let weight = p.strength * ease(wide - h, p.depth_ramp) * ease(g2.sqrt(), p.grad_ramp);
    if weight <= 0.0 || g2 <= 0.0 {
        return (wx, wz);
    }
    // Contour direction c = (-gz, gx) / |g|; c * (c . w) without normalising c.
    let dot = (-gz * wx + gx * wz) / g2;
    let (px, pz) = (-gz * dot, gx * dot);
    (wx + (px - wx) * weight, wz + (pz - wz) * weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: ChannelParams = ChannelParams {
        strength: 0.8,
        depth_ramp: 6.0,
        grad_ramp: 0.2,
        delta: 2.0,
    };

    /// North-south valley (floor at x = 0, walls rising 0.5 per block east
    /// and west) under a local mean of 70.
    fn valley(x: f64, _z: f64) -> (f64, f64) {
        (60.0 + 0.5 * x.abs(), 70.0)
    }

    fn norm(a: (f64, f64)) -> f64 {
        (a.0 * a.0 + a.1 * a.1).sqrt()
    }

    #[test]
    fn westerly_turns_along_valley_keeping_its_direction() {
        for side in [-5.0, 5.0] {
            let south = channel(&P, valley, side, 0.0, 5.0, 1.0);
            assert!(
                south.1 / south.0 > 1.0 / 5.0 * 2.0 && south.1 > 0.0,
                "{south:?}"
            );
            let north = channel(&P, valley, side, 0.0, 5.0, -1.0);
            assert!(
                north.1 / north.0 < -1.0 / 5.0 * 2.0 && north.1 < 0.0,
                "{north:?}"
            );
        }
    }

    #[test]
    fn full_strength_projects_onto_the_contour() {
        let q = ChannelParams { strength: 1.0, ..P };
        let out = channel(&q, valley, 5.0, 0.0, 3.0, 4.0);
        assert!(
            out.0.abs() < 1e-12 && (out.1 - 4.0).abs() < 1e-12,
            "{out:?}"
        );
    }

    #[test]
    fn no_turn_on_flat_or_high_ground() {
        let flat = |_x: f64, _z: f64| (60.0, 70.0);
        assert_eq!(channel(&P, flat, 5.0, 0.0, 5.0, 1.0), (5.0, 1.0));
        let high = |x: f64, _z: f64| (80.0 + 0.5 * x.abs(), 70.0);
        assert_eq!(channel(&P, high, 5.0, 0.0, 5.0, 1.0), (5.0, 1.0));
        // Exactly at the mean: depth zero.
        let mean = |x: f64, _z: f64| (70.0 + 0.5 * x.abs(), 70.0);
        assert_eq!(channel(&P, mean, 5.0, 0.0, 5.0, 1.0), (5.0, 1.0));
    }

    #[test]
    fn continuous_in_position_and_wind() {
        // Smooth bowl so the finite difference changes slowly.
        let bowl = |x: f64, z: f64| (60.0 + 0.02 * x * x + 0.01 * z * z, 70.0);
        let e = 1e-4;
        let w = (5.0, 1.0);
        // The blend is a convex mix of w and a projection of w, so a wind
        // step moves the output by at most that step plus the weight change.
        // The weight slope is at most strength * 1.5 / ramp per ramp input;
        // depth slope <= 0.04 * 8 + 0.02 * 8 and |g| slope is the bowl curvature.
        let wslope = P.strength * 1.5 * (0.48 / P.depth_ramp + 0.04 / P.grad_ramp);
        let mut x = 1.0;
        while x < 8.0 {
            let a = channel(&P, bowl, x, 3.0, w.0, w.1);
            let b = channel(&P, bowl, x + e, 3.0, w.0, w.1);
            let c = channel(&P, bowl, x, 3.0, w.0 + e, w.1 - e);
            let dw = norm((w.0, w.1)) * 2.0;
            // The projected direction also turns; allow 2 / |g| of it, |g| >= 0.04.
            let dir = P.strength * dw;
            assert!(
                norm((a.0 - b.0, a.1 - b.1)) <= (wslope * dw + dir) * e,
                "x {x}"
            );
            assert!(
                norm((a.0 - c.0, a.1 - c.1)) <= 2.0_f64.sqrt() * e * 1.0001,
                "x {x}"
            );
            x += 0.37;
        }
    }
}
