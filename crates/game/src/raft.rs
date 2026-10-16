//! The raft as a collision source that carries riders
//! (basic-gameplay.md 4.7, first bullet and the #140 bullet; 4.4).
//!
//! The raft owns its mask cells (the layout crate's `raft_cells`). Each tick it
//! takes a `RaftFrame`: it translates by the displacement and turns by the yaw
//! change in 90-degree steps only, so its boxes stay axis-aligned. Entities
//! whose ground support is a raft cell get the same translation and quarter
//! turns around the raft's centre before their own movement. Tilt is carried in
//! the frame but not used here (the swell push is a separate step).

use crate::collision::{block_boxes_in, Aabb, CollisionWorld};
use crate::physics::PlayerPhysics;
use crate::player::Player;
use std::collections::HashSet;
use std::f64::consts::FRAC_PI_2;
use terrain::Block;

/// How far below a body's feet a raft box may be to count as support.
const SUPPORT_EPS: f64 = 1e-3;

/// One tick of raft motion.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaftFrame {
    pub displacement: [f64; 3],
    /// Yaw change in radians; positive turns forward (-z) towards +x.
    pub yaw_delta: f64,
    /// Degrees about the x and z axes.
    pub tilt: [f64; 2],
}

/// Rotate `p` by `turns` quarter turns (same sense as positive yaw) about
/// the point `c`, in the xz plane.
fn rotate(p: [f64; 3], c: [f64; 2], turns: i32) -> [f64; 3] {
    let (mut dx, mut dz) = (p[0] - c[0], p[2] - c[1]);
    for _ in 0..turns.rem_euclid(4) {
        (dx, dz) = (-dz, dx);
    }
    [c[0] + dx, p[1], c[1] + dz]
}

#[derive(Clone, Debug)]
pub struct Raft {
    /// Mask cells at the rest pose.
    cells: Vec<[i32; 3]>,
    rest: HashSet<[i32; 3]>,
    /// Rest-pose centre of the mask's xz bounding box.
    centre: [f64; 2],
    offset: [f64; 3],
    /// Yaw change not yet turned into a quarter turn.
    yaw: f64,
    /// Quarter turns applied so far.
    turns: i32,
}

impl Raft {
    pub fn new(cells: &[[i32; 3]]) -> Raft {
        let range = |i: usize| {
            let lo = cells.iter().map(|c| c[i]).min().unwrap_or(0);
            let hi = cells.iter().map(|c| c[i]).max().unwrap_or(0);
            (lo as f64 + hi as f64 + 1.0) / 2.0
        };
        Raft {
            cells: cells.to_vec(),
            rest: cells.iter().copied().collect(),
            centre: [range(0), range(2)],
            offset: [0.0; 3],
            yaw: 0.0,
            turns: 0,
        }
    }

    /// The raft's current centre in the xz plane.
    pub fn centre(&self) -> [f64; 2] {
        [
            self.centre[0] + self.offset[0],
            self.centre[1] + self.offset[2],
        ]
    }

    pub fn offset(&self) -> [f64; 3] {
        self.offset
    }

    /// Quarter turns applied so far.
    pub fn quarter_turns(&self) -> i32 {
        self.turns
    }

