//! Animals in water, end to end: the Phase C "done when" of
//! docs/design/raft-scene-feedback.md (item 2) with the 4.3 water numbers of
//! docs/design/basic-gameplay.md.
//!
//! A sea over a stone floor holds a shoal (an island whose top is one block
//! under the surface) and a raft that sways with the swell. Animals are spawned
//! in it and the store is ticked against the `RaftWorld`.

use game::animal_entities::{AnimalEntity, AnimalStore};
use game::animals::Animals;
use game::collision::{CollisionWorld, MemoryWorld};
use game::item_entities::ItemEntityConfig;
use game::physics::PlayerPhysics;
use game::raft::{Raft, RaftWorld, Swell};
use game::targeting::EntityId;
use terrain::Block;

const SEA: i32 = 32;
/// Water fills `y = 1..SURFACE`; its top face is at `y = SURFACE`.
const SURFACE: i32 = 64;
/// The shoal covers `x >= SHOAL_X`, `|z| <= 3`, with its top at `y = SURFACE - 1`.
const SHOAL_X: i32 = 10;
const SEED: u64 = 7;

fn base() -> MemoryWorld {
    let mut w = MemoryWorld::new();
    for x in -SEA..SEA {
        for z in -SEA..SEA {
            w.set(x, 0, z, Block::Stone);
            let shoal = (SHOAL_X..SHOAL_X + 4).contains(&x) && z.abs() <= 3;
            for y in 1..SURFACE {
                let b = if shoal && y < SURFACE - 1 {
                    Block::Stone
                } else {
                    Block::Water
                };
                w.set(x, y, z, b);
            }
        }
    }
    w
}

/// A small raft far from the shoal, at the western edge of the sea.
fn raft() -> Raft {
    Raft::new(&[[-24, SURFACE, 0], [-23, SURFACE, 0], [-24, SURFACE, 1]])
}

fn drown_ticks(species: &str) -> u32 {
    Animals::default_animals().get(species).unwrap().drown_ticks
}

struct Run {
    store: AnimalStore,
    base: MemoryWorld,
    raft: Raft,
    swell: Option<Swell>,
    phys: PlayerPhysics,
    tick: u64,
}

impl Run {
    fn new(seed: u64, swell: bool) -> Run {
        Run {
            store: AnimalStore::new(
                Animals::default_animals(),
                ItemEntityConfig::default_from_data(),
                1000,
                seed,
            ),
            base: base(),
            raft: raft(),
            swell: swell.then(|| Swell::default_from_data().unwrap()),
            phys: PlayerPhysics::default_from_data().unwrap(),
            tick: 0,
        }
    }

    fn spawn(&mut self, species: &str, pos: [f64; 3]) -> EntityId {
        self.store.spawn(species, pos, 0.0).unwrap()
    }

    fn step(&mut self) {
        if let Some(s) = &self.swell {
            self.raft.advance(&s.frame(self.tick), &mut [], &self.phys);
        }
        self.tick += 1;
        self.store.tick(&RaftWorld::new(&self.base, &self.raft));
    }

    fn animal(&self, id: EntityId) -> &AnimalEntity {
        self.store.get(id).expect("animal still in the store")
    }

    fn head_out(&self, id: EntityId) -> bool {
        let world = RaftWorld::new(&self.base, &self.raft);
        let mut bb = self.animal(id).aabb();
        bb.min[1] = bb.max[1] - 1e-3;
        !world.body_in_water(&bb)
    }
}

/// Spawned 1.2 blocks under the surface, away from any support.
const OPEN_WATER: [f64; 3] = [0.5, SURFACE as f64 - 1.2, -20.5];

#[test]
fn an_animal_in_water_rises_and_stays_at_the_surface() {
    for swell in [false, true] {
        for species in ["cow", "pig", "chicken"] {
            let mut r = Run::new(SEED, swell);
            let id = r.spawn(species, OPEN_WATER);
            let h = r.animal(id).box_size[1];
            for t in 0..250 {
                r.step();
                let a = r.animal(id);
                assert!(a.death_ticks_left.is_none(), "{species} {swell} t{t}");
                assert!(a.pos[1] < SURFACE as f64, "{species} {swell} t{t} {a:?}");
                // Once risen (well inside 40 ticks) it never sinks back.
                if t >= 40 {
                    assert!(
                        a.pos[1] > SURFACE as f64 - h - 0.1,
                        "{species} {swell} t{t} {a:?}"
                    );
                }
            }
            assert!(r.head_out(id), "{species} {swell}: head under water");
        }
    }
}

