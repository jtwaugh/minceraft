//! World layout: bias terms that steer the terrain generator.
//! See docs/design/world-layout.md (Phase A).

use terrain::BaseHeightBias;

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

#[cfg(test)]
mod tests;
