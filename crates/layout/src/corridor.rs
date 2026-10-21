//! The corridor: regions near the route (design: world-layout Phase E and
//! Decision 5). Regions outside it are not generated or drawn.
//!
//! A region is "passed through" if the route polyline touches its closed
//! square. The corridor is every region within `margin` regions (Chebyshev)
//! of one of those.

use std::collections::BTreeSet;

use crate::{RegionCoord, Route, REGION_SIZE};

/// A set of region coordinates around a route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Corridor {
    regions: BTreeSet<RegionCoord>,
}

/// Whether segment `a`-`b` meets the closed square `[lo, lo + size]` in x and z.
fn segment_meets_square(a: [f64; 2], b: [f64; 2], lo: [f64; 2], size: f64) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for axis in 0..2 {
        let d = b[axis] - a[axis];
        let (min, max) = (lo[axis], lo[axis] + size);
        if d == 0.0 {
            if a[axis] < min || a[axis] > max {
                return false;
            }
        } else {
            let (u, v) = ((min - a[axis]) / d, (max - a[axis]) / d);
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

impl Corridor {
    /// The corridor of `route` with `margin` regions of slack on every side.
    pub fn new(route: &Route, margin: u32) -> Corridor {
        let size = REGION_SIZE as f64;
        let mut passed = BTreeSet::new();
        let pts = &route.points;
        if let [p] = pts.as_slice() {
            passed.insert(RegionCoord::containing(
                p[0].floor() as i32,
                p[1].floor() as i32,
            ));
        }
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let lo = |f: fn(f64, f64) -> f64, i: usize| (f(a[i], b[i]) / size).floor() as i32;
            let (rx0, rx1) = (lo(f64::min, 0), lo(f64::max, 0));
            let (rz0, rz1) = (lo(f64::min, 1), lo(f64::max, 1));
            for rx in rx0..=rx1 {
                for rz in rz0..=rz1 {
                    let c = RegionCoord::new(rx, rz);
                    let (ox, oz) = c.origin();
                    if segment_meets_square(a, b, [ox as f64, oz as f64], size) {
                        passed.insert(c);
                    }
                }
            }
        }
        let m = margin as i32;
        let mut regions = BTreeSet::new();
        for c in &passed {
            for dx in -m..=m {
                for dz in -m..=m {
                    regions.insert(RegionCoord::new(c.rx + dx, c.rz + dz));
                }
            }
        }
        Corridor { regions }
    }

    /// Whether `coord` is within the corridor.
    pub fn contains(&self, coord: RegionCoord) -> bool {
        self.regions.contains(&coord)
    }

    /// Every region in the corridor, sorted by `(rx, rz)`.
    pub fn regions(&self) -> Vec<RegionCoord> {
        self.regions.iter().copied().collect()
    }
}
