//! Wind exposure: a speed factor from how far a point stands above the wider
//! local mean ground height. See docs/design/terrain-wind-shelter.md.
//!
//! Only `+ - * /` and comparisons are used, so native and wasm agree.

/// Tuning for [`exposure_factor`]. The values will come from `data/wind.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureParams {
    /// Factor change per block of height above the mean.
    pub gain: f64,
    /// Lowest factor (hollows).
    pub floor: f64,
    /// Highest factor (hilltops).
    pub ceiling: f64,
    /// Height difference in blocks over which the effect eases in.
    pub ease_len: f64,
}

impl ExposureParams {
    /// Upper bound on `|factor(y + dy) - factor(y)|` for any `y`.
    /// The eased height has slope at most 27/16 (< 2) per block.
    pub fn step_bound(&self, dy: f64) -> f64 {
        2.0 * self.gain.abs() * dy.abs()
    }
}

/// Height difference with the ground-level jump removed: `d` times a
/// smoothstep of `|d| / ease_len`, so it is 0 at the mean and `d` beyond
/// `ease_len`. Monotone in `d`.
fn eased(d: f64, ease_len: f64) -> f64 {
    if ease_len <= 0.0 {
        return d;
    }
    let t = (d.abs() / ease_len).min(1.0);
    d * t * t * (3.0 - 2.0 * t)
}

/// Speed factor for a point `above_mean` blocks above `H_wide(x, z)`
/// (negative below it). 1 at zero, above 1 for positive, below 1 for
/// negative, clamped to `[floor, ceiling]`.
pub fn exposure_factor(above_mean: f64, p: &ExposureParams) -> f64 {
    let f = 1.0 + p.gain * eased(above_mean, p.ease_len);
    f.max(p.floor).min(p.ceiling)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: ExposureParams = ExposureParams {
        gain: 0.02,
        floor: 0.6,
        ceiling: 1.5,
        ease_len: 4.0,
    };

    fn heights() -> impl Iterator<Item = f64> {
        (-400..=400).map(|i| i as f64 * 0.25)
    }

    #[test]
    fn neutral_at_mean_and_signed_around_it() {
        assert_eq!(exposure_factor(0.0, &P), 1.0);
        assert!(exposure_factor(2.0, &P) > 1.0);
        assert!(exposure_factor(-2.0, &P) < 1.0);
    }

    #[test]
    fn monotone_in_height() {
        let mut prev = f64::MIN;
        for h in heights() {
            let f = exposure_factor(h, &P);
            assert!(f >= prev, "decreased at {h}");
            prev = f;
        }
    }

    #[test]
    fn clamped_to_floor_and_ceiling() {
        for h in heights() {
            let f = exposure_factor(h, &P);
            assert!((P.floor..=P.ceiling).contains(&f));
        }
        assert_eq!(exposure_factor(1e6, &P), P.ceiling);
        assert_eq!(exposure_factor(-1e6, &P), P.floor);
    }

    #[test]
    fn small_steps_give_bounded_steps() {
        let dy = 0.05;
        let bound = P.step_bound(dy);
        for h in heights() {
            let step = (exposure_factor(h + dy, &P) - exposure_factor(h, &P)).abs();
            assert!(step <= bound + 1e-12, "step {step} > {bound} at {h}");
        }
    }

    #[test]
    fn eases_in_with_no_jump_at_ground() {
        // Just above the mean the factor is much closer to 1 than the
        // un-eased linear response.
        let f = exposure_factor(0.5, &P);
        assert!(f - 1.0 < P.gain * 0.5 * 0.2);
        // Past the ease length the response is linear in the gain.
        let f = exposure_factor(10.0, &P);
        assert!((f - (1.0 + P.gain * 10.0)).abs() < 1e-12);
    }
}