    /// The collision box of every cell at the current pose.
    pub fn boxes(&self) -> impl Iterator<Item = Aabb> + '_ {
        self.cells.iter().map(|c| {
            let (x, y, z) = (c[0] as f64, c[1] as f64, c[2] as f64);
            let a = rotate([x, y, z], self.centre, self.turns);
            let b = rotate([x + 1.0, y + 1.0, z + 1.0], self.centre, self.turns);
            let lo = |i: usize| a[i].min(b[i]) + self.offset[i];
            let hi = |i: usize| a[i].max(b[i]) + self.offset[i];
            Aabb::new([lo(0), lo(1), lo(2)], [hi(0), hi(1), hi(2)])
        })
    }

    /// Whether a raft box holds up a body occupying `bb`: the box under its
    /// feet is touching a raft top.
    pub fn supports(&self, bb: &Aabb) -> bool {
        let probe = bb.offset([0.0, -SUPPORT_EPS, 0.0]);
        self.boxes().any(|b| b.intersects(&probe))
    }

    /// Move the raft by one frame. Each rider supported by the raft before it
    /// moves is carried by the same translation and quarter turns.
    pub fn advance(
        &mut self,
        frame: &RaftFrame,
        riders: &mut [&mut Player],
        physics: &PlayerPhysics,
    ) {
        let carried: Vec<bool> = riders
            .iter()
            .map(|p| self.supports(&p.aabb(physics)))
            .collect();
        for i in 0..3 {
            self.offset[i] += frame.displacement[i];
        }
        self.yaw += frame.yaw_delta;
        let step = (self.yaw / FRAC_PI_2).round() as i32;
        self.yaw -= step as f64 * FRAC_PI_2;
        self.turns = (self.turns + step).rem_euclid(4);
        let centre = self.centre();
        for (p, carried) in riders.iter_mut().zip(carried) {
            if carried {
                for i in 0..3 {
                    p.pos[i] += frame.displacement[i];
                }
                p.pos = rotate(p.pos, centre, step);
            }
        }
    }
}

/// The base world with the raft laid over it. The raft's rest-pose cells are
/// air in the base blocks (the raft has moved off them); its boxes are added.
pub struct RaftWorld<'a, W: CollisionWorld> {
    pub base: &'a W,
    pub raft: &'a Raft,
}

impl<'a, W: CollisionWorld> RaftWorld<'a, W> {
    pub fn new(base: &'a W, raft: &'a Raft) -> RaftWorld<'a, W> {
        RaftWorld { base, raft }
    }
}

