//! Player state and the land-movement tick.
//! See docs/design/basic-gameplay.md section 4.2 (steps 1 to 7) and 4.1.
//!
//! Sneak edge protection follows 4.5 and fall distance and fall damage follow
//! 4.6. Water movement and climbing out of water follow 4.3. Not handled here
//! yet: step-up and the raft.

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
    /// Sprinting (docs/design/sprinting.md section 2). Started by
    /// `input.sprint`, ended in `tick`.
    pub sprinting: bool,
    pub health: u32,
    /// Blocks fallen since last on the ground or in water.
    pub fall_distance: f64,
    /// The fall distance of a landing on the ground made this tick (before
    /// it was reset); cleared at the start of the next tick.
    pub last_landing: Option<f64>,
    /// Set on the tick health reaches 0 and kept after. While set, `tick`
    /// ignores input.
    pub dead: bool,
    /// True only on the tick on which the player died; cleared at the start
    /// of the next tick.
    pub died_this_tick: bool,
}

impl Player {
    pub fn new(pos: [f64; 3], physics: &PlayerPhysics) -> Player {
        Player {
            pos,
            vel: [0.0; 3],
            on_ground: false,
            sprinting: false,
            health: physics.max_health,
            fall_distance: 0.0,
            last_landing: None,
            dead: false,
            died_this_tick: false,
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
            if self.fall_distance > 0.0 {
                self.last_landing = Some(self.fall_distance);
            }
            let damage = (self.fall_distance - physics.fall_damage_threshold as f64).ceil();
            if damage > 0.0 {
                self.health = self.health.saturating_sub(damage as u32);
            }
            self.fall_distance = 0.0;
        }
    }

    /// 4.5: shrink the horizontal part of `motion` in `sneak_edge_step` steps,
    /// x then z, then both together (so a corner can't be rounded diagonally),
    /// until the moved box still has ground within a step height below it.
    fn shrink_at_ledge(
        &self,
        motion: &mut [f64; 3],
        physics: &PlayerPhysics,
        world: &impl CollisionWorld,
    ) {
        let bb = self.aabb(physics);
        let drop = physics.step_height as f64;
        let step = physics.sneak_edge_step as f64;
        let mut boxes = Vec::new();
        let mut supported = |dx: f64, dz: f64| {
            boxes.clear();
            let probe = bb.offset([dx, -drop, dz]);
            world.boxes_in(&probe, &mut boxes);
            !boxes.is_empty()
        };
        // Move `d` one step towards zero, snapping to zero when it crosses.
        let shrink = |d: f64| {
            if d.abs() <= step {
                0.0
            } else {
                d - step * d.signum()
            }
        };
        let (mut dx, mut dz) = (motion[0], motion[2]);
        while dx != 0.0 && !supported(dx, 0.0) {
            dx = shrink(dx);
        }
        while dz != 0.0 && !supported(0.0, dz) {
            dz = shrink(dz);
        }
        while dx != 0.0 && dz != 0.0 && !supported(dx, dz) {
            dx = shrink(dx);
            dz = shrink(dz);
        }
        motion[0] = dx;
        motion[2] = dz;
    }

