//! The player's fishing line: at most one bobber, cast and reeled in.
//! See docs/design/fish-and-fishing.md sections 4.2 and 4.6.
//!
//! A floating bobber draws a bite delay; a fish near it is lured in and
//! bites on arrival. Catching and the window closing are not built yet, so
//! every reel still brings in an empty line.

use crate::bobber::{Bobber, BobberState};
use crate::collision::CollisionWorld;
use crate::events::GameEvent;
use crate::fish_world::FishWorld;
use crate::fishing_config::FishingConfig;
use crate::item_entities::ItemEntityConfig;
use crate::rng::Rng;
use crate::targeting::EntityId;

/// Seed of the bite-delay stream.
const BITE_SEED: u64 = 0x2B17_E5A9_61D4_C803;

/// Where a line is in its wait for a bite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bite {
    /// No delay drawn yet (the bobber has not settled on water).
    Idle,
    /// Ticks left before a fish is picked.
    Waiting(u32),
    /// This fish is swimming to the bobber.
    Luring(EntityId),
    /// A fish bit; the reel window has this many ticks left.
    Open(u32),
}

#[derive(Clone, Debug)]
pub struct Fishing {
    cfg: FishingConfig,
    item_cfg: ItemEntityConfig,
    bobber: Option<Bobber>,
    rng: Rng,
    bite: Bite,
}

impl Fishing {
    pub fn new(cfg: FishingConfig, item_cfg: ItemEntityConfig) -> Fishing {
        Fishing {
            cfg,
            item_cfg,
            bobber: None,
            rng: Rng::new(BITE_SEED),
            bite: Bite::Idle,
        }
    }

    /// Whether a bobber is out.
    pub fn line_out(&self) -> bool {
        self.bobber.is_some()
    }

    pub fn config(&self) -> &FishingConfig {
        &self.cfg
    }

    pub fn bobber(&self) -> Option<&Bobber> {
        self.bobber.as_ref()
    }

    /// Cast from `eye` along the unit vector `look`. Does nothing with a
    /// line already out.
    pub fn cast(&mut self, eye: [f64; 3], look: [f64; 3], events: &mut Vec<GameEvent>) {
        if self.bobber.is_some() {
            return;
        }
        self.bite = Bite::Idle;
        self.bobber = Some(Bobber::cast(self.item_cfg, &self.cfg, eye, look));
        events.push(GameEvent::LineCast {
            player: "player".to_string(),
            position: eye,
        });
    }

    /// Bring the line in with nothing on it.
    pub fn reel(&mut self, events: &mut Vec<GameEvent>) {
        if self.bobber.take().is_some() {
            self.bite = Bite::Idle;
            events.push(GameEvent::LineReeled {
                player: "player".to_string(),
                caught: false,
            });
        }
    }

    /// Advance the bobber one tick, then reel in if the rod is no longer in
    /// hand or the player at `player` is beyond `max_distance`.
    pub fn tick(
        &mut self,
        world: &impl CollisionWorld,
        rod_in_hand: bool,
        player: [f64; 3],
        events: &mut Vec<GameEvent>,
    ) {
        let Some(b) = &mut self.bobber else { return };
        b.tick(world);
        let d: f64 = (0..3).map(|i| (b.pos()[i] - player[i]).powi(2)).sum();
        if !rod_in_hand || d > self.cfg.max_distance * self.cfg.max_distance {
            self.reel(events);
        }
    }
}

impl Fishing {
    /// Advance the bite one tick; call after the fish have moved. Only a
    /// floating bobber gets bites. With no fish within `near_radius` when
    /// the delay runs out, nothing happens and a new delay is drawn.
    pub fn tick_bite(
        &mut self,
        world: &impl CollisionWorld,
        fish: &mut FishWorld,
        events: &mut Vec<GameEvent>,
    ) {
        let Some(b) = &mut self.bobber else { return };
        if b.state() != BobberState::Floating {
            return;
        }
        let pos = b.pos();
        let [lo, hi] = self.cfg.bite_delay_ticks;
        self.bite = match self.bite {
            Bite::Idle => Bite::Waiting(self.rng.range(lo as i32, hi as i32) as u32),
            Bite::Waiting(n) if n > 1 => Bite::Waiting(n - 1),
            Bite::Waiting(_) => match nearest_fish(fish, pos, self.cfg.near_radius) {
                Some(id) => {
                    fish.lure_to_bobber(id, pos, world);
                    Bite::Luring(id)
                }
                None => Bite::Idle,
            },
            Bite::Luring(id) => match fish.store().get(id).map(|f| (f.species.clone(), f.lure)) {
                None | Some((_, None)) => Bite::Idle,
                Some((species, _)) if fish.lure_arrived(id) => {
                    b.set_dipping(true);
                    events.push(GameEvent::FishBite {
                        player: "player".to_string(),
                        species,
                    });
                    Bite::Open(self.cfg.reel_window_ticks)
                }
                Some(_) => Bite::Luring(id),
            },
            Bite::Open(n) => Bite::Open(n.saturating_sub(1)),
        };
    }
}

