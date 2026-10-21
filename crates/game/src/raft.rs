//! The raft as a collision source that carries riders
//! (basic-gameplay.md 4.7, first bullet and the #140 bullet; 4.4).
//!
//! The raft owns its mask cells (the layout crate's `raft_cells`). Each tick it
//! takes a `RaftFrame`: it translates by the displacement and turns by the yaw
//! change in 90-degree steps only, so its boxes stay axis-aligned. Entities
//! whose ground support is a raft cell get the same translation and quarter
//! turns around the raft's centre before their own movement. Tilt is carried in
//! the frame and drives the swell push of 4.7 (third and fourth bullets).

use crate::collision::{
    block_boxes_in, block_selection_boxes_in, Aabb, CollisionWorld, SelectionBox,
};
use crate::physics::PlayerPhysics;
use crate::player::Player;
use std::collections::HashSet;
use std::f64::consts::FRAC_PI_2;
use terrain::Block;

const DEFAULT_RAFT: &str = include_str!("../../../data/raft.toml");

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

/// The scripted swell from `data/raft.toml`: the raft sways along a fixed
/// direction and its deck tilts, as a pure function of the tick number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swell {
    /// Peak sway from the rest pose, blocks.
    pub sway_amplitude: f64,
    /// Peak deck tilt, degrees.
    pub tilt_amplitude: f64,
    /// Ticks per swell cycle.
    pub period_ticks: u64,
    /// Direction in the xz plane, radians: 0 is +x, a quarter turn is +z.
    pub direction: f64,
}

impl Swell {
    /// The shipped defaults from `data/raft.toml`.
    pub fn default_from_data() -> Result<Swell, String> {
        Swell::parse(DEFAULT_RAFT)
    }

    pub fn parse(text: &str) -> Result<Swell, String> {
        let root: toml::Value = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let get = |key: &str| -> Result<f64, String> {
            let n = match root.get("swell").and_then(|t| t.get(key)) {
                Some(toml::Value::Float(f)) => *f,
                Some(toml::Value::Integer(i)) => *i as f64,
                Some(_) => return Err(format!("swell.{key} must be a number")),
                None => return Err(format!("missing swell.{key}")),
            };
            if !n.is_finite() {
                return Err(format!("swell.{key} must be finite"));
            }
            Ok(n)
        };
        let period = get("period_ticks")?;
        if period < 1.0 || period.fract() != 0.0 {
            return Err("swell.period_ticks must be a whole number of at least 1".into());
        }
        Ok(Swell {
            sway_amplitude: get("sway_amplitude")?,
            tilt_amplitude: get("tilt_amplitude")?,
            period_ticks: period as u64,
            direction: get("direction_degrees")?.to_radians(),
        })
    }

    /// The raft's motion for tick `tick`: the step from the sway position at
    /// `tick` to the one at `tick + 1`, and the deck tilt at `tick`. Tilt
    /// leads the sway by a quarter cycle and is laid out as `tilt[0]` about
    /// x (pushing towards +z) and `tilt[1]` about z (pushing towards +x).
    pub fn frame(&self, tick: u64) -> RaftFrame {
        let phase = |t: u64| {
            std::f64::consts::TAU * (t % self.period_ticks) as f64 / self.period_ticks as f64
        };
        let (dx, dz) = (self.direction.cos(), self.direction.sin());
        let sway = |t: u64| self.sway_amplitude * phase(t).sin();
        let step = sway(tick + 1) - sway(tick);
        let tilt = self.tilt_amplitude * phase(tick).cos();
        RaftFrame {
            displacement: [dx * step, 0.0, dz * step],
            yaw_delta: 0.0,
            tilt: [dz * tilt, dx * tilt],
        }
    }
}

/// Driving tunables from `[drive]` in `data/raft.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    /// Blocks per tick with forward held fully.
    pub speed: f64,
    /// Radians per tick with the turn axis held fully.
    pub turn_rate: f64,
}

impl Drive {
    /// The shipped defaults from `data/raft.toml`.
    pub fn default_from_data() -> Result<Drive, String> {
        Drive::parse(DEFAULT_RAFT)
    }