#[test]
fn an_animal_near_land_reaches_support_and_does_not_drown() {
    for swell in [false, true] {
        let mut r = Run::new(SEED, swell);
        // Five blocks off the shoal.
        let id = r.spawn("cow", [SHOAL_X as f64 - 5.0, SURFACE as f64 - 1.2, 0.5]);
        for _ in 0..(drown_ticks("cow") + 200) {
            r.step();
        }
        let a = r.animal(id);
        assert!(
            a.death_ticks_left.is_none() && a.health > 0,
            "{swell} {a:?}"
        );
        assert_eq!(a.breath_ticks, 0, "{swell} {a:?}");
        // Pressed against the shoal's edge, its body touching the support.
        let edge = SHOAL_X as f64 - a.box_size[0] / 2.0;
        assert!((a.pos[0] - edge).abs() < 0.05, "{swell} {a:?}");
    }
}

#[test]
fn an_animal_floating_out_of_reach_of_support_keeps_its_head_up() {
    // Drowning counts ticks with the head under water (#716), and a swimmer
    // rises until its head is clear, so open water alone never drowns it.
    for swell in [false, true] {
        let mut r = Run::new(SEED, swell);
        let id = r.spawn("pig", OPEN_WATER);
        for _ in 0..(drown_ticks("pig") + 100) {
            r.step();
        }
        assert!(r.animal(id).death_ticks_left.is_none(), "{swell}");
        assert!(r.head_out(id), "{swell}");
    }
}

#[test]
fn a_held_down_animal_drowns_after_exactly_drown_ticks() {
    // A pig under a ceiling cannot reach the air: it drowns on the tick its
    // breath count reaches the configured number.
    let n = drown_ticks("pig");
    for swell in [false, true] {
        let mut r = Run::new(SEED, swell);
        for x in -3..=3 {
            for z in -23..=-17 {
                // A roof one block under the surface, over the spawn.
                r.base.set(x, SURFACE - 1, z, Block::Stone);
            }
        }
        let id = r.spawn("pig", [0.5, SURFACE as f64 - 3.0, -20.5]);
        for t in 1..=n {
            r.step();
            let a = r.animal(id);
            if t < n {
                assert!(a.death_ticks_left.is_none(), "{swell}: died early at {t}");
                assert_eq!(a.breath_ticks, t, "{swell}");
            } else {
                assert_eq!(a.health, 0, "{swell}: alive at tick {n}");
                assert!(a.death_ticks_left.is_some(), "{swell}");
            }
        }
    }
}

fn trace(seed: u64, swell: bool) -> Vec<Vec<AnimalEntity>> {
    let mut r = Run::new(seed, swell);
    r.spawn("cow", OPEN_WATER);
    r.spawn("pig", [4.5, SURFACE as f64 - 1.0, -18.5]);
    r.spawn("chicken", [SHOAL_X as f64 - 4.0, SURFACE as f64 - 1.0, 0.5]);
    (0..250)
        .map(|_| {
            r.step();
            r.store.iter().cloned().collect()
        })
        .collect()
}

#[test]
fn same_seed_and_spawns_match_step_by_step_and_a_different_seed_differs() {
    for swell in [false, true] {
        let a = trace(SEED, swell);
        let b = trace(SEED, swell);
        for (t, (x, y)) in a.iter().zip(&b).enumerate() {
            assert_eq!(x, y, "swell {swell}: diverged at tick {t}");
        }
        assert_ne!(a, trace(SEED + 1, swell), "swell {swell}: seed ignored");
    }
}

#[test]
fn the_swell_does_not_change_animal_motion() {
    // Deck swell never pushes animals (#635), and the raft is out of range.
    assert_eq!(trace(SEED, false), trace(SEED, true));
}