/// The fish whose centre is nearest `pos` and within `radius`; the lowest
/// id wins a tie.
fn nearest_fish(fish: &FishWorld, pos: [f64; 3], radius: f64) -> Option<EntityId> {
    fish.store()
        .iter()
        .map(|f| {
            let c = [f.pos[0], f.pos[1] + f.box_size[1] / 2.0, f.pos[2]];
            let d2: f64 = (0..3).map(|i| (c[i] - pos[i]).powi(2)).sum();
            (d2, f.id)
        })
        .filter(|(d2, _)| *d2 <= radius * radius)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Aabb, MemoryWorld};
    use terrain::Block;

    fn fishing() -> Fishing {
        Fishing::new(
            FishingConfig::default_from_data().unwrap(),
            ItemEntityConfig::default_from_data(),
        )
    }

    #[test]
    fn one_bobber_at_a_time() {
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        f.cast([9.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        assert_eq!(ev.len(), 1);
        assert!(f.line_out());
        f.reel(&mut ev);
        f.reel(&mut ev);
        assert!(!f.line_out());
        assert_eq!(ev.len(), 2);
        assert_eq!(
            ev[1],
            GameEvent::LineReeled {
                player: "player".into(),
                caught: false
            }
        );
    }

    #[test]
    fn putting_the_rod_away_or_walking_off_reels_in() {
        let w = MemoryWorld::new();
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        f.tick(&w, true, [0.0, 5.0, 0.0], &mut ev);
        assert!(f.line_out());
        f.tick(&w, false, [0.0, 5.0, 0.0], &mut ev);
        assert!(!f.line_out());
        f.cast([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &mut ev);
        f.tick(&w, true, [500.0, 5.0, 0.0], &mut ev);
        assert!(!f.line_out());
    }

    /// A square pool, water from `floor + 1` to y = 58, with a wall one
    /// block outside `-half..half`.
    fn pool(half: i32, floor: i32) -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -half - 1..=half {
            for z in -half - 1..=half {
                let wall = !(-half..half).contains(&x) || !(-half..half).contains(&z);
                for y in floor..=58 {
                    let stone = y == floor || wall;
                    w.set(x, y, z, if stone { Block::Stone } else { Block::Water });
                }
            }
        }
        w
    }

    /// A pool small and shallow enough that every fish stays in range.
    fn pen() -> MemoryWorld {
        pool(2, 56)
    }

    /// Cast straight down onto the pool (or onto bare ground if `dry`),
    /// with a fish at each of `fish`; run `ticks`. Returns the tick the
    /// bobber settled, the bite events with their ticks and the fish ids.
    fn run(
        fish: &[[f64; 3]],
        w: MemoryWorld,
        dry: bool,
        ticks: u64,
    ) -> (Option<u64>, Vec<(u64, GameEvent)>, Vec<EntityId>, usize) {
        let mut w = w;
        if dry {
            w.set(0, 59, 0, Block::Stone);
        }
        let mut world = FishWorld::default_from_data(1, 1 << 28).unwrap();
        let ids: Vec<_> = fish
            .iter()
            .map(|p| world.spawn("mackerel", *p, 0.0).unwrap())
            .collect();
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.5, 62.0, 0.5], [0.0, -1.0, 0.0], &mut ev);
        let far = Aabb::new([100.0, 100.0, 100.0], [101.0, 102.0, 101.0]);
        let (mut settled, mut bites) = (None, Vec::new());
        for t in 0..ticks {
            f.tick(&w, true, [0.5, 62.0, 0.5], &mut ev);
            world.tick(t, [0.0, 0.0], &far, &w, |_, _| None);
            f.tick_bite(&w, &mut world, &mut ev);
            let landed = f.bobber().is_some_and(|b| b.state() != BobberState::Flying);
            if settled.is_none() && landed {
                settled = Some(t);
            }
            for e in ev.drain(..) {
                if matches!(e, GameEvent::FishBite { .. }) {
                    bites.push((t, e));
                }
            }
        }
        (settled, bites, ids, world.store().len())
    }

    #[test]
    fn a_fish_at_the_bobber_bites_within_the_delay_range() {
        let (settled, bites, ids, _) = run(&[[0.5, 57.5, 0.5]], pen(), false, 600);
        let [lo, hi] = fishing().config().bite_delay_ticks;
        assert_eq!(bites.len(), 1);
        let after = bites[0].0 - settled.unwrap();
        assert!((lo as u64..=hi as u64 + 40).contains(&after), "{after}");
        assert_eq!(
            bites[0].1,
            GameEvent::FishBite {
                player: "player".into(),
                species: "mackerel".into()
            }
        );
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn the_nearest_fish_in_range_is_the_one_lured() {
        let w = pool(12, 50);
        let mut world = FishWorld::default_from_data(1, 1 << 28).unwrap();
        let far = world.spawn("mackerel", [3.0, 57.5, 0.5], 0.0).unwrap();
        let near = world.spawn("mackerel", [1.5, 57.5, 0.5], 0.0).unwrap();
        let out = world.spawn("mackerel", [9.5, 57.5, 0.5], 0.0).unwrap();
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.5, 62.0, 0.5], [0.0, -1.0, 0.0], &mut ev);
        for _ in 0..80 {
            f.tick(&w, true, [0.5, 62.0, 0.5], &mut ev);
        }
        f.bite = Bite::Waiting(1);
        f.tick_bite(&w, &mut world, &mut ev);
        assert_eq!(f.bite, Bite::Luring(near));
        assert!(world.store().get(near).unwrap().lure.is_some());
        for id in [far, out] {
            assert!(world.store().get(id).unwrap().lure.is_none());
        }
    }

    #[test]
    fn a_scatter_during_luring_goes_back_to_idle() {
        let w = pool(12, 50);
        let mut world = FishWorld::default_from_data(1, 1 << 28).unwrap();
        let id = world.spawn("mackerel", [3.0, 57.5, 0.5], 0.0).unwrap();
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.5, 62.0, 0.5], [0.0, -1.0, 0.0], &mut ev);
        for _ in 0..80 {
            f.tick(&w, true, [0.5, 62.0, 0.5], &mut ev);
        }
        f.bite = Bite::Waiting(1);
        f.tick_bite(&w, &mut world, &mut ev);
        assert_eq!(f.bite, Bite::Luring(id));
        world.clear_lure(id);
        f.tick_bite(&w, &mut world, &mut ev);
        assert_eq!(f.bite, Bite::Idle);
        f.tick_bite(&w, &mut world, &mut ev);
        assert!(matches!(f.bite, Bite::Waiting(_)));
    }

    #[test]
    fn no_fish_near_means_no_bite_and_no_spawn() {
        let (_, bites, _, count) = run(&[[9.5, 57.5, 0.5]], pool(12, 50), false, 900);
        assert!(bites.is_empty());
        assert_eq!(count, 1);
        let (_, bites, _, count) = run(&[], pool(12, 50), false, 900);
        assert!(bites.is_empty() && count == 0);
    }

    #[test]
    fn a_resting_bobber_never_gets_a_bite() {
        let (settled, bites, _, _) = run(&[[0.5, 57.5, 0.5]], pen(), true, 600);
        assert!(settled.is_some());
        assert!(bites.is_empty());
    }

    #[test]
    fn the_same_input_bites_on_the_same_tick() {
        let a = run(&[[0.5, 57.5, 0.5]], pen(), false, 600);
        let b = run(&[[0.5, 57.5, 0.5]], pen(), false, 600);
        assert_eq!(a.1.len(), 1);
        assert_eq!(a.1, b.1);
    }

    #[test]
    fn a_bite_dips_the_bobber_and_opens_the_window() {
        let w = pen();
        let mut world = FishWorld::default_from_data(1, 1 << 28).unwrap();
        world.spawn("mackerel", [0.5, 57.5, 0.5], 0.0).unwrap();
        let mut f = fishing();
        let mut ev = Vec::new();
        f.cast([0.5, 62.0, 0.5], [0.0, -1.0, 0.0], &mut ev);
        let far = Aabb::new([100.0, 100.0, 100.0], [101.0, 102.0, 101.0]);
        for t in 0..400 {
            f.tick(&w, true, [0.5, 62.0, 0.5], &mut ev);
            world.tick(t, [0.0, 0.0], &far, &w, |_, _| None);
            f.tick_bite(&w, &mut world, &mut ev);
            if matches!(f.bite, Bite::Open(_)) {
                break;
            }
        }
        let window = f.config().reel_window_ticks;
        assert_eq!(f.bite, Bite::Open(window));
        assert!(f.bobber().unwrap().dipping());
    }
}