    pub fn parse(text: &str) -> Result<Drive, String> {
        let root: toml::Value = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let get = |key: &str| -> Result<f64, String> {
            let n = match root.get("drive").and_then(|t| t.get(key)) {
                Some(toml::Value::Float(f)) => *f,
                Some(toml::Value::Integer(i)) => *i as f64,
                Some(_) => return Err(format!("drive.{key} must be a number")),
                None => return Err(format!("missing drive.{key}")),
            };
            if !n.is_finite() || n <= 0.0 {
                return Err(format!("drive.{key} must be a positive number"));
            }
            Ok(n)
        };
        Ok(Drive {
            speed: get("speed")?,
            turn_rate: get("turn_rate")?,
        })
    }

    /// One tick of driving with `forward` and `turn` in -1..1 (clamped), from
    /// continuous `heading` (see `Raft::heading`). Heading 0 faces -z and
    /// positive turns towards +x, like `RaftFrame::yaw_delta`.
    pub fn frame(&self, forward: f64, turn: f64, heading: f64) -> RaftFrame {
        let step = forward.clamp(-1.0, 1.0) * self.speed;
        RaftFrame {
            displacement: [step * heading.sin(), 0.0, -step * heading.cos()],
            yaw_delta: turn.clamp(-1.0, 1.0) * self.turn_rate,
            tilt: [0.0, 0.0],
        }
    }
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

/// Merge per-region raft masks into one sorted list without duplicates, so
/// the result does not depend on the order the regions are given in.
pub fn merge_masks<'a>(masks: impl IntoIterator<Item = &'a [[i32; 3]]>) -> Vec<[i32; 3]> {
    let mut cells: Vec<[i32; 3]> = masks.into_iter().flatten().copied().collect();
    cells.sort();
    cells.dedup();
    cells
}

