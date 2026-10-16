//! Swept-AABB movement with collision (basic-gameplay.md 4.4).
//!
//! Boxes overlapping the swept box are collected once, then the move is
//! resolved y first, then x, then z, clipping against every box per axis so
//! the mover ends flush against what blocks it. [`move_with_step`] adds step-up.

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

/// [`move_box`] with step-up (4.4): when the horizontal move was blocked and
/// the mover is on ground, retry with the box raised by `step_height` and
/// keep that result only if it moved farther horizontally. `on_ground` is the
/// mover's state before the move.
pub fn move_with_step(
    world: &impl CollisionWorld,
    bb: &Aabb,
    velocity: [f64; 3],
    step_height: f64,
    on_ground: bool,
) -> Move {
    let plain = move_box(world, bb, velocity);
    let blocked = plain.applied[0] != velocity[0] || plain.applied[2] != velocity[2];
    if !blocked || !(on_ground || plain.on_ground) {
        return plain;
    }
    // Raise (stopping at any ceiling), move horizontally, then settle back down.
    let up = move_box(world, bb, [0.0, step_height, 0.0]);
    let across = move_box(world, &up.aabb, [velocity[0], 0.0, velocity[2]]);
    let down = move_box(world, &across.aabb, [0.0, -up.applied[1], 0.0]);
    let dist = |a: &[f64; 3]| a[0] * a[0] + a[2] * a[2];
    let stepped = [across.applied[0], 0.0, across.applied[2]];
    if dist(&stepped) <= dist(&plain.applied) {
        return plain;
    }
    Move {
        aabb: down.aabb,
        applied: [stepped[0], down.aabb.min[1] - bb.min[1], stepped[2]],
        on_ground: down.on_ground,
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

    /// Test world with game-side half-height slabs (decided in #88).
    struct SlabWorld {
        inner: MemoryWorld,
        slabs: Vec<(i32, i32, i32)>,
    }

    impl CollisionWorld for SlabWorld {
        fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
            self.inner.block_at(x, y, z)
        }
        fn boxes_in(&self, query: &Aabb, out: &mut Vec<Aabb>) {
            self.inner.boxes_in(query, out);
            for &(x, y, z) in &self.slabs {
                let (x, y, z) = (x as f64, y as f64, z as f64);
                let b = Aabb::new([x, y, z], [x + 1.0, y + 0.5, z + 1.0]);
                if b.intersects(query) {
                    out.push(b);
                }
            }
        }
    }

    fn slab_world() -> SlabWorld {
        SlabWorld {
            inner: floor(),
            slabs: vec![(1, 1, 0)],
        }
    }

    #[test]
    fn step_up_succeeds_on_slab_edge() {
        let w = slab_world();
        let bb = player(0.5, 1.0, 0.5);
        let m = move_with_step(&w, &bb, [0.4, 0.0, 0.0], 0.5, true);
        assert!((m.applied[0] - 0.4).abs() < 1e-12);
        assert!((m.aabb.min[1] - 1.5).abs() < 1e-12);
        assert!(m.on_ground);
        // Without stepping the slab blocks.
        assert!(move_box(&w, &bb, [0.4, 0.0, 0.0]).applied[0] < 0.4);
    }

    #[test]
    fn step_up_fails_on_full_block() {
        let mut w = slab_world();
        w.inner.set(1, 1, 0, Block::Stone);
        w.slabs.clear();
        let bb = player(0.5, 1.0, 0.5);
        let plain = move_box(&w, &bb, [0.4, 0.0, 0.0]);
        let m = move_with_step(&w, &bb, [0.4, 0.0, 0.0], 0.5, true);
        assert_eq!(m, plain);
        assert_eq!(m.aabb.min[1], 1.0);
    }

    #[test]
    fn step_requires_ground() {
        let w = slab_world();
        let bb = player(0.5, 1.2, 0.5);
        let m = move_with_step(&w, &bb, [0.4, 0.0, 0.0], 0.5, false);
        assert_eq!(m, move_box(&w, &bb, [0.4, 0.0, 0.0]));
    }

    #[test]
    fn stepped_result_discarded_when_not_farther() {
        // A ceiling just above the slab stops the raised box from moving
        // farther than the plain move, so the plain result is kept.
        let mut w = slab_world();
        w.inner.set(1, 3, 0, Block::Stone);
        let bb = player(0.5, 1.0, 0.5);
        let m = move_with_step(&w, &bb, [0.4, 0.0, 0.0], 0.5, true);
        assert_eq!(m, move_box(&w, &bb, [0.4, 0.0, 0.0]));
        assert_eq!(m.aabb.min[1], 1.0);
    }

    #[test]
    fn jump_onto_one_block_ledge() {
        let mut w = floor();
        w.set(1, 1, 0, Block::Stone);
        let mut bb = player(0.5, 1.0, 0.5);
        let (g, drag) = (0.08, 0.98);
        let mut vy = 0.42;
        let mut ground = true;
        for _ in 0..40 {
            let m = move_with_step(&w, &bb, [0.1, vy, 0.0], 0.5, ground);
            bb = m.aabb;
            ground = m.on_ground;
            if ground && bb.min[1] > 1.5 {
                break;
            }
            vy = if ground { -g } else { (vy - g) * drag };
        }
        assert!(ground);
        assert!((bb.min[1] - 2.0).abs() < 1e-9, "{bb:?}");
        assert!(bb.max[0] > 1.0);
        // From flat ground the same step height cannot climb the block.
        let m = move_with_step(&w, &player(0.5, 1.0, 0.5), [0.4, 0.0, 0.0], 0.5, true);
        assert_eq!(m.aabb.min[1], 1.0);
    }

    #[test]
    fn fuzz_with_stepping_never_leaves_walled_box() {
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
        w.set(3, 1, 3, Block::Stone);
        let mut rng = Rng(0xD1B54A32D192ED03);
        for _ in 0..300 {
            let mut bb = Aabb::from_size(
                [
                    rng.range(1.0, 5.4),
                    rng.range(1.0, 4.2),
                    rng.range(1.0, 5.4),
                ],
                [0.6, 1.8, 0.6],
            );
            let mut ground = false;
            for _ in 0..20 {
                let v = [
                    rng.range(-12.0, 12.0),
                    rng.range(-12.0, 12.0),
                    rng.range(-12.0, 12.0),
                ];
                let m = move_with_step(&w, &bb, v, 0.5, ground || rng.next() < 0.5);
                bb = m.aabb;
                ground = m.on_ground;
                for i in 0..3 {
                    assert!(bb.min[i] >= 1.0 - 1e-6 && bb.max[i] <= 6.0 + 1e-6, "{bb:?}");
                }
            }
        }
    }
}
