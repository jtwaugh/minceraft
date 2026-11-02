//! The fishing bobber in flight: cast from the eye, fall with item-entity
//! physics, then float on water or lie on a block.
//! See docs/design/fish-and-fishing.md section 4.2.
//!
//! Pure: the path depends only on the launch and the blocks it meets.

use crate::collision::{Aabb, CollisionWorld};
use crate::fishing_config::FishingConfig;
use crate::item_entities::{lift_out_of_blocks, ItemEntityConfig, TICKS_PER_SECOND};
use crate::movement::move_box;

/// Moves per tick, so a fast bobber cannot step over a thin water layer.
const SUBSTEPS: usize = 4;

/// Highest water surface searched above a landing, in blocks.
const SURFACE_SCAN: i32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BobberState {
    Flying,
    /// Landed in water: rests at the water surface.
    Floating,
    /// Landed on a block: lies there.
    Resting,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bobber {
    cfg: ItemEntityConfig,
    /// Centre of the box, in world coordinates.
    pos: [f64; 3],
    /// Blocks per second.
    velocity: [f64; 3],
    state: BobberState,
}

impl Bobber {
    /// Cast from `eye` along the unit vector `look`.
    pub fn cast(
        cfg: ItemEntityConfig,
        fishing: &FishingConfig,
        eye: [f64; 3],
        look: [f64; 3],
    ) -> Bobber {
        let mut velocity = look.map(|c| c * fishing.cast_speed);
        velocity[1] += fishing.cast_upward_kick;
        Bobber {
            cfg,
            pos: eye,
            velocity,
            state: BobberState::Flying,
        }
    }

    pub fn pos(&self) -> [f64; 3] {
        self.pos
    }

    pub fn state(&self) -> BobberState {
        self.state
    }

    fn aabb(&self) -> Aabb {
        let h = self.cfg.box_size / 2.0;
        Aabb::new(
            [0, 1, 2].map(|i| self.pos[i] - h),
            [0, 1, 2].map(|i| self.pos[i] + h),
        )
    }

    /// Advance one tick. Does nothing once landed.
    pub fn tick(&mut self, world: &impl CollisionWorld) {
        if self.state != BobberState::Flying {
            return;
        }
        let dt = 1.0 / TICKS_PER_SECOND;
        let mut bb = self.aabb();
        lift_out_of_blocks(world, &mut bb, &mut self.pos, &mut Vec::new());
        self.velocity[1] -= self.cfg.gravity * dt;
        for _ in 0..SUBSTEPS {
            let bb = self.aabb();
            let v = self.velocity.map(|c| c * dt / SUBSTEPS as f64);
            let m = move_box(world, &bb, v);
            for i in 0..3 {
                self.pos[i] += m.applied[i];
            }
            if world.body_in_water(&self.aabb()) {
                if let Some(top) = self.surface(world) {
                    self.pos[1] = top;
                    self.velocity = [0.0; 3];
                    self.state = BobberState::Floating;
                    return;
                }
            }
            if m.on_ground {
                self.velocity = [0.0; 3];
                self.state = BobberState::Resting;
                return;
            }
            // Hitting a side or a ceiling stops that axis only.
            for i in 0..3 {
                if m.applied[i] != v[i] {
                    self.velocity[i] = 0.0;
                }
            }
        }
        self.velocity = self.velocity.map(|c| c * self.cfg.drag);
    }

    /// Top of the water the box is in: the highest water cell it overlaps,
    /// followed up while the cell above is water.
    fn surface(&self, world: &impl CollisionWorld) -> Option<f64> {
        let bb = self.aabb();
        let lo = |i: usize| bb.min[i].floor() as i32;
        let hi = |i: usize| bb.max[i].ceil() as i32 - 1;
        let mut top: Option<i32> = None;
        for x in lo(0)..=hi(0) {
            for z in lo(2)..=hi(2) {
                for y in (lo(1)..=hi(1)).rev() {
                    if world.block_at(x, y, z).info().is_water() {
                        let mut t = y;
                        while t - y < SURFACE_SCAN && world.block_at(x, t + 1, z).info().is_water()
                        {
                            t += 1;
                        }
                        top = Some(top.map_or(t, |b| b.max(t)));
                        break;
                    }
                }
            }
        }
        top.map(|t| t as f64 + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use terrain::Block;

    fn cfgs() -> (ItemEntityConfig, FishingConfig) {
        (
            ItemEntityConfig::default_from_data(),
            FishingConfig::default_from_data().unwrap(),
        )
    }

    /// Stone floor at y = 0 over -40..40, shifted by `dx`; water fills
    /// y = 1..4 where x >= `wx` (before the shift).
    fn world(dx: i32, wx: i32) -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -40..40 {
            for z in -40..40 {
                w.set(x + dx, 0, z, Block::Stone);
                if x >= wx {
                    for y in 1..4 {
                        w.set(x + dx, y, z, Block::Water);
                    }
                }
            }
        }
        w
    }

    fn fly(w: &MemoryWorld, eye: [f64; 3], look: [f64; 3]) -> Bobber {
        let (item, fishing) = cfgs();
        let mut b = Bobber::cast(item, &fishing, eye, look);
        for _ in 0..400 {
            b.tick(w);
        }
        b
    }

    #[test]
    fn launches_with_speed_and_kick() {
        let (item, fishing) = cfgs();
        let b = Bobber::cast(item, &fishing, [0.0, 10.0, 0.0], [1.0, 0.0, 0.0]);
        assert_eq!(b.state(), BobberState::Flying);
        assert_eq!(
            b.velocity,
            [fishing.cast_speed, fishing.cast_upward_kick, 0.0]
        );
        let mut b = b;
        b.tick(&MemoryWorld::new());
        assert!(b.pos()[0] > 0.0 && b.pos()[1] > 10.0);
        assert_eq!(b.state(), BobberState::Flying);
    }

    #[test]
    fn lands_on_water_and_floats_at_the_surface() {
        let w = world(0, -40);
        let b = fly(&w, [0.5, 6.0, 0.5], [1.0, 0.0, 0.0]);
        assert_eq!(b.state(), BobberState::Floating);
        assert_eq!(b.pos()[1], 4.0);
        assert!(b.pos()[0] > 3.0);
    }

    #[test]
    fn lands_on_a_block_and_rests_on_top() {
        let w = world(0, 100);
        let b = fly(&w, [0.5, 3.0, 0.5], [1.0, 0.0, 0.0]);
        assert_eq!(b.state(), BobberState::Resting);
        assert!((b.pos()[1] - (1.0 + b.cfg.box_size / 2.0)).abs() < 1e-6);
    }

    #[test]
    fn a_landed_bobber_stays_put() {
        let w = world(0, -40);
        let mut b = fly(&w, [0.5, 6.0, 0.5], [1.0, 0.0, 0.0]);
        let at = b.pos();
        for _ in 0..50 {
            b.tick(&w);
        }
        assert_eq!(b.pos(), at);
    }

    #[test]
    fn shifted_worlds_give_the_same_path() {
        let (item, fishing) = cfgs();
        for dx in [0, 7, -13] {
            let w = world(dx, -40);
            let eye = [0.5 + dx as f64, 6.0, 0.5];
            let mut b = Bobber::cast(item, &fishing, eye, [0.6, 0.0, 0.8]);
            let mut base = Bobber::cast(item, &fishing, [0.5, 6.0, 0.5], [0.6, 0.0, 0.8]);
            let w0 = world(0, -40);
            for _ in 0..100 {
                b.tick(&w);
                base.tick(&w0);
                assert_eq!(b.state(), base.state());
                assert!((b.pos()[0] - dx as f64 - base.pos()[0]).abs() < 1e-9);
                assert_eq!(b.pos()[1], base.pos()[1]);
            }
        }
    }
}
