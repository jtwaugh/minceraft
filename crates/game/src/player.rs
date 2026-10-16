//! Player state and the land-movement tick.
//! See docs/design/basic-gameplay.md section 4.2 (steps 1 to 7) and 4.1.
//!
//! Fall distance and fall damage follow section 4.6. Not handled here yet:
//! step-up, sneak edge protection, water movement and the raft.

use crate::collision::{Aabb, CollisionWorld};
use crate::input::TickInput;
use crate::movement::move_box;
use crate::physics::PlayerPhysics;

/// The player's simulated state. `pos` is the centre of the feet.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub on_ground: bool,
    pub health: u32,
    /// Blocks fallen since last on the ground or in water.
    pub fall_distance: f64,
}

impl Player {
    pub fn new(pos: [f64; 3], physics: &PlayerPhysics) -> Player {
        Player {
            pos,
            vel: [0.0; 3],
            on_ground: false,
            health: physics.max_health,
            fall_distance: 0.0,
        }
    }

    /// The bounding box at the current position.
    pub fn aabb(&self, physics: &PlayerPhysics) -> Aabb {
        let (w, h) = (physics.width as f64, physics.height as f64);
        Aabb::from_size(
            [self.pos[0] - w / 2.0, self.pos[1], self.pos[2] - w / 2.0],
            [w, h, w],
        )
    }

    /// Section 4.6: accumulate fall distance while airborne; on landing apply
    /// `ceil(fall_distance - threshold)` damage if positive. Water resets the
    /// distance without damage.
    fn update_fall(&mut self, dy: f64, physics: &PlayerPhysics, world: &impl CollisionWorld) {
        if world.body_in_water(&self.aabb(physics)) {
            self.fall_distance = 0.0;
            return;
        }
        if dy < 0.0 {
            self.fall_distance -= dy;
        }
        if self.on_ground {
            let damage = (self.fall_distance - physics.fall_damage_threshold as f64).ceil();
            if damage > 0.0 {
                self.health = self.health.saturating_sub(damage as u32);
            }
            self.fall_distance = 0.0;
        }
    }

    /// Advance one sim tick, following 4.2 steps 1 to 7 in order.
    pub fn tick(
        &mut self,
        input: &TickInput,
        physics: &PlayerPhysics,
        world: &impl CollisionWorld,
    ) {
        // 1. Read input and scale it. Over-long raw input (diagonals) is
        // normalised first so no direction is faster than straight ahead.
        let (mut fwd, mut strafe) = (input.move_forward as f64, input.move_strafe as f64);
        let len = fwd.hypot(strafe);
        if len > 1.0 {
            fwd /= len;
            strafe /= len;
        }
        let mut scale = physics.input_scale as f64;
        if input.sneak {
            scale *= physics.sneak_scale as f64;
        }
        fwd *= scale;
        strafe *= scale;

        // 2. Choose acceleration.
        let f = (physics.slipperiness * physics.friction) as f64;
        let accel = if self.on_ground {
            physics.ground_accel as f64 * (physics.ground_accel_ref as f64 / f.powi(3))
        } else {
            physics.air_accel as f64
        };

        // 3. Add it to horizontal velocity along the facing direction.
        let (sin, cos) = (input.yaw as f64).sin_cos();
        self.vel[0] += (fwd * sin + strafe * cos) * accel;
        self.vel[2] += (strafe * sin - fwd * cos) * accel;

        // 4. Jump.
        if input.jump && self.on_ground {
            self.vel[1] = physics.jump_velocity as f64;
        }

        // 5. Move with collision. A blocked axis loses its velocity.
        let m = move_box(world, &self.aabb(physics), self.vel);
        let half = physics.width as f64 / 2.0;
        self.pos = [m.aabb.min[0] + half, m.aabb.min[1], m.aabb.min[2] + half];
        for i in 0..3 {
            if m.applied[i] != self.vel[i] {
                self.vel[i] = 0.0;
            }
        }
        self.on_ground = m.on_ground;
        self.update_fall(m.applied[1], physics, world);

        // 6. Gravity.
        self.vel[1] = (self.vel[1] - physics.gravity as f64) * physics.drag as f64;

        // 7. Horizontal friction.
        let friction = if self.on_ground {
            f
        } else {
            physics.friction as f64
        };
        self.vel[0] *= friction;
        self.vel[2] *= friction;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use terrain::Block;

    fn phys() -> PlayerPhysics {
        PlayerPhysics::default_from_data().unwrap()
    }

    /// Stone floor with its top surface at y = 1.
    fn floor() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -40..40 {
            for z in -40..40 {
                w.set(x, 0, z, Block::Stone);
            }
        }
        w
    }

    /// A player resting on the floor.
    fn grounded(p: &PlayerPhysics, w: &MemoryWorld) -> Player {
        let mut pl = Player::new([0.5, 1.5, 0.5], p);
        for _ in 0..10 {
            pl.tick(&TickInput::default(), p, w);
        }
        assert!(pl.on_ground && pl.pos[1] == 1.0);
        pl
    }

    fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
        (a[0] - b[0]).hypot(a[2] - b[2])
    }

    /// Blocks per second covered over 100 ticks once velocity has settled.
    fn speed(input: TickInput) -> f64 {
        let (p, w) = (phys(), floor());
        let mut pl = grounded(&p, &w);
        for _ in 0..50 {
            pl.tick(&input, &p, &w);
        }
        let start = pl.pos;
        for _ in 0..100 {
            pl.tick(&input, &p, &w);
        }
        dist(start, pl.pos) / 5.0
    }

    fn walk() -> TickInput {
        TickInput {
            move_forward: 1.0,
            ..TickInput::default()
        }
    }

    fn close(got: f64, want: f64) {
        assert!((got - want).abs() <= want * 0.01, "got {got}, want {want}");
    }

    #[test]
    fn walking_and_sneaking_speed() {
        close(speed(walk()), 4.3);
        close(
            speed(TickInput {
                sneak: true,
                ..walk()
            }),
            1.3,
        );
    }

    #[test]
    fn diagonal_is_no_faster_than_straight() {
        let straight = speed(walk());
        let diag = speed(TickInput {
            move_strafe: 1.0,
            ..walk()
        });
        assert!(diag <= straight + 1e-9, "{diag} > {straight}");
        close(diag, straight);
        // Facing changes direction, not speed.
        let turned = speed(TickInput { yaw: 1.0, ..walk() });
        close(turned, straight);
    }

    #[test]
    fn forward_at_yaw_zero_is_negative_z() {
        let (p, w) = (phys(), floor());
        let mut pl = grounded(&p, &w);
        pl.tick(&walk(), &p, &w);
        assert!(pl.pos[2] < 0.5 && (pl.pos[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn jump_apex_and_airtime() {
        let (p, w) = (phys(), floor());
        let mut pl = grounded(&p, &w);
        let jump = TickInput {
            jump: true,
            ..TickInput::default()
        };
        pl.tick(&jump, &p, &w);
        let (mut apex, mut air) = (pl.pos[1] - 1.0, 1);
        let idle = TickInput::default();
        while !pl.on_ground {
            pl.tick(&idle, &p, &w);
            apex = apex.max(pl.pos[1] - 1.0);
            air += 1;
            assert!(air < 100);
        }
        close(apex, 1.25);
        assert_eq!(air, 12);
    }

    #[test]
    fn jump_only_works_on_ground() {
        let (p, w) = (phys(), floor());
        let jump = TickInput {
            jump: true,
            ..TickInput::default()
        };
        // In the air: holding jump does nothing, the player just falls.
        let mut pl = Player::new([0.5, 5.0, 0.5], &p);
        pl.tick(&jump, &p, &w);
        assert!(!pl.on_ground && pl.vel[1] < 0.0 && pl.pos[1] == 5.0);
        // On ground: it jumps.
        let mut pl = grounded(&p, &w);
        pl.tick(&jump, &p, &w);
        assert!(pl.pos[1] > 1.3 && !pl.on_ground);
    }

    /// Drop the player from `h` blocks above the floor and settle.
    fn drop_from(h: f64, w: &impl CollisionWorld) -> Player {
        let p = phys();
        let mut pl = Player::new([0.5, 1.0 + h, 0.5], &p);
        for _ in 0..200 {
            pl.tick(&TickInput::default(), &p, w);
        }
        assert!(pl.on_ground);
        pl
    }

    #[test]
    fn short_falls_do_no_damage() {
        let w = floor();
        for h in [0.0, 1.0, 3.0] {
            let pl = drop_from(h, &w);
            assert_eq!(pl.health, phys().max_health, "h={h}");
        }
    }

    #[test]
    fn long_falls_deal_ceil_of_excess() {
        let w = floor();
        let max = phys().max_health;
        assert_eq!(drop_from(3.5, &w).health, max - 1);
        assert_eq!(drop_from(4.0, &w).health, max - 1);
        assert_eq!(drop_from(6.2, &w).health, max - 4);
        assert_eq!(drop_from(100.0, &w).health, 0);
    }

    #[test]
    fn fall_distance_resets_after_landing() {
        let (p, w) = (phys(), floor());
        let mut pl = drop_from(8.0, &w);
        assert_eq!(pl.fall_distance, 0.0);
        let health = pl.health;
        for _ in 0..20 {
            pl.tick(&TickInput::default(), &p, &w);
        }
        assert_eq!(pl.health, health);
        // A second small drop only counts its own distance.
        let pl2 = drop_from(2.0, &w);
        assert_eq!(pl2.health, p.max_health);
    }

    /// A floor with water everywhere above y = 1 up to `top`.
    struct Wet(MemoryWorld);
    impl CollisionWorld for Wet {
        fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
            self.0.block_at(x, y, z)
        }
        fn body_in_water(&self, bb: &Aabb) -> bool {
            bb.min[1] < 4.0
        }
    }

    #[test]
    fn water_resets_distance_and_prevents_damage() {
        let p = phys();
        let w = Wet(floor());
        let mut pl = Player::new([0.5, 20.0, 0.5], &p);
        let mut max_fd: f64 = 0.0;
        for _ in 0..200 {
            pl.tick(&TickInput::default(), &p, &w);
            max_fd = max_fd.max(pl.fall_distance);
        }
        assert!(max_fd > 10.0, "fell through air first");
        assert!(pl.on_ground);
        assert_eq!(pl.health, p.max_health);
        assert_eq!(pl.fall_distance, 0.0);
    }

    #[test]
    fn starts_with_max_health() {
        let p = phys();
        assert_eq!(Player::new([0.0; 3], &p).health, p.max_health);
    }

    #[test]
    fn same_inputs_give_same_state() {
        let p = phys();
        let mut w = floor();
        w.set(3, 1, -2, Block::Stone);
        let run = || {
            let mut pl = Player::new([0.5, 3.0, 0.5], &p);
            for i in 0..300u32 {
                let input = TickInput {
                    move_forward: ((i % 7) as f32 - 3.0) / 3.0,
                    move_strafe: ((i % 5) as f32 - 2.0) / 2.0,
                    yaw: i as f32 * 0.05,
                    jump: i % 11 < 3,
                    sneak: i % 13 == 0,
                    ..TickInput::default()
                };
                pl.tick(&input, &p, &w);
            }
            pl
        };
        assert_eq!(run(), run());
    }
}
