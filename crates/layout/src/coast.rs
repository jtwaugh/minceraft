//! `DensityTerm::Coast` evaluation (design: villages-and-villagers 3.0).

use crate::route::unit;
use crate::{nearest_on_segment, smoothstep, DensityTerm};
use terrain::{BaseHeightBias, SEA_LEVEL};

/// Hash streams for the three varying parameters, clear of the route
/// builder's and the island streams (`stream + 3 * lattice + k`).
const COAST_STREAM: u64 = 1 << 33;

/// One polyline segment with its bounding box grown by the coast's reach.
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: (f64, f64),
    b: (f64, f64),
    bounds: (f64, f64, f64, f64),
    /// Arc length of the polyline at `a`.
    start: f64,
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
    seed: u64,
    near: (f64, f64),
    ramp: (f64, f64),
    height: (f64, f64),
    width: f64,
    fade: f64,
    wobble_length: f64,
}

impl CoastEval {
    /// `None` unless `term` is a `Coast`.
    pub fn new(term: &DensityTerm) -> Option<CoastEval> {
        let DensityTerm::Coast {
            points,
            seed,
            near,
            ramp,
            height,
            width,
            fade,
            wobble_length,
        } = term
        else {
            return None;
        };
        let reach = near.1 + ramp.1 + width + fade;
        let mut start = 0.0;
        // A single point has no heading, so it has no right side.
        let segments = points
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                let s = Segment {
                    a,
                    b,
                    bounds: (
                        a.0.min(b.0) - reach,
                        a.1.min(b.1) - reach,
                        a.0.max(b.0) + reach,
                        a.1.max(b.1) + reach,
                    ),
                    start,
                };
                start += (b.0 - a.0).hypot(b.1 - a.1);
                s
            })
            .collect();
        Some(CoastEval {
            segments,
            seed: *seed,
            near: *near,
            ramp: *ramp,
            height: *height,
            width: *width,
            fade: *fade,
            wobble_length: *wobble_length,
        })
    }

    /// Parameter `k` (0 near, 1 ramp, 2 height) in `[min, max]` at arc
    /// length `s`.
    fn wobble(&self, k: u64, s: f64, (min, max): (f64, f64)) -> f64 {
        let u = s.max(0.0) / self.wobble_length;
        let i = u.floor();
        let at = |i: f64| unit(self.seed, COAST_STREAM + 3 * i as u64 + k);
        let f = smoothstep(0.0, 1.0, u - i);
        let v = at(i) + (at(i + 1.0) - at(i)) * f;
        (min + (max - min) * v).clamp(min, max)
    }

    /// `(near, ramp, height)` at the nearest point of the polyline to
    /// world column (x, z), or `None` when no segment is in reach.
    pub fn params_at(&self, x: i32, z: i32) -> Option<(f64, f64, f64)> {
        let (_, _, arc) = self.nearest(x, z)?;
        Some((
            self.wobble(0, arc, self.near),
            self.wobble(1, arc, self.ramp),
            self.wobble(2, arc, self.height),
        ))
    }

    /// Squared distance, the nearest segment and the arc length of the
    /// nearest point.
    fn nearest(&self, x: i32, z: i32) -> Option<(f64, &Segment, f64)> {
        let p = (x as f64, z as f64);
        let mut best: Option<(f64, &Segment, f64)> = None;
        for s in self.segments.iter().filter(|s| {
            p.0 >= s.bounds.0 && p.0 <= s.bounds.2 && p.1 >= s.bounds.1 && p.1 <= s.bounds.3
        }) {
            let (d2, t) = nearest_on_segment(p, s.a, s.b);
            if best.is_none_or(|(b, _, _)| d2 < b) {
                best = Some((d2, s, s.start + t * (s.b.0 - s.a.0).hypot(s.b.1 - s.a.1)));
            }
        }
        best
    }

    /// The blend weight `w` at world column (x, z).
    pub fn weight(&self, x: i32, z: i32) -> f64 {
        self.shape(x, z).0
    }

    /// The weight and the target height at the column.
    fn shape(&self, x: i32, z: i32) -> (f64, f64) {
        let p = (x as f64, z as f64);
        let Some((d2, s, arc)) = self.nearest(x, z) else {
            return (0.0, 0.0);
        };
        let (dx, dz) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
        // Dot with the right normal (-dz, dx).
        if (p.0 - s.a.0) * -dz + (p.1 - s.a.1) * dx <= 0.0 {
            return (0.0, 0.0);
        }
        let d = d2.sqrt();
        let near = self.wobble(0, arc, self.near);
        let start = near + self.wobble(1, arc, self.ramp);
        let end = start + self.width;
        let w = smoothstep(near, start, d) * (1.0 - smoothstep(end, end + self.fade, d));
        (w, SEA_LEVEL as f64 + self.wobble(2, arc, self.height))
    }
}

impl BaseHeightBias for CoastEval {
    fn bias(&self, x: i32, z: i32, base: f64) -> f64 {
        let (w, target) = self.shape(x, z);
        (target - base) * w
    }
}