impl<W: CollisionWorld> CollisionWorld for RaftWorld<'_, W> {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        if self.raft.rest.contains(&[x, y, z]) {
            Block::Air
        } else {
            self.base.block_at(x, y, z)
        }
    }

    fn boxes_in(&self, query: &Aabb, out: &mut Vec<Aabb>) {
        block_boxes_in(self, query, out);
        out.extend(self.raft.boxes().filter(|b| b.intersects(query)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::input::TickInput;

    fn phys() -> PlayerPhysics {
        PlayerPhysics::default_from_data().unwrap()
    }

    /// A 4x4 deck at y = 64, x and z in 0..4, stamped into the base world too.
    fn deck() -> (MemoryWorld, Raft) {
        let mut w = MemoryWorld::new();
        let mut cells = Vec::new();
        for x in 0..4 {
            for z in 0..4 {
                w.set(x, 64, z, Block::Stone);
                cells.push([x, 64, z]);
            }
        }
        (w, Raft::new(&cells))
    }

    fn settle(pl: &mut Player, p: &PlayerPhysics, w: &impl CollisionWorld) {
        for _ in 0..20 {
            pl.tick(&TickInput::default(), p, w);
        }
        assert!(pl.on_ground);
    }

    fn slide(dx: f64) -> RaftFrame {
        RaftFrame {
            displacement: [dx, 0.0, 0.0],
            ..RaftFrame::default()
        }
    }

    #[test]
    fn deck_blocks_and_supports_like_full_blocks() {
        let (base, raft) = deck();
        let p = phys();
        let world = RaftWorld::new(&base, &raft);
        let mut pl = Player::new([2.0, 65.5, 2.0], &p);
        settle(&mut pl, &p, &world);
        assert_eq!(pl.pos[1], 65.0);
        // Walk: forward at yaw 0 is -z, and the deck edge is at z = 0.
        let walk = TickInput {
            move_forward: 1.0,
            ..TickInput::default()
        };
        for _ in 0..5 {
            pl.tick(&walk, &p, &world);
        }
        assert!(pl.pos[2] < 2.0 && pl.on_ground);
        // Jump and land back on the deck.
        let jump = TickInput {
            jump: true,
            ..TickInput::default()
        };
        pl.tick(&jump, &p, &world);
        assert!(!pl.on_ground && pl.pos[1] > 65.0);
        settle(&mut pl, &p, &world);
        assert_eq!(pl.pos[1], 65.0);
    }

    #[test]
    fn rider_moves_with_the_raft() {
        let (base, mut raft) = deck();
        let p = phys();
        let mut pl = Player::new([2.0, 65.0, 2.0], &p);
        settle(&mut pl, &p, &RaftWorld::new(&base, &raft));
        for n in 1..=20 {
            raft.advance(&slide(0.05), &mut [&mut pl], &p);
            let world = RaftWorld::new(&base, &raft);
            pl.tick(&TickInput::default(), &p, &world);
            assert!(pl.on_ground, "tick {n}");
            assert!(
                (pl.pos[0] - (2.0 + 0.05 * n as f64)).abs() < 1e-9,
                "tick {n}"
            );
        }
        assert!((raft.offset()[0] - 1.0).abs() < 1e-9);
        // The old cells are gone: a body left behind at x = 0.5 falls through.
        let world = RaftWorld::new(&base, &raft);
        let mut other = Player::new([0.5, 66.0, 0.5], &p);
        for _ in 0..20 {
            other.tick(&TickInput::default(), &p, &world);
        }
        assert!(other.pos[1] < 64.0);
    }

    #[test]
    fn unsupported_player_is_not_carried() {
        let (_, mut raft) = deck();
        let p = phys();
        let mut off = Player::new([10.0, 65.0, 2.0], &p);
        let mut up = Player::new([2.0, 67.0, 2.0], &p);
        raft.advance(&slide(0.5), &mut [&mut off, &mut up], &p);
        assert_eq!(off.pos, [10.0, 65.0, 2.0]);
        assert_eq!(up.pos, [2.0, 67.0, 2.0]);
    }

    #[test]
    fn support_is_lost_when_walking_off() {
        let (base, mut raft) = deck();
        let p = phys();
        let mut pl = Player::new([2.0, 65.0, 2.0], &p);
        pl.on_ground = true;
        let walk = TickInput {
            move_strafe: 1.0,
            ..TickInput::default()
        };
        // Walk +x, off the edge at x = 4 (the raft is still).
        for _ in 0..40 {
            raft.advance(&RaftFrame::default(), &mut [&mut pl], &p);
            let world = RaftWorld::new(&base, &raft);
            pl.tick(&walk, &p, &world);
        }
        assert!(pl.pos[0] > 4.3 && !raft.supports(&pl.aabb(&p)));
        let at = pl.pos;
        raft.advance(&slide(1.0), &mut [&mut pl], &p);
        assert_eq!(pl.pos, at);
    }

    #[test]
    fn yaw_turns_in_quarter_steps_and_carries_riders() {
        let (base, mut raft) = deck();
        let p = phys();
        let mut pl = Player::new([3.5, 65.0, 2.0], &p);
        settle(&mut pl, &p, &RaftWorld::new(&base, &raft));
        let before: Vec<Aabb> = raft.boxes().collect();
        let eighth = RaftFrame {
            yaw_delta: FRAC_PI_2 / 3.0,
            ..RaftFrame::default()
        };
        // Below half a quarter turn nothing happens.
        raft.advance(&eighth, &mut [&mut pl], &p);
        assert_eq!(raft.quarter_turns(), 0);
        assert_eq!(pl.pos, [3.5, 65.0, 2.0]);
        // Reaching the half-step threshold turns by 90 degrees about (2, 2).
        raft.advance(&eighth, &mut [&mut pl], &p);
        assert_eq!(raft.quarter_turns(), 1);
        assert!((pl.pos[0] - 2.0).abs() < 1e-9 && (pl.pos[2] - 3.5).abs() < 1e-9);
        // The square deck maps onto itself, still axis-aligned.
        let mut after: Vec<Aabb> = raft.boxes().collect();
        let key = |b: &Aabb| (b.min[0] as i64, b.min[2] as i64);
        let mut b2 = before.clone();
        after.sort_by_key(key);
        b2.sort_by_key(key);
        assert_eq!(after, b2);
        let world = RaftWorld::new(&base, &raft);
        pl.tick(&TickInput::default(), &p, &world);
        assert!(pl.on_ground);
        // Four quarter turns come back to the start.
        for _ in 0..3 {
            raft.advance(
                &RaftFrame {
                    yaw_delta: FRAC_PI_2,
                    ..RaftFrame::default()
                },
                &mut [],
                &p,
            );
        }
        assert_eq!(raft.quarter_turns(), 0);
    }
}