#[derive(Clone, Debug)]
pub struct Raft {
    /// Mask cells at the rest pose.
    cells: Vec<[i32; 3]>,
    rest: HashSet<[i32; 3]>,
    /// The helm: one of `cells`, where the player steers from.
    helm: Option<[i32; 3]>,
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
            helm: None,
            centre: [range(0), range(2)],
            offset: [0.0; 3],
            yaw: 0.0,
            turns: 0,
        }
    }

    /// A raft with a helm at `helm`, which must be one of `cells`; any other
    /// cell leaves the raft without a helm.
    pub fn with_helm(cells: &[[i32; 3]], helm: [i32; 3]) -> Raft {
        let mut raft = Raft::new(cells);
        raft.helm = raft.rest.contains(&helm).then_some(helm);
        raft
    }

    /// Build the raft from the per-region raft masks (world-layout 4.5): the
    /// union of every region's cells, so a raft whose mask straddles a region
    /// border is the same raft as one built from a region covering both.
    pub fn from_region_masks<'a>(masks: impl IntoIterator<Item = &'a [[i32; 3]]>) -> Raft {
        Raft::new(&merge_masks(masks))
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

    /// Continuous yaw in radians: the quarter turns applied plus the yaw not
    /// yet turned into one. Collision still uses `quarter_turns`.
    pub fn heading(&self) -> f64 {
        self.turns as f64 * FRAC_PI_2 + self.yaw
    }

    /// The collision box of every cell at the current pose.
    pub fn boxes(&self) -> impl Iterator<Item = Aabb> + '_ {
        self.cells.iter().map(|c| self.cell_box(c))
    }

    fn cell_box(&self, c: &[i32; 3]) -> Aabb {
        let (x, y, z) = (c[0] as f64, c[1] as f64, c[2] as f64);
        let a = rotate([x, y, z], self.centre, self.turns);
        let b = rotate([x + 1.0, y + 1.0, z + 1.0], self.centre, self.turns);
        let lo = |i: usize| a[i].min(b[i]) + self.offset[i];
        let hi = |i: usize| a[i].max(b[i]) + self.offset[i];
        Aabb::new([lo(0), lo(1), lo(2)], [hi(0), hi(1), hi(2)])
    }

    /// The helm cell at the rest pose, if the raft has one.
    pub fn helm(&self) -> Option<[i32; 3]> {
        self.helm
    }

    /// Whether a body occupying `bb` stands on the helm cell: the same test
    /// as `supports`, against the helm cell's box alone.
    pub fn on_helm(&self, bb: &Aabb) -> bool {
        let probe = bb.offset([0.0, -SUPPORT_EPS, 0.0]);
        self.helm
            .is_some_and(|h| self.cell_box(&h).intersects(&probe))
    }

    /// Whether a raft box holds up a body occupying `bb`: the box under its
    /// feet is touching a raft top.
    pub fn supports(&self, bb: &Aabb) -> bool {
        let probe = bb.offset([0.0, -SUPPORT_EPS, 0.0]);
        self.boxes().any(|b| b.intersects(&probe))
    }

    /// Move the raft by one frame. Each rider supported by the raft before it
    /// moves is carried by the same translation and quarter turns. A body that
    /// is not carried but ends up overlapping the raft is shoved by the
    /// translation.
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
            } else if self.boxes().any(|b| b.intersects(&p.aabb(physics))) {
                // The moving raft swept into a body it was not carrying, such
                // as a swimmer beside it: shove it along rather than leave it
                // inside the raft.
                for i in 0..3 {
                    p.pos[i] += frame.displacement[i];
                }
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

    fn selection_boxes_in(&self, query: &Aabb, out: &mut Vec<SelectionBox>) {
        block_selection_boxes_in(self, query, out);
        out.extend(self.raft.boxes().filter_map(|aabb| {
            let touches =
                (0..3).all(|i| aabb.min[i] <= query.max[i] && aabb.max[i] >= query.min[i]);
            // The cell the box mostly occupies now.
            let pos = [0, 1, 2].map(|i| ((aabb.min[i] + aabb.max[i]) / 2.0).floor() as i32);
            touches.then_some(SelectionBox { pos, aabb })
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::data_error::test_support::assert_err_contains;
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

    /// A body standing in the middle of the top of cell `c` at the current pose.
    fn stand_on(c: [i32; 3], r: &Raft) -> Aabb {
        let b = r.cell_box(&c);
        let (x, z) = ((b.min[0] + b.max[0]) / 2.0, (b.min[2] + b.max[2]) / 2.0);
        Aabb::from_size([x - 0.3, b.max[1], z - 0.3], [0.6, 1.8, 0.6])
    }

    fn check_helm(raft: &Raft, helm: [i32; 3]) {
        for x in 0..4 {
            for z in 0..4 {
                let c = [x, 64, z];
                assert_eq!(
                    raft.on_helm(&stand_on(c, raft)),
                    c == helm,
                    "{c:?} turns {}",
                    raft.quarter_turns()
                );
            }
        }
    }

    #[test]
    fn helm_is_only_under_the_helm_cell_through_moves_and_turns() {
        let (_, plain) = deck();
        assert_eq!(plain.helm(), None);
        assert!(!plain.on_helm(&stand_on([1, 64, 2], &plain)));
        let cells: Vec<[i32; 3]> = plain.cells.clone();
        let helm = [1, 64, 2];
        let mut raft = Raft::with_helm(&cells, helm);
        assert_eq!(raft.helm(), Some(helm));
        check_helm(&raft, helm);
        raft.advance(&slide(0.75), &mut [], &phys());
        check_helm(&raft, helm);
        let quarter = RaftFrame {
            yaw_delta: FRAC_PI_2,
            ..RaftFrame::default()
        };
        raft.advance(&quarter, &mut [], &phys());
        assert_eq!(raft.quarter_turns(), 1);
        check_helm(&raft, helm);
        // A body in the air above the helm is not on it.
        let high = stand_on(helm, &raft).offset([0.0, 2.0, 0.0]);
        assert!(!raft.on_helm(&high));
        // A cell outside the mask is no helm.
        assert_eq!(Raft::with_helm(&cells, [9, 64, 9]).helm(), None);
    }

    #[test]
    fn swell_is_periodic_and_sways_back_to_rest() {
        let sw = Swell::default_from_data().unwrap();
        let mut sum = [0.0; 3];
        for t in 0..sw.period_ticks {
            let f = sw.frame(t);
            assert_eq!(f, sw.frame(t + sw.period_ticks));
            for i in 0..3 {
                sum[i] += f.displacement[i];
            }
        }
        assert!(sum.iter().all(|v| v.abs() < 1e-9), "{sum:?}");
        let peak = (0..sw.period_ticks)
            .map(|t| sw.frame(t).tilt[0].hypot(sw.frame(t).tilt[1]))
            .fold(0.0, f64::max);
        assert!((peak - sw.tilt_amplitude).abs() < 0.05, "{peak}");
    }

    #[test]
    fn drive_loads_and_rejects_bad_data() {
        let d = Drive::default_from_data().unwrap();
        assert!(d.speed > 0.0 && d.turn_rate > 0.0);
        assert_err_contains(Drive::parse("[drive]\n"), "missing drive.");
        assert_err_contains(
            Drive::parse("[drive]\nspeed=\"x\"\nturn_rate=0.1\n"),
            "drive.speed must be a number",
        );
        assert_err_contains(
            Drive::parse("[drive]\nspeed=0.1\nturn_rate=0.0\n"),
            "drive.turn_rate must be",
        );
    }

    #[test]
    fn drive_frame_is_zero_linear_clamped_and_along_heading() {
        let d = Drive {
            speed: 0.2,
            turn_rate: 0.05,
        };
        assert_eq!(d.frame(0.0, 0.0, 1.3), RaftFrame::default());
        let f = d.frame(0.5, -0.5, 0.0);
        assert!((f.displacement[2] + 0.1).abs() < 1e-12 && f.displacement[0].abs() < 1e-12);
        assert!((f.yaw_delta + 0.025).abs() < 1e-12);
        assert_eq!(d.frame(7.0, 7.0, 0.4), d.frame(1.0, 1.0, 0.4));
        // A quarter turn faces +x; backwards reverses.
        let f = d.frame(1.0, 0.0, FRAC_PI_2);
        assert!((f.displacement[0] - 0.2).abs() < 1e-12 && f.displacement[2].abs() < 1e-12);
        let b = d.frame(-1.0, 0.0, FRAC_PI_2);
        assert!((b.displacement[0] + 0.2).abs() < 1e-12);
    }

    #[test]
    fn heading_is_continuous_across_quarter_turn_boundary() {
        let mut raft = Raft::new(&[[0, 64, 0]]);
        let step = RaftFrame {
            yaw_delta: FRAC_PI_2 / 10.0,
            ..RaftFrame::default()
        };
        let mut prev = raft.heading();
        for i in 1..=25 {
            raft.advance(&step, &mut [], &phys());
            let h = raft.heading();
            assert!((h - prev - step.yaw_delta).abs() < 1e-9, "step {i}");
            prev = h;
        }
        assert_eq!(raft.quarter_turns(), 3);
        assert!((prev - 2.5 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn swell_parse_rejects_bad_data() {
        assert_err_contains(Swell::parse("[swell]\n"), "missing swell.");
        let bad = "[swell]\nsway_amplitude=0.1\ntilt_amplitude=1.0\nperiod_ticks=0\ndirection_degrees=0\n";
        assert_err_contains(Swell::parse(bad), "swell.period_ticks must be");
    }

    #[test]
    fn raft_shoves_a_swimmer_it_sweeps_into() {
        let (_, mut raft) = deck();
        let p = phys();
        // Just beside the deck's +x edge (x = 4), inside the cells' height.
        let mut pl = Player::new([4.4, 64.0, 2.0], &p);
        let before = pl.pos;
        raft.advance(&slide(0.5), &mut [&mut pl], &p);
        assert!((pl.pos[0] - before[0] - 0.5).abs() < 1e-12, "{pl:?}");
        // Nowhere near the raft: left alone.
        let mut far = Player::new([20.0, 64.0, 2.0], &p);
        raft.advance(&slide(0.5), &mut [&mut far], &p);
        assert_eq!(far.pos, [20.0, 64.0, 2.0]);
    }
}
