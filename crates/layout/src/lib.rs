//! World layout: bias terms that steer the terrain generator.
//! See docs/design/world-layout.md (Phase A).

use terrain::{BaseHeightBias, ProtectedMask};

/// Hermite smoothstep from `e0` to `e1`; `e0 > e1` gives a falling edge.
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1.0 inside `r_inner`, 0.0 beyond `r_outer`, smooth in between.
pub fn falloff(d: f64, r_inner: f64, r_outer: f64) -> f64 {
    smoothstep(r_outer, r_inner, d)
}

/// A term that shapes the terrain. Only base-height terms exist so far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DensityTerm {
    /// Raises (or lowers) the base height by up to `delta` blocks around
    /// `center` (world x, z): full effect inside `r_inner`, none beyond
    /// `r_outer`.
    RadialBaseHeight {
        center: (f64, f64),
        r_inner: f64,
        r_outer: f64,
        delta: f64,
    },
}

impl BaseHeightBias for DensityTerm {
    fn bias(&self, x: i32, z: i32) -> f64 {
        match *self {
            DensityTerm::RadialBaseHeight {
                center,
                r_inner,
                r_outer,
                delta,
            } => {
                let d = (x as f64 - center.0).hypot(z as f64 - center.1);
                delta * falloff(d, r_inner, r_outer)
            }
        }
    }
}

/// A zone where decoration (trees, plants, snow) must not modify any cell.
/// Terrain is unaffected. Boundaries are inclusive: a column exactly
/// `radius` (or `half_width`) away is protected. Protection covers every y.
#[derive(Clone, Debug, PartialEq)]
pub enum Exclusion {
    Circle {
        center: (f64, f64),
        radius: f64,
    },
    /// Every column within `half_width` of the polyline through `points`.
    Corridor {
        points: Vec<(f64, f64)>,
        half_width: f64,
    },
}

/// Squared distance from `p` to the segment `a`-`b`.
fn dist2_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dz * dz;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / len2).clamp(0.0, 1.0)
    };
    let (cx, cz) = (a.0 + t * dx, a.1 + t * dz);
    (p.0 - cx).powi(2) + (p.1 - cz).powi(2)
}

impl ProtectedMask for Exclusion {
    fn protected(&self, x: i32, _y: i32, z: i32) -> bool {
        let p = (x as f64, z as f64);
        match self {
            Exclusion::Circle { center, radius } => {
                (p.0 - center.0).powi(2) + (p.1 - center.1).powi(2) <= radius * radius
            }
            Exclusion::Corridor { points, half_width } => {
                let h = *half_width;
                let hit = |a: (f64, f64), b: (f64, f64)| {
                    // Per-segment bounding box (grown by the half-width) first.
                    p.0 >= a.0.min(b.0) - h
                        && p.0 <= a.0.max(b.0) + h
                        && p.1 >= a.1.min(b.1) - h
                        && p.1 <= a.1.max(b.1) + h
                        && dist2_to_segment(p, a, b) <= h * h
                };
                match points.as_slice() {
                    [] => false,
                    [a] => hit(*a, *a),
                    pts => pts.windows(2).any(|w| hit(w[0], w[1])),
                }
            }
        }
    }
}

/// A set of exclusions; protects a cell if any member does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exclusions(pub Vec<Exclusion>);

impl ProtectedMask for Exclusions {
    fn protected(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.iter().any(|e| e.protected(x, y, z))
    }
}

#[cfg(test)]
mod exclusion_tests;
#[cfg(test)]
mod tests;
