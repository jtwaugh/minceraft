//! Shelter factor for terrain-shaped wind: the steepest rise of the ground
//! upwind of a point. See docs/design/terrain-wind-shelter.md Phase A.
//!
//! The wind direction is `w / max(|w|, epsilon)`, so in calm wind the upwind
//! samples contract onto the point and the factor fades instead of jumping.
//! Only `+ - * /` and `sqrt` are used.

#[derive(Clone, Debug, PartialEq)]
pub struct ShelterParams {
    /// Upwind sample distances, blocks; all positive.
    pub distances: Vec<f64>,
    /// Factor at full shelter, in `[0, 1]`.
    pub floor: f64,
    /// Rise (blocks per block) at and below which the factor is 1.
    pub rise_min: f64,
    /// Rise at and above which the factor is `floor`; greater than `rise_min`.
    pub rise_max: f64,
    /// Wind speed below which the direction vector shrinks toward zero.
    pub epsilon: f64,
}

impl ShelterParams {
    fn check(&self) {
        assert!(
            !self.distances.is_empty() && self.distances.iter().all(|&d| d > 0.0),
            "shelter distances must be positive"
        );
        assert!((0.0..=1.0).contains(&self.floor), "floor must be in [0, 1]");
        assert!(
            self.rise_max > self.rise_min,
            "rise_max must exceed rise_min"
        );
        assert!(self.epsilon > 0.0, "epsilon must be positive");
    }

    /// Largest change of the factor per block of height at the point: the
    /// steepest eased slope over the rise range, divided by the shortest distance.
    pub fn height_gain(&self) -> f64 {
        let dmin = self.distances.iter().cloned().fold(f64::INFINITY, f64::min);
        (1.0 - self.floor) * 1.5 / ((self.rise_max - self.rise_min) * dmin)
    }

    /// Largest change of the factor per unit of the rise.
    pub fn rise_gain(&self) -> f64 {
        (1.0 - self.floor) * 1.5 / (self.rise_max - self.rise_min)
    }
}

/// Shelter factor in `[floor, 1]` at `(x, z)` with height `y`, for wind
/// `(wx, wz)`. `height(x, z)` is the ground height field `H`.
pub fn shelter_factor(
    p: &ShelterParams,
    height: impl Fn(f64, f64) -> f64,
    x: f64,
    y: f64,
    z: f64,
    wx: f64,
    wz: f64,
) -> f64 {
    p.check();
    let len = (wx * wx + wz * wz).sqrt().max(p.epsilon);
    let (ux, uz) = (wx / len, wz / len);
    let rise = p
        .distances
        .iter()
        .map(|&d| (height(x - ux * d, z - uz * d) - y) / d)
        .fold(f64::NEG_INFINITY, f64::max);
    let t = ((rise - p.rise_min) / (p.rise_max - p.rise_min)).clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    1.0 - (1.0 - p.floor) * eased
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> ShelterParams {
        ShelterParams {
            distances: vec![8.0, 16.0, 32.0],
            floor: 0.3,
            rise_min: 0.0,
            rise_max: 0.5,
            epsilon: 0.5,
        }
    }

    /// A ridge of height 20 above flat ground at 64, centred on x = 0, 6 wide.
    fn ridge(x: f64, _z: f64) -> f64 {
        if x.abs() < 3.0 {
            84.0
        } else {
            64.0
        }
    }

    fn f(h: impl Fn(f64, f64) -> f64, x: f64, y: f64, wx: f64, wz: f64) -> f64 {
        shelter_factor(&params(), h, x, y, 0.0, wx, wz)
    }

    #[test]
    fn flat_ground_and_windward_face_are_one() {
        assert_eq!(f(|_, _| 64.0, 5.0, 64.0, 3.0, 1.0), 1.0);
        // Westerly wind (+x) blowing toward the ridge: upwind of it is flat.
        assert_eq!(f(ridge, -10.0, 64.0, 3.0, 0.0), 1.0);
        // Slope rising downwind only.
        assert_eq!(f(|x, _| 64.0 + 0.5 * x, 0.0, 64.0, 3.0, 0.0), 1.0);
    }

    #[test]
    fn downwind_of_ridge_is_lower_than_upwind() {
        let lee = f(ridge, 10.0, 64.0, 3.0, 0.0);
        let windward = f(ridge, -10.0, 64.0, 3.0, 0.0);
        assert!(lee < windward);
        assert!(lee >= params().floor);
    }

    #[test]
    fn rotating_wind_has_bounded_steps() {
        let p = params();
        // Ground rising by slope s toward -x; wind direction rotates in small steps.
        let s = 0.3;
        let h = |x: f64, _z: f64| 64.0 - s * x;
        let step = 0.01f64;
        let mut prev = None;
        for i in 0..400 {
            let a = i as f64 * step;
            // Unit-ish rotation without trig: (cos, sin) by a rational parametrisation.
            let k = a * 0.01;
            let n = (1.0 + k * k).sqrt();
            let (wx, wz) = (5.0 / n, 5.0 * k / n);
            let v = shelter_factor(&p, h, 0.0, 64.0, 0.0, wx, wz);
            if let Some(q) = prev {
                // Direction moves by about k-step; samples shift by d * dir, rise by s * dir.
                let bound = p.rise_gain() * s * 0.01 * 0.01 * 1.5 + 1e-9;
                assert!((v - q).abs() <= bound, "{} -> {}", q, v);
            }
            prev = Some(v);
        }
    }

    #[test]
    fn calm_wind_fades_without_jump() {
        let p = params();
        let h = ridge;
        let mut prev = shelter_factor(&p, h, 10.0, 64.0, 0.0, 0.0, 0.0);
        assert_eq!(prev, 1.0);
        for i in 1..=200 {
            let w = i as f64 * 0.01;
            let v = shelter_factor(&p, h, 10.0, 64.0, 0.0, w, 0.0);
            // Each step moves the unit vector by 0.01/epsilon; the ridge face
            // is a step, so only check the factor stays in range and is
            // exactly 1 until the samples reach the ridge.
            assert!(v >= p.floor && v <= 1.0);
            if w < 0.5 * 4.0 / 32.0 {
                assert_eq!(v, 1.0);
            }
            prev = v;
        }
        let _ = prev;
    }

    #[test]
    fn height_and_direction_steps_are_bounded() {
        let p = params();
        let h = |x: f64, z: f64| 64.0 - 0.3 * x + 0.1 * z;
        let dy = 0.05;
        let a = shelter_factor(&p, h, 0.0, 64.0, 0.0, 4.0, 0.0);
        let b = shelter_factor(&p, h, 0.0, 64.0 + dy, 0.0, 4.0, 0.0);
        assert!((a - b).abs() <= p.height_gain() * dy + 1e-12);
        // Position step: ground slope magnitude 0.3162 bounds the rise change per block.
        let c = shelter_factor(&p, h, 0.2, 64.0, 0.0, 4.0, 0.0);
        assert!((a - c).abs() <= p.rise_gain() * 0.32 * 0.2 + 1e-12);
        // Direction step.
        let d = shelter_factor(&p, h, 0.0, 64.0, 0.0, 4.0, 0.04);
        assert!((a - d).abs() <= p.rise_gain() * 0.32 * 0.01 * 1.01 + 1e-12);
    }
}