    /// Advance one sim tick, following 4.2 steps 1 to 7 in order.
    pub fn tick(
        &mut self,
        input: &TickInput,
        physics: &PlayerPhysics,
        world: &impl CollisionWorld,
    ) {
        self.last_landing = None;
        self.died_this_tick = false;
        // A dead player is pushed by gravity and collision only.
        let neutral = TickInput::default();
        let input = if self.dead { &neutral } else { input };
        // Sprint state: a double-tap starts it only while walking forward and
        // not sneaking or in water; releasing forward, sneaking or entering
        // water ends it.
        let in_water = world.body_in_water(&self.aabb(physics));
        if input.move_forward <= 0.0 || input.sneak || in_water {
            self.sprinting = false;
        } else if input.sprint {
            self.sprinting = true;
        }
        // 1. Read input and scale it. Over-long raw input (diagonals) is
        // normalised first so no direction is faster than straight ahead.
        let (mut fwd, mut strafe) = input.movement();
        let mut scale = physics.input_scale as f64;
        if input.sneak {
            scale *= physics.sneak_scale as f64;
        }
        if self.sprinting {
            scale *= physics.sprint_scale as f64;
        }
        fwd *= scale;
        strafe *= scale;

        // 2. Choose acceleration.
        let f = (physics.slipperiness * physics.friction) as f64;
        let accel = if in_water {
            physics.water_accel as f64
        } else if self.on_ground {
            physics.ground_accel as f64 * (physics.ground_accel_ref as f64 / f.powi(3))
        } else {
            physics.air_accel as f64
        };

        // 3. Add it to horizontal velocity along the facing direction.
        let (sin, cos) = (input.yaw as f64).sin_cos();
        self.vel[0] += (fwd * sin + strafe * cos) * accel;
        self.vel[2] += (strafe * sin - fwd * cos) * accel;

        // 4. Jump.
        // In water, holding jump swims up instead (4.3).
        if in_water {
            if input.jump {
                self.vel[1] += physics.water_swim_up_impulse as f64;
            }
        } else if input.jump && self.on_ground {
            self.vel[1] = physics.jump_velocity as f64;
            // A sprint-jump pushes along the yaw facing only, whatever keys
            // are held.
            if self.sprinting {
                let boost = physics.sprint_jump_boost as f64;
                self.vel[0] += boost * sin;
                self.vel[2] -= boost * cos;
            }
        }

        // 5. Move with collision. A blocked axis loses its velocity.
        // Sneaking on ground first shrinks the move so it cannot leave a ledge
        // (4.5); a shrunk axis counts as blocked and loses its velocity too.
        let mut motion = self.vel;
        if input.sneak && self.on_ground {
            self.shrink_at_ledge(&mut motion, physics, world);
        }
        let m = move_box(world, &self.aabb(physics), motion);
        let half = physics.width as f64 / 2.0;
        self.pos = [m.aabb.min[0] + half, m.aabb.min[1], m.aabb.min[2] + half];
        let pushed = input.move_forward != 0.0 || input.move_strafe != 0.0;
        let blocked_sideways = m.applied[0] != self.vel[0] || m.applied[2] != self.vel[2];
        // A collision that removes velocity along the facing direction ends
        // the sprint; a strafe-only block does not.
        let lost_along_facing =
            (self.vel[0] - m.applied[0]) * sin - (self.vel[2] - m.applied[2]) * cos;
        if lost_along_facing > 1e-6 && input.move_forward > 0.0 {
            self.sprinting = false;
        }
        for i in 0..3 {
            if m.applied[i] != self.vel[i] {
                self.vel[i] = 0.0;
            }
        }
        self.on_ground = m.on_ground;
        self.update_fall(m.applied[1], physics, world);
        if self.health == 0 && !self.dead {
            self.dead = true;
            self.died_this_tick = true;
        }

        if in_water {
            // 4.3 replaces steps 6 and 7: scale every axis, then sink.
            let k = physics.water_velocity_multiplier as f64;
            for v in &mut self.vel {
                *v *= k;
            }
            self.vel[1] -= physics.water_gravity as f64;
            // Exiting water (4.3): pushing into a block face with the space
            // above the box clear lifts the player out onto the ledge.
            if pushed && blocked_sideways {
                let raised =
                    self.aabb(physics)
                        .offset([0.0, physics.water_climb_out_clearance as f64, 0.0]);
                let mut boxes = Vec::new();
                world.boxes_in(&raised, &mut boxes);
                if boxes.is_empty() {
                    self.vel[1] = physics.water_climb_out_velocity as f64;
                }
            }
            return;
        }

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

    fn sprint_tick() -> TickInput {
        TickInput {
            sprint: true,
            ..walk()
        }
    }

    /// A grounded player who has been walking, then double-tapped.
    fn sprinter(p: &PlayerPhysics, w: &MemoryWorld) -> Player {
        let mut pl = grounded(p, w);
        pl.tick(&sprint_tick(), p, w);
        assert!(pl.sprinting);
        pl
    }

    /// Distance covered over 20 ticks of `input`, from the current state.
    fn run(pl: &mut Player, input: &TickInput, p: &PlayerPhysics, w: &MemoryWorld) -> f64 {
        let start = pl.pos;
        for _ in 0..20 {
            pl.tick(input, p, w);
        }
        dist(start, pl.pos)
    }

    #[test]
    fn sprint_starts_and_is_faster() {
        let (p, w) = (phys(), floor());
        let mut pl = sprinter(&p, &w);
        run(&mut pl, &walk(), &p, &w);
        assert!(pl.sprinting);
        let fast = run(&mut pl, &walk(), &p, &w);
        let mut slow = grounded(&p, &w);
        run(&mut slow, &walk(), &p, &w);
        let normal = run(&mut slow, &walk(), &p, &w);
        close(fast, normal * p.sprint_scale as f64);
    }

    #[test]
    fn releasing_forward_ends_sprint() {
        let (p, w) = (phys(), floor());
        let mut pl = sprinter(&p, &w);
        pl.tick(&TickInput::default(), &p, &w);
        assert!(!pl.sprinting);
        for _ in 0..50 {
            pl.tick(&walk(), &p, &w);
        }
        assert!(!pl.sprinting);
        close(run(&mut pl, &walk(), &p, &w), 4.3);
    }

    #[test]
    fn sneaking_ends_sprint() {
        let (p, w) = (phys(), floor());
        let mut pl = sprinter(&p, &w);
        let sneak = TickInput {
            sneak: true,
            ..walk()
        };
        pl.tick(&sneak, &p, &w);
        assert!(!pl.sprinting);
        for _ in 0..50 {
            pl.tick(&walk(), &p, &w);
        }
        assert!(!pl.sprinting);
        close(run(&mut pl, &walk(), &p, &w), 4.3);
    }

    #[test]
    fn hitting_a_wall_ends_sprint() {
        let (p, mut w) = (phys(), floor());
        for x in -2..3 {
            for y in 1..3 {
                w.set(x, y, -3, Block::Stone);
            }
        }
        let mut pl = sprinter(&p, &w);
        for _ in 0..60 {
            pl.tick(&walk(), &p, &w);
        }
        assert!(!pl.sprinting);
        assert!(pl.pos[2] > -2.9);
    }

    #[test]
    fn sprint_request_needs_forward_and_no_sneak() {
        let (p, w) = (phys(), floor());
        let mut pl = grounded(&p, &w);
        pl.tick(
            &TickInput {
                sprint: true,
                ..TickInput::default()
            },
            &p,
            &w,
        );
        assert!(!pl.sprinting);
        pl.tick(
            &TickInput {
                sneak: true,
                ..sprint_tick()
            },
            &p,
            &w,
        );
        assert!(!pl.sprinting);
    }

    #[test]
    fn sprint_request_in_water_does_not_start() {
        let p = phys();
        let w = Deep(floor());
        let mut pl = Player::new([0.5, 1.0, 0.5], &p);
        pl.tick(&sprint_tick(), &p, &w);
        assert!(!pl.sprinting);
    }

    #[test]
    fn entering_water_ends_sprint_and_needs_new_tap() {
        let (p, w) = (phys(), floor());
        let mut pl = sprinter(&p, &w);
        pl.tick(&walk(), &p, &Deep(floor()));
        assert!(!pl.sprinting);
        for _ in 0..50 {
            pl.tick(&walk(), &p, &w);
        }
        assert!(!pl.sprinting);
        close(run(&mut pl, &walk(), &p, &w), 4.3);
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
        assert!(pl.pos[2] < 0.5 && (pl.pos[0] - 0.5).abs() < 1e-6);
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

    fn jump_tick_velocity(
        pl: &mut Player,
        input: TickInput,
        p: &PlayerPhysics,
        w: &MemoryWorld,
    ) -> [f64; 3] {
        let before = pl.vel;
        pl.tick(&input, p, w);
        [pl.vel[0] - before[0], pl.vel[1], pl.vel[2] - before[2]]
    }

    #[test]
    fn sprint_jump_pushes_along_yaw_only() {
        let (p, w) = (phys(), floor());
        let yaw = 0.7f32;
        let jump = TickInput {
            jump: true,
            yaw,
            ..sprint_tick()
        };
        // Reference: same jump without sprinting-state boost.
        let mut base = sprinter(&p, &w);
        let mut noboost = p.clone();
        noboost.sprint_jump_boost = 0.0;
        let a = jump_tick_velocity(&mut base, jump, &noboost, &w);
        let mut pl = sprinter(&p, &w);
        let b = jump_tick_velocity(&mut pl, jump, &p, &w);
        let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
        // Drag is applied after the push, so compare against the same tick.
        assert!(dx > 0.0 && dz < 0.0);
        assert!((dx / dz + (yaw as f64).tan()).abs() < 1e-6);
        assert!((b[1] - a[1]).abs() < 1e-12);
        // Strafe keys and pitch make no difference to the push.
        let mut pl2 = sprinter(&p, &w);
        let c = jump_tick_velocity(
            &mut pl2,
            TickInput {
                jump: true,
                yaw,
                pitch: 0.9,
                move_strafe: 1.0,
                ..sprint_tick()
            },
            &p,
            &w,
        );
        let mut pl3 = sprinter(&p, &w);
        let mut nb = p.clone();
        nb.sprint_jump_boost = 0.0;
        let d = jump_tick_velocity(
            &mut pl3,
            TickInput {
                jump: true,
                yaw,
                pitch: 0.9,
                move_strafe: 1.0,
                ..sprint_tick()
            },
            &nb,
            &w,
        );
        assert!(((c[0] - d[0]) - dx).abs() < 1e-9 && ((c[2] - d[2]) - dz).abs() < 1e-9);
    }

    #[test]
    fn walking_or_released_or_airborne_jump_has_no_push() {
        let (p, w) = (phys(), floor());
        let mut nb = p.clone();
        nb.sprint_jump_boost = 0.0;
        let jump = TickInput {
            jump: true,
            ..walk()
        };
        let same = |a: &mut Player, b: &mut Player, i: &TickInput| {
            a.tick(i, &p, &w);
            b.tick(i, &nb, &w);
            assert_eq!(a.vel, b.vel);
        };
        // Walking.
        same(&mut grounded(&p, &w), &mut grounded(&nb, &w), &jump);
        // Forward released on the jump tick.
        let release = TickInput {
            jump: true,
            ..TickInput::default()
        };
        same(&mut sprinter(&p, &w), &mut sprinter(&nb, &w), &release);
        // Sprinting but airborne.
        let (mut a, mut b) = (sprinter(&p, &w), sprinter(&nb, &w));
        a.pos[1] += 3.0;
        b.pos[1] += 3.0;
        a.on_ground = false;
        b.on_ground = false;
        same(&mut a, &mut b, &jump);
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

    /// Deep water: everything above y = 1 is water.
    struct Deep(MemoryWorld);
    impl CollisionWorld for Deep {
        fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
            self.0.block_at(x, y, z)
        }
        fn body_in_water(&self, _: &Aabb) -> bool {
            true
        }
    }

    #[test]
    fn water_acceleration_ignores_ground() {
        let p = phys();
        let w = Deep(floor());
        let input = walk();
        let mut grounded = Player::new([0.5, 1.0, 0.5], &p);
        grounded.on_ground = true;
        let mut airborne = Player::new([0.5, 5.0, 0.5], &p);
        grounded.tick(&input, &p, &w);
        airborne.tick(&input, &p, &w);
        let want =
            -(p.input_scale as f64) * p.water_accel as f64 * p.water_velocity_multiplier as f64;
        assert!((grounded.vel[2] - want).abs() < 1e-6, "{:?}", grounded.vel);
        assert!((airborne.vel[2] - want).abs() < 1e-6, "{:?}", airborne.vel);
    }

    #[test]
    fn water_scales_velocity_then_sinks() {
        let p = phys();
        let w = Deep(floor());
        let mut pl = Player::new([0.5, 10.0, 0.5], &p);
        pl.vel = [1.0, -1.0, 0.5];
        pl.tick(&TickInput::default(), &p, &w);
        assert!((pl.vel[0] - 0.8).abs() < 1e-6);
        assert!((pl.vel[2] - 0.4).abs() < 1e-6);
        assert!((pl.vel[1] - (-0.8 - 0.02)).abs() < 1e-6);
    }

    #[test]
    fn sinking_speed_with_no_input() {
        let p = phys();
        let w = Deep(floor());
        let mut pl = Player::new([0.5, 30.0, 0.5], &p);
        for _ in 0..100 {
            pl.tick(&TickInput::default(), &p, &w);
        }
        // Steady state of v = 0.8 v - 0.02 is -0.1.
        assert!((pl.vel[1] + 0.1).abs() < 1e-6, "{}", pl.vel[1]);
        assert!(pl.pos[1] < 30.0);
    }

    #[test]
    fn holding_jump_swims_up_to_steady_rise() {
        let p = phys();
        let w = Deep(floor());
        let jump = TickInput {
            jump: true,
            ..TickInput::default()
        };
        // Start on the bottom: no land jump impulse.
        let mut pl = grounded(&p, &floor());
        pl.tick(&jump, &p, &w);
        assert!(pl.vel[1] < 0.1, "{pl:?}");
        for _ in 0..10 {
            pl.tick(&jump, &p, &w);
        }
        let mut last = pl.pos[1];
        for _ in 0..60 {
            pl.tick(&jump, &p, &w);
            assert!(pl.pos[1] > last);
            last = pl.pos[1];
        }
        // Steady state: v = (v + 0.04) * 0.8 - 0.02 gives 0.06.
        assert!((pl.vel[1] - 0.06).abs() < 1e-6, "{}", pl.vel[1]);
    }

    /// A pool west of x = 1 (water y = 1..5) beside a ledge whose top is y = 3.
    /// With `roof`, a block over the pool at y = 5 leaves less than 0.6 of
    /// clearance above a player floating at y = 2.8.
    fn pool_and_ledge(roof: bool) -> MemoryWorld {
        let mut w = floor();
        for x in -5..1 {
            for y in 1..5 {
                for z in -5..5 {
                    w.set(x, y, z, Block::Water);
                }
            }
        }
        for x in 1..6 {
            for y in 1..3 {
                for z in -5..5 {
                    w.set(x, y, z, Block::Stone);
                }
            }
        }
        if roof {
            w.set(0, 5, 0, Block::Stone);
        }
        w
    }

    /// One tick of the player floating against the ledge, optionally pushing.
    fn float_against_ledge(roof: bool, push: bool) -> Player {
        let (p, w) = (phys(), pool_and_ledge(roof));
        let mut pl = Player::new([0.69, 2.8, 0.5], &p);
        let input = TickInput {
            move_forward: if push { 1.0 } else { 0.0 },
            yaw: std::f32::consts::FRAC_PI_2,
            ..TickInput::default()
        };
        pl.tick(&input, &p, &w);
        pl
    }

    #[test]
    fn pushing_into_ledge_climbs_out() {
        let pl = float_against_ledge(false, true);
        assert_eq!(pl.vel[1], phys().water_climb_out_velocity as f64);
    }

    #[test]
    fn blocked_clearance_prevents_climb_out() {
        let pl = float_against_ledge(true, true);
        assert_ne!(pl.vel[1], phys().water_climb_out_velocity as f64);
        assert!(pl.vel[1] < 0.0);
    }

    #[test]
    fn not_pushing_never_climbs_out() {
        let pl = float_against_ledge(false, false);
        assert!(pl.vel[1] < 0.0);
    }

    #[test]
    fn starts_with_max_health() {
        let p = phys();
        assert_eq!(Player::new([0.0; 3], &p).health, p.max_health);
    }

    #[test]
    fn fatal_fall_sets_dead_on_that_tick_only() {
        let (p, w) = (phys(), floor());
        assert!(!Player::new([0.5, 5.0, 0.5], &p).dead);
        let mut pl = Player::new([0.5, 101.0, 0.5], &p);
        let mut deaths = 0;
        for _ in 0..200 {
            let was_on_ground = pl.on_ground;
            pl.tick(&TickInput::default(), &p, &w);
            if pl.died_this_tick {
                deaths += 1;
                assert!(pl.dead && pl.health == 0 && pl.on_ground && !was_on_ground);
            } else if !pl.dead {
                assert!(pl.health > 0);
            }
        }
        assert_eq!(deaths, 1);
        assert!(pl.dead && !pl.died_this_tick);
    }

    #[test]
    fn survivable_fall_does_not_kill() {
        let pl = drop_from(6.2, &floor());
        assert!(!pl.dead && !pl.died_this_tick);
    }

    #[test]
    fn dead_player_ignores_input_but_still_falls() {
        let (p, w) = (phys(), floor());
        let all = TickInput {
            move_forward: 1.0,
            move_strafe: 1.0,
            jump: true,
            sneak: true,
            ..TickInput::default()
        };
        // On the ground: no input-driven motion, no jump.
        let mut pl = grounded(&p, &w);
        pl.dead = true;
        let before = pl.clone();
        for _ in 0..20 {
            pl.tick(&all, &p, &w);
        }
        assert_eq!((pl.pos, pl.vel), (before.pos, before.vel));
        // In the air: gravity applies, horizontal velocity only decays.
        let mut pl = Player::new([0.5, 10.0, 0.5], &p);
        pl.dead = true;
        pl.tick(&all, &p, &w);
        assert_eq!((pl.vel[0], pl.vel[2]), (0.0, 0.0));
        assert!(pl.vel[1] < 0.0 && pl.pos[1] == 10.0);
        pl.tick(&all, &p, &w);
        assert!(pl.pos[1] < 10.0 && pl.pos[0] == 0.5 && pl.pos[2] == 0.5);
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
                    sprint: i % 4 == 0,
                    ..TickInput::default()
                };
                pl.tick(&input, &p, &w);
            }
            pl
        };
        assert_eq!(run(), run());
    }

    /// A 3x3 island (blocks 0..3 on x and z) with its top at y = 1.
    fn island() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in 0..3 {
            for z in 0..3 {
                w.set(x, 0, z, Block::Stone);
            }
        }
        w
    }

    /// Sneak for 200 ticks from the island's centre and return the player.
    fn sneak_on_island(forward: f32, strafe: f32, yaw: f32, sneak: bool) -> Player {
        let (p, w) = (phys(), island());
        let mut pl = Player::new([1.5, 1.0, 1.5], &p);
        pl.tick(&TickInput::default(), &p, &w);
        let input = TickInput {
            move_forward: forward,
            move_strafe: strafe,
            yaw,
            sneak,
            ..TickInput::default()
        };
        for _ in 0..200 {
            pl.tick(&input, &p, &w);
            if sneak {
                assert!(pl.on_ground && pl.pos[1] == 1.0, "left the ledge: {pl:?}");
            }
        }
        pl
    }

    #[test]
    fn sneaking_never_leaves_ledge_from_any_direction() {
        let half = std::f32::consts::FRAC_PI_2;
        for k in 0..4 {
            let pl = sneak_on_island(1.0, 0.0, k as f32 * half, true);
            // It got to the edge, rather than stopping early.
            let edge = (0..2).any(|i| !(0.05..=2.95).contains(&pl.pos[2 * i]));
            assert!(edge, "{pl:?}");
            // Only the body's edge may overhang, never the centre.
            assert!(pl.pos[0] > -0.3 && pl.pos[0] < 3.3 && pl.pos[2] > -0.3 && pl.pos[2] < 3.3);
        }
    }

    #[test]
    fn sneaking_diagonally_into_a_corner_stays_on() {
        for k in 0..4 {
            let yaw = std::f32::consts::FRAC_PI_4 + k as f32 * std::f32::consts::FRAC_PI_2;
            sneak_on_island(1.0, 0.0, yaw, true);
            sneak_on_island(1.0, 1.0, yaw, true);
        }
    }

    #[test]
    fn sneaking_slides_along_a_ledge() {
        // Forward is -z; strafe pushes +x. Walk to the north edge first, then
        // press into it while strafing: z is held back, x keeps moving.
        let (p, w) = (phys(), island());
        let mut pl = Player::new([1.5, 1.0, 1.5], &p);
        pl.tick(&TickInput::default(), &p, &w);
        let fwd = TickInput {
            move_forward: 1.0,
            sneak: true,
            ..TickInput::default()
        };
        for _ in 0..100 {
            pl.tick(&fwd, &p, &w);
        }
        let (x0, z0) = (pl.pos[0], pl.pos[2]);
        let diag = TickInput {
            move_strafe: 1.0,
            ..fwd
        };
        for _ in 0..20 {
            pl.tick(&diag, &p, &w);
            assert!(pl.on_ground);
        }
        assert!(pl.pos[0] > x0 + 0.2, "{pl:?}");
        assert!(pl.pos[2] >= z0 - 0.06 && pl.pos[2] < 0.5, "{pl:?}");
    }

    #[test]
    fn not_sneaking_or_airborne_is_unaffected() {
        // Walking off the island falls.
        let pl = sneak_on_island(1.0, 0.0, 0.0, false);
        assert!(pl.pos[1] < 1.0 && pl.pos[2] < 0.0, "{pl:?}");
        // In the air, sneaking does not hold the player back.
        let (p, w) = (phys(), island());
        let mut pl = Player::new([1.5, 5.0, 1.5], &p);
        pl.vel[2] = -3.0;
        let input = TickInput {
            sneak: true,
            ..TickInput::default()
        };
        pl.tick(&input, &p, &w);
        assert!(!pl.on_ground && pl.pos[2] < -1.0, "{pl:?}");
    }
}
