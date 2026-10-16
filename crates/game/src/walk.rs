//! The walking player as the client drives it: a `Player` stepped at 20 Hz
//! over loaded terrain regions, plus the eye pose between ticks.
//! See docs/design/basic-gameplay.md sections 1 (rule 1) and 3.

use crate::collision::{CollisionWorld, RegionWorld};
use crate::input::TickInput;
use crate::physics::{PhysicsError, PlayerPhysics};
use crate::player::Player;
use terrain::{Block, Region, HEIGHT};

/// Collision over several generated regions; the first region that contains a
/// column answers, and air is returned outside all of them. Regions generated
/// from one seed agree where they overlap.
#[derive(Default)]
pub struct RegionSet {
    regions: Vec<Region>,
}

impl RegionSet {
    pub fn push(&mut self, region: Region) {
        self.regions.push(region);
    }

    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

impl CollisionWorld for RegionSet {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        if y < 0 || y >= HEIGHT as i32 {
            return Block::Air;
        }
        for r in &self.regions {
            let (lx, lz) = (x as i64 - r.x0 as i64, z as i64 - r.z0 as i64);
            if lx >= 0 && lz >= 0 && lx < r.width as i64 && lz < r.depth as i64 {
                return RegionWorld::new(r).block_at(x, y, z);
            }
        }
        Block::Air
    }
}

/// The player plus the eye positions of the last two ticks.
pub struct Walker {
    player: Player,
    physics: PlayerPhysics,
    world: RegionSet,
    prev_eye: [f64; 3],
    cur_eye: [f64; 3],
}

impl Walker {
    /// A player with its feet at `feet`, standing on nothing until regions
    /// are added.
    pub fn new(feet: [f64; 3]) -> Result<Walker, PhysicsError> {
        let physics = PlayerPhysics::default_from_data()?;
        let player = Player::new(feet, &physics);
        let eye = [feet[0], feet[1] + physics.eye_offset(false) as f64, feet[2]];
        Ok(Walker {
            player,
            physics,
            world: RegionSet::default(),
            prev_eye: eye,
            cur_eye: eye,
        })
    }

    pub fn add_region(&mut self, region: Region) {
        self.world.push(region);
    }

    pub fn has_world(&self) -> bool {
        !self.world.is_empty()
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    /// One 20 Hz step.
    pub fn tick(&mut self, input: &TickInput) {
        self.player.tick(input, &self.physics, &self.world);
        self.prev_eye = self.cur_eye;
        let up = self.physics.eye_offset(input.sneak) as f64;
        self.cur_eye = [
            self.player.pos[0],
            self.player.pos[1] + up,
            self.player.pos[2],
        ];
    }

    /// The eye between the last two ticks, `alpha` in 0..1.
    pub fn eye(&self, alpha: f64) -> [f64; 3] {
        let a = alpha.clamp(0.0, 1.0);
        let l = |i: usize| self.prev_eye[i] + (self.cur_eye[i] - self.prev_eye[i]) * a;
        [l(0), l(1), l(2)]
    }

    /// Put the player at `feet` with no velocity, e.g. when leaving fly mode.
    pub fn teleport(&mut self, feet: [f64; 3]) {
        self.player = Player::new(feet, &self.physics);
        let eye = [
            feet[0],
            feet[1] + self.physics.eye_offset(false) as f64,
            feet[2],
        ];
        self.prev_eye = eye;
        self.cur_eye = eye;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(x0: i32, z0: i32, n: u32, top: u32) -> Region {
        let mut blocks = vec![0u8; n as usize * n as usize * HEIGHT];
        let r = Region {
            blocks: Vec::new(),
            width: n,
            depth: n,
            height: HEIGHT as u32,
            x0,
            z0,
        };
        for z in 0..n {
            for x in 0..n {
                for y in 0..=top {
                    blocks[r.index(x, y, z)] = Block::Stone as u8;
                }
            }
        }
        Region { blocks, ..r }
    }

    fn settled() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.add_region(flat(-8, -8, 16, 3));
        for _ in 0..40 {
            w.tick(&TickInput::default());
        }
        w
    }

    #[test]
    fn eye_is_above_the_feet_and_lower_when_sneaking() {
        let mut w = settled();
        assert!(w.player().on_ground);
        let feet = w.player().pos[1];
        assert_eq!(feet, 4.0);
        assert!((w.eye(1.0)[1] - (feet + 1.62)).abs() < 1e-6);
        w.tick(&TickInput {
            sneak: true,
            ..TickInput::default()
        });
        assert!((w.eye(1.0)[1] - (feet + 1.62 - 0.08)).abs() < 1e-6);
    }

    #[test]
    fn eye_interpolates_between_ticks() {
        let mut w = settled();
        let before = w.eye(1.0);
        w.tick(&TickInput {
            move_forward: 1.0,
            ..TickInput::default()
        });
        let after = w.eye(1.0);
        assert!(after[2] < before[2]);
        assert_eq!(w.eye(0.0), before);
        let mid = w.eye(0.5);
        assert!((mid[2] - (before[2] + after[2]) / 2.0).abs() < 1e-12);
    }

    #[test]
    fn region_set_answers_across_regions_and_is_air_outside() {
        let mut set = RegionSet::default();
        set.push(flat(0, 0, 4, 1));
        set.push(flat(4, 0, 4, 2));
        assert_eq!(set.block_at(1, 1, 1), Block::Stone);
        assert_eq!(set.block_at(5, 2, 1), Block::Stone);
        assert_eq!(set.block_at(1, 2, 1), Block::Air);
        assert_eq!(set.block_at(9, 0, 0), Block::Air);
        assert_eq!(set.block_at(1, -1, 1), Block::Air);
    }

    #[test]
    fn sneaking_stops_at_the_region_edge() {
        let mut w = settled();
        // Walk toward -z (yaw 0) into the region edge at z = -8 while sneaking.
        let sneak = TickInput {
            move_forward: 1.0,
            sneak: true,
            ..TickInput::default()
        };
        for _ in 0..400 {
            w.tick(&sneak);
        }
        assert!(w.player().on_ground && w.player().pos[1] == 4.0);
        assert!(w.player().pos[2] > -8.0 - 0.31);
    }
}
