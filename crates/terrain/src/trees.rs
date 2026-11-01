//! Tree generators (SPEC 11): oak, birch, taiga A ("pine"), taiga B
//! ("spruce") and big oak. Shapes come from `[trees.*]` in
//! `data/biomes.toml`.
//!
//! Every generator stages its writes (`World::stage`) against the pre-tree
//! state and commits them at the end; a tree touching a protected cell is
//! skipped entirely (`World::commit`).

use crate::data::{is_leaves, is_opaque_cube, tree_params, TreeKind};
use crate::decorate::World;
use crate::noise::{floor, Rng};
use crate::{AIR, DIRT, GRASS, HEIGHT};

const H: i32 = HEIGHT as i32;

#[cfg(test)]
fn is_log(b: u8) -> bool {
    use crate::data::Block;
    b == Block::OakLog as u8 || b == Block::BirchLog as u8 || b == Block::SpruceLog as u8
}

#[cfg(test)]
std::thread_local! {
    /// Test hook: trunk-column log cells of every tree grown on this thread.
    pub(crate) static TRUNKS: core::cell::RefCell<alloc::vec::Vec<alloc::vec::Vec<(i32, i32, i32, u8)>>> =
        const { core::cell::RefCell::new(alloc::vec::Vec::new()) };
}

#[inline]
fn air_or_leaves(b: u8) -> bool {
    b == AIR || is_leaves(b)
}

/// Grow a tree of `kind` with its lowest trunk block at (x, y, z).
pub(crate) fn grow(w: &mut World, rng: &mut Rng, kind: TreeKind, x: i32, y: i32, z: i32) -> bool {
    let ok = match kind {
        TreeKind::Oak => small(w, rng, &tree_params::OAK, x, y, z),
        TreeKind::Birch => small(w, rng, &tree_params::BIRCH, x, y, z),
        TreeKind::TaigaA => taiga_a(w, rng, x, y, z),
        TreeKind::TaigaB => taiga_b(w, rng, x, y, z),
        TreeKind::BigOak => big_oak(w, rng, x, y, z),
    };
    if ok {
        #[cfg(test)]
        let trunk: alloc::vec::Vec<_> = w
            .pending()
            .iter()
            .filter(|e| e.0 == x && e.2 == z && is_log(e.3))
            .copied()
            .collect();
        let done = w.commit();
        #[cfg(test)]
        if done {
            TRUNKS.with(|t| t.borrow_mut().push(trunk));
        }
        done
    } else {
        w.discard();
        false
    }
}

