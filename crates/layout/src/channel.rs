//! `DensityTerm::Channel` evaluation (design: world-layout 4.1).

use crate::{falloff, nearest_on_segment, DensityTerm};
use terrain::{BaseHeightBias, DensityClamp, SEA_LEVEL};

/// One polyline segment with its bounding box grown by the channel's reach.
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: (f64, f64),
    b: (f64, f64),
    /// `(min_x, min_z, max_x, max_z)`, already grown by `half_width + falloff`.
    bounds: (f64, f64, f64, f64),
}

/// A `Channel` term with a bounding box stored per segment, so a column
/// far from every segment costs one box test per segment.
#[derive(Clone, Debug)]
pub struct ChannelEval {
    segments: Vec<Segment>,
    half_width: f64,
    reach: f64,
    /// `SEA_LEVEL - depth_below_sea`.
    target: f64,
    /// `SEA_LEVEL + clear_above`.
    top: f64,
}

impl ChannelEval {
    /// `None` unless `term` is a `Channel`.
    pub fn new(term: &DensityTerm) -> Option<ChannelEval> {
        let DensityTerm::Channel {
            points,
            half_width,
            falloff: fall,
            depth_below_sea,
            clear_above,
        } = term
        else {
            return None;
        };
        let reach = half_width + fall;
        let seg = |a: (f64, f64), b: (f64, f64)| Segment {
            a,
            b,
            bounds: (
                a.0.min(b.0) - reach,
                a.1.min(b.1) - reach,
                a.0.max(b.0) + reach,
                a.1.max(b.1) + reach,
            ),
        };
        let segments = match points.as_slice() {
            [] => Vec::new(),
            [a] => vec![seg(*a, *a)],
            pts => pts.windows(2).map(|w| seg(w[0], w[1])).collect(),
        };
        Some(ChannelEval {
            segments,
            half_width: *half_width,
            reach,
            target: SEA_LEVEL as f64 - depth_below_sea,
            top: (SEA_LEVEL + clear_above) as f64,
        })
    }

    /// The blend weight `w` at world column (x, z); 0 beyond the reach.
    pub fn weight(&self, x: i32, z: i32) -> f64 {
        let p = (x as f64, z as f64);
        let d2 = self
            .segments
            .iter()
            .filter(|s| {
                p.0 >= s.bounds.0 && p.0 <= s.bounds.2 && p.1 >= s.bounds.1 && p.1 <= s.bounds.3
            })
            .map(|s| nearest_on_segment(p, s.a, s.b).0)
            .fold(f64::INFINITY, f64::min);
        if d2.is_finite() {
            falloff(d2.sqrt(), self.half_width, self.reach)
        } else {
            0.0
        }
    }
}

impl BaseHeightBias for ChannelEval {
    fn bias(&self, x: i32, z: i32, base: f64) -> f64 {
        (self.target - base) * self.weight(x, z)
    }
}

impl DensityClamp for ChannelEval {
    fn clamp(&self, x: i32, y: i32, z: i32, density: f64) -> f64 {
        let yf = y as f64;
        if yf < self.target || yf >= self.top {
            return density;
        }
        let w = self.weight(x, z);
        density.min(density + (-1.0 - density) * w)
    }
}
