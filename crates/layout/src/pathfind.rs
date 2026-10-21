//! Eroded navigable set and 8-connected A* (docs/design/world-layout.md 6.2,
//! Phase E).
//!
//! Choices made here:
//! - Coordinates are local to the [`NavMap`] window: `[x, z]` with
//!   `0 <= x < width`, `0 <= z < depth`. The caller picks the window and
//!   converts world columns (anchorages) to local ones.
//! - Erosion treats columns outside the window as unknown, not blocked: only
//!   a non-navigable column inside the window erodes its neighbours. The
//!   caller should pad the window by at least the half-width.
//! - [`find_path`] searches the navigable columns of the map it is given;
//!   pass it the result of [`erode`]. If an endpoint was eroded away there is
//!   no path.
//! - Corner rule: a diagonal step is allowed only when both orthogonal
//!   neighbours it passes between are navigable. A path never squeezes or
//!   cuts across the corner of a blocked column.
//! - Orthogonal steps cost 1, diagonal steps sqrt 2. Ties in the open set go
//!   to the lowest estimate, then the lowest column index (`z * width + x`). Neighbours are tried in a fixed
//!   order, and a parent changes only for a strictly cheaper route, so the
//!   same map always gives the same path.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::NavMap;

const SQRT2: f64 = std::f64::consts::SQRT_2;

/// Neighbour offsets `(dx, dz)` in the fixed expansion order.
const STEPS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

/// Remove every navigable column that has a non-navigable column inside the
/// window within `half_width` (Chebyshev distance).
pub fn erode(nav: &NavMap, half_width: u32) -> NavMap {
    let (w, d, r) = (nav.width as i64, nav.depth as i64, half_width as i64);
    // Separable: a column survives if its whole (2r+1)-square is navigable.
    let run = |flags: &[bool], along_x: bool| -> Vec<bool> {
        let mut out = vec![false; flags.len()];
        for z in 0..d {
            for x in 0..w {
                out[(z * w + x) as usize] = (-r..=r).all(|k| {
                    let (nx, nz) = if along_x { (x + k, z) } else { (x, z + k) };
                    nx < 0 || nz < 0 || nx >= w || nz >= d || flags[(nz * w + nx) as usize]
                });
            }
        }
        out
    };
    NavMap {
        width: nav.width,
        depth: nav.depth,
        navigable: run(&run(&nav.navigable, true), false),
    }
}

/// Open-set entry: ordered so the `BinaryHeap` pops the lowest estimate,
/// then the lowest column index.
struct Open {
    f: f64,
    index: usize,
}

impl PartialEq for Open {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.index.cmp(&self.index))
    }
}

/// Shortest 8-connected path over the navigable columns of `nav`, from
/// `from` to `to` inclusive (local `[x, z]`), or `None` when either end is
/// outside the window or not navigable, or no path exists.
pub fn find_path(nav: &NavMap, from: [i32; 2], to: [i32; 2]) -> Option<Vec<[i32; 2]>> {
    let (w, d) = (nav.width as i32, nav.depth as i32);
    let open_at =
        |x: i32, z: i32| x >= 0 && z >= 0 && x < w && z < d && nav.navigable[(z * w + x) as usize];
    if !open_at(from[0], from[1]) || !open_at(to[0], to[1]) {
        return None;
    }
    let idx = |p: [i32; 2]| (p[1] * w + p[0]) as usize;
    let heuristic = |x: i32, z: i32| {
        let (dx, dz) = ((x - to[0]).abs() as f64, (z - to[1]).abs() as f64);
        dx.max(dz) + (SQRT2 - 1.0) * dx.min(dz)
    };
    let n = (w * d) as usize;
    let mut g = vec![f64::INFINITY; n];
    let mut parent = vec![usize::MAX; n];
    let mut closed = vec![false; n];
    let mut heap = BinaryHeap::new();
    g[idx(from)] = 0.0;
    heap.push(Open {
        f: heuristic(from[0], from[1]),
        index: idx(from),
    });
    while let Some(Open { index, .. }) = heap.pop() {
        if closed[index] {
            continue;
        }
        closed[index] = true;
        let (x, z) = (index as i32 % w, index as i32 / w);
        if [x, z] == to {
            let mut path = vec![to];
            let mut at = index;
            while parent[at] != usize::MAX {
                at = parent[at];
                path.push([at as i32 % w, at as i32 / w]);
            }
            path.reverse();
            return Some(path);
        }
        for (dx, dz) in STEPS {
            let (nx, nz) = (x + dx, z + dz);
            if !open_at(nx, nz) {
                continue;
            }
            let diagonal = dx != 0 && dz != 0;
            if diagonal && !(open_at(x + dx, z) && open_at(x, z + dz)) {
                continue;
            }
            let ni = idx([nx, nz]);
            let cost = g[index] + if diagonal { SQRT2 } else { 1.0 };
            if !closed[ni] && cost < g[ni] {
                g[ni] = cost;
                parent[ni] = index;
                heap.push(Open {
                    f: cost + heuristic(nx, nz),
                    index: ni,
                });
            }
        }
    }
    None
}
