//! `DensityTerm::Coast` evaluation (design: villages-and-villagers 3.0).

use crate::{nearest_on_segment, smoothstep, DensityTerm};
use terrain::{BaseHeightBias, SEA_LEVEL};

/// One polyline segment with its bounding box grown by the coast's reach.
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: (f64, f64),
    b: (f64, f64),
    bounds: (f64, f64, f64, f64),
}

/// A `Coast` term with a bounding box stored per segment.
///
/// The side is decided by the nearest segment (first wins ties): a column
/// is on the right when the cross product of that segment's heading
/// `(dx, dz)` with the vector to the column puts it on `(-dz, dx)`. At a
/// bend, the columns nearest the shared vertex are on the same side of both
/// segments on the outside of the bend, and the segments' equidistant line
/// lies on one side of both on the inside, so the side never flips away
/// from the polyline and the weight stays continuous. Distances are to the
/// whole polyline, so the inside of a bend stays within `near` of it
/// wherever the other segment is close.
#[derive(Clone, Debug)]
pub struct CoastEval {
    segments: Vec<Segment>,
    near: f64,
    ramp: f64,
    width: f64,
    fade: f64,
    /// `SEA_LEVEL + height`.
    target: f64,
}

impl CoastEval {
    /// `None` unless `term` is a `Coast`.
    pub fn new(term: &DensityTerm) -> Option<CoastEval> {
        let DensityTerm::Coast {
            points,
            near,
            ramp,
            height,
            width,
            fade,
        } = term
        else {
            return None;
        };
        let reach = near + ramp + width + fade;
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
        // A single point has no heading, so it has no right side.
        let segments = points.windows(2).map(|w| seg(w[0], w[1])).collect();
        Some(CoastEval {
            segments,
            near: *near,
            ramp: *ramp,
            width: *width,
            fade: *fade,
            target: (SEA_LEVEL as f64) + height,
        })
    }

    /// The blend weight `w` at world column (x, z).
    pub fn weight(&self, x: i32, z: i32) -> f64 {
        let p = (x as f64, z as f64);
        let mut best: Option<(f64, &Segment)> = None;
        for s in self.segments.iter().filter(|s| {
            p.0 >= s.bounds.0 && p.0 <= s.bounds.2 && p.1 >= s.bounds.1 && p.1 <= s.bounds.3
        }) {
            let d2 = nearest_on_segment(p, s.a, s.b).0;
            if best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, s));
            }
        }
        let Some((d2, s)) = best else { return 0.0 };
        let (dx, dz) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
        // Dot with the right normal (-dz, dx).
        if (p.0 - s.a.0) * -dz + (p.1 - s.a.1) * dx <= 0.0 {
            return 0.0;
        }
        let d = d2.sqrt();
        let start = self.near + self.ramp;
        let end = start + self.width;
        smoothstep(self.near, start, d) * (1.0 - smoothstep(end, end + self.fade, d))
    }
}

impl BaseHeightBias for CoastEval {
    fn bias(&self, x: i32, z: i32, base: f64) -> f64 {
        (self.target - base) * self.weight(x, z)
    }
}
