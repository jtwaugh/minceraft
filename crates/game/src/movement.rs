//! Swept-AABB movement with collision (basic-gameplay.md 4.4).
//!
//! Boxes overlapping the swept box are collected once, then the move is
//! resolved y first, then x, then z, clipping against every box per axis so
//! the mover ends flush against what blocks it. Step-up is not handled here.

use crate::collision::{Aabb, CollisionWorld};

/// Tolerance for "touching": boxes clipped flush may differ by float error.
const EPS: f64 = 1e-9;

/// The outcome of [`move_box`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    /// The box after moving.
    pub aabb: Aabb,
    /// The velocity actually applied on each axis (`[x, y, z]`).
    pub applied: [f64; 3],
    /// True when the vertical move was clipped while moving downward.
    pub on_ground: bool,
}

/// True when `a` and `b` overlap on every axis except `skip`.
fn overlaps_except(a: &Aabb, b: &Aabb, skip: usize) -> bool {
    (0..3)
        .filter(|&i| i != skip)
        .all(|i| a.min[i] + EPS < b.max[i] && a.max[i] > b.min[i] + EPS)
}

/// Clip `d` along `axis` so `bb` moved by it stays out of every box in `boxes`.
fn clip(bb: &Aabb, boxes: &[Aabb], axis: usize, mut d: f64) -> f64 {
    for b in boxes {
        if !overlaps_except(bb, b, axis) {
            continue;
        }
        if d > 0.0 && b.min[axis] >= bb.max[axis] - EPS {
            d = d.min(b.min[axis] - bb.max[axis]);
        } else if d < 0.0 && b.max[axis] <= bb.min[axis] + EPS {
            d = d.max(b.max[axis] - bb.min[axis]);
        }
    }
    d
}

/// Move `bb` by `velocity` through `world`, resolving y, then x, then z.
pub fn move_box(world: &impl CollisionWorld, bb: &Aabb, velocity: [f64; 3]) -> Move {
    // Every box the mover can touch lies inside the box swept over all three moves.
    let mut swept = *bb;
    for i in 0..3 {
        swept.min[i] += velocity[i].min(0.0);
        swept.max[i] += velocity[i].max(0.0);
    }
    let mut boxes = Vec::new();
    world.boxes_in(&swept, &mut boxes);

    let mut cur = *bb;
    let mut applied = [0.0; 3];
    for axis in [1, 0, 2] {
        let d = clip(&cur, &boxes, axis, velocity[axis]);
        let mut delta = [0.0; 3];
        delta[axis] = d;
        cur = cur.offset(delta);
        applied[axis] = d;
    }
    Move {
        aabb: cur,
        applied,
        on_ground: velocity[1] < 0.0 && applied[1] > velocity[1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use terrain::Block;

    fn player(x: f64, y: f64, z: f64) -> Aabb {
        Aabb::from_size([x - 0.3, y, z - 0.3], [0.6, 1.8, 0.6])
    }

    fn floor() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -5..5 {
            for z in -5..5 {
                w.set(x, 0, z, Block::Stone);
            }
        }
        w
    }

    #[test]
    fn falls_flush_onto_floor_and_is_grounded() {
        let w = floor();
        let m = move_box(&w, &player(0.0, 2.5, 0.0), [0.0, -5.0, 0.0]);
        assert_eq!(m.aabb.min[1], 1.0);
        assert_eq!(m.applied[1], -1.5);
        assert!(m.on_ground);
    }

    #[test]
    fn on_ground_only_when_downward_move_clipped() {
        let w = floor();
        // Free fall that does not reach the floor.
        assert!(!move_box(&w, &player(0.0, 3.0, 0.0), [0.0, -1.0, 0.0]).on_ground);
        // Upward and horizontal moves while resting on the floor.
        let rest = player(0.0, 1.0, 0.0);
        assert!(!move_box(&w, &rest, [0.1, 0.2, 0.0]).on_ground);
        assert!(!move_box(&w, &rest, [0.1, 0.0, 0.0]).on_ground);
        // Hitting a ceiling while rising is not ground.
        let mut c = floor();
        c.set(0, 3, 0, Block::Stone);
        let m = move_box(&c, &player(0.0, 1.0, 0.0), [0.0, 2.0, 0.0]);
        assert_eq!(m.aabb.max[1], 3.0);
        assert!(!m.on_ground);
        // Pressing down while resting is clipped to zero: grounded.
        assert!(move_box(&w, &rest, [0.0, -0.08, 0.0]).on_ground);
    }

    #[test]
    fn horizontal_moves_clip_flush_per_axis() {
        let mut w = floor();
        w.set(2, 1, 0, Block::Stone);
        w.set(1, 1, 2, Block::Stone);
        let m = move_box(&w, &player(0.5, 1.0, 0.5), [3.0, 0.0, 3.0]);
        assert!((m.aabb.max[0] - 2.0).abs() < 1e-12);
        assert!((m.aabb.max[2] - 2.0).abs() < 1e-12);
        // Sliding: blocked in x, free in z.
        let m = move_box(&w, &player(1.6, 1.0, 0.5), [1.0, 0.0, 0.3]);
        assert_eq!(m.applied[0].round(), 0.0);
        assert!((m.applied[2] - 0.3).abs() < 1e-12);
    }

    #[test]
    fn plants_never_block() {
        let mut w = floor();
        for (i, b) in [Block::TallGrass, Block::Dandelion, Block::Rose, Block::Fern]
            .into_iter()
            .enumerate()
        {
            w.set(i as i32 + 1, 1, 0, b);
            w.set(i as i32 + 1, 2, 0, b);
        }
        let m = move_box(&w, &player(0.5, 1.0, 0.5), [5.0, 0.0, 0.0]);
        assert_eq!(m.applied, [5.0, 0.0, 0.0]);
        assert!(!m.on_ground);
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next()
        }
    }

    #[test]
    fn fuzz_never_leaves_walled_box() {
        // Hollow stone cube: interior is 1..6 on every axis.
        let mut w = MemoryWorld::new();
        for x in 0..7 {
            for y in 0..7 {
                for z in 0..7 {
                    if [x, y, z].iter().any(|&c| c == 0 || c == 6) {
                        w.set(x, y, z, Block::Stone);
                    }
                }
            }
        }
        let mut rng = Rng(0x9E3779B97F4A7C15);
        for _ in 0..300 {
            let mut bb = Aabb::from_size(
                [
                    rng.range(1.0, 5.4),
                    rng.range(1.0, 4.2),
                    rng.range(1.0, 5.4),
                ],
                [0.6, 1.8, 0.6],
            );
            for _ in 0..20 {
                let v = [
                    rng.range(-12.0, 12.0),
                    rng.range(-12.0, 12.0),
                    rng.range(-12.0, 12.0),
                ];
                bb = move_box(&w, &bb, v).aabb;
                for i in 0..3 {
                    assert!(bb.min[i] >= 1.0 - 1e-6 && bb.max[i] <= 6.0 + 1e-6, "{bb:?}");
                }
            }
        }
    }
}