/// Clearance: squares of radius `r(level)` from y to y + h + 1 must be air
/// or leaves and inside 0..128.
fn clear(w: &World, x: i32, y: i32, z: i32, top: i32, r: impl Fn(i32) -> i32) -> bool {
    for yy in y..=top {
        if !(0..H).contains(&yy) {
            return false;
        }
        let rr = r(yy);
        for dx in -rr..=rr {
            for dz in -rr..=rr {
                if !air_or_leaves(w.get(x + dx, yy, z + dz)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Ground check shared by the small trees; stages grass -> dirt.
fn ground(w: &mut World, x: i32, y: i32, z: i32, h: i32) -> bool {
    let g = w.get(x, y - 1, z);
    if !(g == GRASS || g == DIRT) || y >= H - h - 1 {
        return false;
    }
    w.stage(x, y - 1, z, DIRT);
    true
}

/// A leaf layer of radius r with corners omitted (`keep_corner` decides).
fn leaf_layer(
    w: &mut World,
    x: i32,
    yy: i32,
    z: i32,
    r: i32,
    leaves: u8,
    mut keep_corner: impl FnMut() -> bool,
) {
    for dx in -r..=r {
        for dz in -r..=r {
            if r > 0 && dx.abs() == r && dz.abs() == r && !keep_corner() {
                continue;
            }
            if !is_opaque_cube(w.get(x + dx, yy, z + dz)) {
                w.stage(x + dx, yy, z + dz, leaves);
            }
        }
    }
}

fn trunk(w: &mut World, x: i32, y0: i32, y1: i32, z: i32, log: u8) {
    for yy in y0..=y1 {
        if air_or_leaves(w.get(x, yy, z)) {
            w.stage(x, yy, z, log);
        }
    }
}

/// Oak / birch (SPEC 11.1, 11.2).
fn small(w: &mut World, rng: &mut Rng, p: &crate::data::SmallTree, x: i32, y: i32, z: i32) -> bool {
    let h = (p.height.0 + rng.below(p.height.1)) as i32;
    if y < 1 || y + h + 1 > H {
        return false;
    }
    let rmax = p.canopy.iter().copied().max().unwrap_or(0);
    let ok = clear(w, x, y, z, y + h + 1, |yy| {
        if yy == y {
            0
        } else if yy <= y + h - 2 {
            (rmax - 1).max(0)
        } else {
            rmax
        }
    });
    if !ok || !ground(w, x, y, z, h) {
        return false;
    }
    let n = p.canopy.len() as i32;
    for (i, &r) in p.canopy.iter().enumerate() {
        let yy = y + h - (n - 1) + i as i32;
        let top = yy == y + h;
        // Lower layers keep each corner with probability 1/2; the top layer never.
        leaf_layer(w, x, yy, z, r, p.leaves, || !top && rng.below(2) != 0);
    }
    trunk(w, x, y, y + h - 1, z, p.log);
    true
}

/// Taiga tree A, the sparse pine (SPEC 11.3).
fn taiga_a(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32) -> bool {
    let p = &tree_params::TAIGA_A;
    let h = (p.height.0 + rng.below(p.height.1)) as i32;
    let a = h - p.bare_trunk.0 as i32 - rng.below(p.bare_trunk.1) as i32;
    let l = h - a;
    let rmax = 1 + rng.below((l + 1) as u32) as i32;
    if y < 1 || y + h + 1 > H {
        return false;
    }
    if !clear(
        w,
        x,
        y,
        z,
        y + h + 1,
        |yy| if yy < y + a { 0 } else { rmax },
    ) {
        return false;
    }
    if !ground(w, x, y, z, h) {
        return false;
    }
    let mut r = 0;
    let mut yy = y + h;
    while yy >= y + a {
        leaf_layer(w, x, yy, z, r, p.leaves, || false);
        if yy == y + a + 1 && r >= 1 {
            r -= 1;
        } else if r < rmax {
            r += 1;
        }
        yy -= 1;
    }
    trunk(w, x, y, y + h - 2, z, p.log);
    true
}

/// Taiga tree B, the layered spruce (SPEC 11.4).
fn taiga_b(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32) -> bool {
    let p = &tree_params::TAIGA_B;
    let h = (p.height.0 + rng.below(p.height.1)) as i32;
    let a = (p.bare_trunk.0 + rng.below(p.bare_trunk.1)) as i32;
    let rmax = (p.max_radius.0 + rng.below(p.max_radius.1)) as i32;
    if y < 1 || y + h + 1 > H {
        return false;
    }
    if !clear(
        w,
        x,
        y,
        z,
        y + h + 1,
        |yy| if yy < y + a { 0 } else { rmax },
    ) {
        return false;
    }
    if !ground(w, x, y, z, h) {
        return false;
    }
    let mut r = rng.below(2) as i32;
    let (mut cap, mut restart) = (1, 0);
    let mut yy = y + h;
    while yy >= y + a {
        leaf_layer(w, x, yy, z, r, p.leaves, || false);
        if r >= cap {
            r = restart;
            restart = 1;
            cap = (cap + 1).min(rmax);
        } else {
            r += 1;
        }
        yy -= 1;
    }
    let logs = h - rng.below(3) as i32;
    trunk(w, x, y, y + logs - 1, z, p.log);
    true
}

// ---------------------------------------------------------------------------
// Big oak (SPEC 11.5)
// ---------------------------------------------------------------------------

/// Walk the line P -> Q one block per major-axis step (SPEC 11.5 "line
/// walk"). `round` selects the log walk (round) over the clearance walk
/// (floor). `f` returns false to stop; the stopping step is returned.
fn line(p: [i32; 3], q: [i32; 3], round: bool, mut f: impl FnMut([i32; 3]) -> bool) -> Option<i32> {
    let diff = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
    let mut major = 0;
    for a in 1..3 {
        if diff[a].abs() > diff[major].abs() {
            major = a;
        }
    }
    let n = diff[major];
    if n == 0 {
        return None;
    }
    let dir = n.signum();
    for step in 0..=n.abs() {
        let t = step * dir;
        let mut pos = [0i32; 3];
        for a in 0..3 {
            pos[a] = if a == major {
                p[a] + t
            } else {
                let v = p[a] as f64 + t as f64 * (diff[a] as f64 / n as f64);
                (if round { floor(v + 0.5) } else { floor(v) }) as i32
            };
        }
        if !f(pos) {
            return Some(step);
        }
    }
    None
}

fn line_clear(w: &World, p: [i32; 3], q: [i32; 3]) -> Option<i32> {
    line(p, q, false, |c| {
        (0..H).contains(&c[1]) && air_or_leaves(w.get(c[0], c[1], c[2]))
    })
}

fn big_oak(w: &mut World, rng: &mut Rng, x: i32, y: i32, z: i32) -> bool {
    let p = &tree_params::BIG_OAK;
    // Own stream seeded from one draw of the cell stream.
    let mut r = Rng::new(rng.next_u64());
    let mut height = (p.height.0 + r.below(p.height.1)) as i32;
    let g = w.get(x, y - 1, z);
    if !(g == GRASS || g == DIRT) || y < 1 {
        return false;
    }
    if let Some(k) = line_clear(w, [x, y, z], [x, y + height - 1, z]) {
        if k < p.min_height {
            return false;
        }
        height = k;
    }
    let hf = height as f64;
    let f = ((p.trunk_ratio * hf) as i32).min(height - 1);
    let per_level = {
        let q = p.foliage_density * hf / 13.0;
        ((1.382 + q * q) as i32).max(1)
    };
    let cluster_h = p.foliage_radii.len() as i32;

    // (cx, cy, cz, branch base y)
    let mut clusters: alloc::vec::Vec<(i32, i32, i32, i32)> = alloc::vec::Vec::new();
    clusters.push((x, y + height - cluster_h, z, y + f));
    let m = hf / 2.0;
    let mut lvl = height - cluster_h;
    while lvl >= 0 {
        let lf = lvl as f64;
        if lf >= p.level_cutoff * hf {
            let s = if lf == m {
                0.5 * m
            } else {
                0.5 * libm_sqrt(m * m - (m - lf) * (m - lf))
            };
            let cy = y + lvl - 1; // B7: shape uses l, position l - 1
            for _ in 0..per_level {
                let u1 = r.next_f64();
                let u2 = r.next_f64();
                let len = s * (u1 + 0.328);
                // SPEC 11.5 specifies 3.14159, not PI; using PI would change world output.
                #[allow(clippy::approx_constant)]
                let th = u2 * 2.0 * 3.14159;
                let cx = floor(x as f64 + len * libm_sin(th) + 0.5) as i32;
                let cz = floor(z as f64 + len * libm_cos(th) + 0.5) as i32;
                if line_clear(w, [cx, cy, cz], [cx, cy + cluster_h, cz]).is_some() {
                    continue;
                }
                let dx = (cx - x) as f64;
                let dz = (cz - z) as f64;
                let dist = libm_sqrt(dx * dx + dz * dz);
                let b = (y + f).min((cy as f64 - p.branch_slope * dist) as i32);
                if line_clear(w, [x, b, z], [cx, cy, cz]).is_some() {
                    continue;
                }
                clusters.push((cx, cy, cz, b));
            }
        }
        lvl -= 1;
    }

    // Foliage.
    for &(cx, cy, cz, _) in &clusters {
        for (i, &rad) in p.foliage_radii.iter().enumerate() {
            let yy = cy + i as i32;
            let e = (rad + 0.618) as i32;
            for dx in -e..=e {
                for dz in -e..=e {
                    let ax = dx.abs() as f64 + 0.5;
                    let az = dz.abs() as f64 + 0.5;
                    if libm_sqrt(ax * ax + az * az) > rad {
                        continue;
                    }
                    if air_or_leaves(w.get(cx + dx, yy, cz + dz)) {
                        w.stage(cx + dx, yy, cz + dz, p.leaves);
                    }
                }
            }
        }
    }
    // Trunk (unconditional log walk), then branches.
    let log = p.log;
    let mut put = |c: [i32; 3]| {
        w.stage(c[0], c[1], c[2], log);
        true
    };
    line([x, y, z], [x, y + f, z], true, &mut put);
    for &(cx, cy, cz, b) in &clusters {
        if (b - y) as f64 >= p.branch_cutoff * hf {
            line([x, b, z], [cx, cy, cz], true, &mut put);
        }
    }
    true
}

// Minimal no_std math (sqrt via Newton; sin/cos via range-reduced Taylor).

pub(crate) fn libm_sqrt(v: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    let mut g = if v > 1.0 { v * 0.5 } else { 1.0 };
    for _ in 0..40 {
        let n = 0.5 * (g + v / g);
        if (n - g).abs() <= 1e-15 * n {
            return n;
        }
        g = n;
    }
    g
}

fn libm_sin(x: f64) -> f64 {
    const PI: f64 = core::f64::consts::PI;
    let mut a = x - floor(x / (2.0 * PI)) * 2.0 * PI; // [0, 2pi)
    if a > PI {
        a -= 2.0 * PI; // (-pi, pi]
    }
    if a > PI / 2.0 {
        a = PI - a;
    } else if a < -PI / 2.0 {
        a = -PI - a;
    }
    let a2 = a * a;
    let mut term = a;
    let mut sum = a;
    for k in 1..12 {
        term *= -a2 / ((2 * k) as f64 * (2 * k + 1) as f64);
        sum += term;
    }
    sum
}

fn libm_cos(x: f64) -> f64 {
    libm_sin(x + core::f64::consts::FRAC_PI_2)
}
