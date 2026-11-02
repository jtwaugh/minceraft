//! Roughness drag on wind speed, eased out with height.
//! See docs/design/biome-roughness-wind.md Phase A.
//!
//! Pure functions of a plain parameter struct. Roughness changes speed only,
//! never direction, and the mean and gust terms stay separate. Only
//! `+ - * /` and a clamp are used, so results are identical everywhere.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragParams {
    /// Speed lost at full roughness (`R = 1`) at the ground.
    pub drag: f64,
    /// Lowest multiplier roughness alone can produce.
    pub floor: f64,
    /// Height above ground, blocks, at and above which roughness has no effect.
    pub blend_height: f64,
    /// Share of the mean's loss applied to gusts; 0 leaves gusts undamped.
    pub gust_damping: f64,
}

/// Smoothstep from 1 at the ground (`h <= 0`) to 0 at `blend_height` and above.
pub fn height_ease(h: f64, blend_height: f64) -> f64 {
    if blend_height <= 0.0 || h >= blend_height {
        return 0.0;
    }
    let t = (h / blend_height).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// `1 - r * drag * g(h)`, at least `floor`.
pub fn mean_multiplier(r: f64, h: f64, p: &DragParams) -> f64 {
    (1.0 - r * p.drag * height_ease(h, p.blend_height)).max(p.floor)
}

/// `1 - gust_damping * (1 - f)` for the mean multiplier `f`.
pub fn gust_multiplier(r: f64, h: f64, p: &DragParams) -> f64 {
    1.0 - p.gust_damping * (1.0 - mean_multiplier(r, h, p))
}

/// Scale a `(mean, gust)` pair of vectors, each by its own multiplier.
pub fn apply(
    mean: [f64; 2],
    gust: [f64; 2],
    r: f64,
    h: f64,
    p: &DragParams,
) -> ([f64; 2], [f64; 2]) {
    let (fm, fg) = (mean_multiplier(r, h, p), gust_multiplier(r, h, p));
    ([mean[0] * fm, mean[1] * fm], [gust[0] * fg, gust[1] * fg])
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: DragParams = DragParams {
        drag: 0.6,
        floor: 0.3,
        blend_height: 16.0,
        gust_damping: 0.0,
    };

    #[test]
    fn ease_ends_and_monotone() {
        assert_eq!(height_ease(0.0, 16.0), 1.0);
        assert_eq!(height_ease(-5.0, 16.0), 1.0);
        assert_eq!(height_ease(16.0, 16.0), 0.0);
        assert_eq!(height_ease(40.0, 16.0), 0.0);
        let mut prev = 1.0;
        for i in 0..=200 {
            let g = height_ease(i as f64 * 0.1, 16.0);
            assert!(g <= prev);
            prev = g;
        }
    }

    #[test]
    fn ease_slope_zero_at_ends() {
        let e = 1e-4;
        assert!((height_ease(e, 16.0) - 1.0).abs() < 1e-6);
        assert!(height_ease(16.0 - e, 16.0) < 1e-6);
    }

    #[test]
    fn mean_bounds() {
        for r in [0.0, 0.3, 1.0, 5.0] {
            assert_eq!(mean_multiplier(r, 16.0, &P), 1.0);
            assert_eq!(mean_multiplier(r, 100.0, &P), 1.0);
            for h in [0.0, 2.0, 8.0, 15.0] {
                assert!(mean_multiplier(r, h, &P) >= P.floor);
            }
        }
        for h in [0.0, 3.0, 9.0, 20.0] {
            assert_eq!(mean_multiplier(0.0, h, &P), 1.0);
        }
        assert_eq!(mean_multiplier(100.0, 0.0, &P), P.floor);
    }

    #[test]
    fn gust_damping() {
        assert_eq!(gust_multiplier(1.0, 0.0, &P), 1.0);
        let q = DragParams {
            gust_damping: 0.5,
            ..P
        };
        let (f, g) = (mean_multiplier(1.0, 0.0, &q), gust_multiplier(1.0, 0.0, &q));
        assert!(g < 1.0 && g > f);
    }

    #[test]
    fn apply_keeps_direction() {
        let q = DragParams {
            gust_damping: 0.5,
            ..P
        };
        let (m, g) = apply([3.0, -4.0], [1.0, 2.0], 0.8, 2.0, &q);
        assert!((m[0] * -4.0 - m[1] * 3.0).abs() < 1e-12 && m[0] > 0.0);
        assert!((g[0] * 2.0 - g[1] * 1.0).abs() < 1e-12 && g[0] > 0.0);
        assert_eq!((m, g), apply([3.0, -4.0], [1.0, 2.0], 0.8, 2.0, &q));
    }
}
