//! Fish in the game tick: the store, the spawner and the swimming, driven by
//! one call per sim tick. See docs/design/fish-and-fishing.md sections 3.1
//! and 3.2.

use crate::collision::{Aabb, CollisionWorld};
use crate::data_error::DataError;
use crate::fish_bodies::FishBodies;
use crate::fish_entities::FishStore;
use crate::fish_spawn_config::FishSpawnConfig;
use crate::fish_spawner::FishSpawner;
use crate::fish_steer::FishSteer;
use crate::phase::PhaseClock;
use crate::rng::Rng;
use crate::targeting::EntityId;
use layout::{Fish, FishColumn, Habitats};

/// Mixed into the seed for the swim RNG, so it is not the spawner's stream.
const SWIM_SEED: u64 = 0x5F1D_33A7_0C6E_9B21;

/// A fish taken out of the water.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catch {
    pub species: String,
    /// The item it becomes, from `data/fish_bodies.toml`.
    pub raw_item: String,
}

pub struct FishWorld {
    store: FishStore,
    spawner: FishSpawner,
    steer: FishSteer,
    rng: Rng,
}

impl FishWorld {
    /// Fish from the shipped data files, with a full stock. Fish ids start at
    /// `first_id`.
    pub fn default_from_data(seed: u64, first_id: EntityId) -> Result<FishWorld, DataError> {
        let config = FishSpawnConfig::default_from_data()?;
        let bodies = FishBodies::default_from_data()?;
        let spawner = FishSpawner::new(
            config.clone(),
            PhaseClock::default_phases()?,
            Fish::shipped(),
            bodies.clone(),
            &Habitats::shipped(),
            seed,
        );
        Ok(FishWorld {
            store: FishStore::new(bodies, first_id),
            spawner,
            steer: FishSteer::new(config),
            rng: Rng::new(seed ^ SWIM_SEED),
        })
    }

    pub fn store(&self) -> &FishStore {
        &self.store
    }

    /// Add a fish outside the spawner, in school 0. `None` for an unknown species.
    pub fn spawn(&mut self, species: &str, pos: [f64; 3], heading: f64) -> Option<EntityId> {
        self.store.spawn(species, pos, heading, 0)
    }

    /// Hit fish `id` for `damage` half-hearts. When that kills it, the fish
    /// is removed and its species and raw item are returned. A removed fish
    /// does not go back to the stock. A survivor and its school scatter from
    /// `attacker` for `hit_scatter_ticks`. `None` if the fish survives or the
    /// id is unknown.
    pub fn hit(&mut self, id: EntityId, damage: u32, attacker: [f64; 3]) -> Option<Catch> {
        let species = self.store.get(id)?.species.clone();
        let (_, died) = self.store.hit(id, damage)?;
        if !died {
            let ticks = self.steer.config().hit_scatter_ticks;
            self.store.scatter_school(id, attacker, ticks);
            return None;
        }
        let raw_item = self.store.bodies().get(&species)?.raw_item.clone();
        Some(Catch { species, raw_item })
    }

    /// Remove fish `id` whole (a hooked fish reeled in) and return what it
    /// becomes. `None` for an unknown id.
    pub fn catch(&mut self, id: EntityId) -> Option<Catch> {
        let fish = self.store.remove(id)?;
        let raw_item = self.store.bodies().get(&fish.species)?.raw_item.clone();
        Some(Catch {
            species: fish.species,
            raw_item,
        })
    }

    /// Make fish `id` swim to `point` instead of wandering, until the lure
    /// is cleared or a scatter. No-op for an unknown id.
    pub fn set_lure(&mut self, id: EntityId, point: [f64; 3]) {
        if let Some(f) = self.store.iter_mut().find(|f| f.id == id) {
            f.lure = Some(point);
        }
    }

    /// Make fish `id` swim to the bobber at `bobber`, as near as the water
    /// lets it get. No-op for an unknown id.
    pub fn lure_to_bobber<W: CollisionWorld + ?Sized>(
        &mut self,
        id: EntityId,
        bobber: [f64; 3],
        world: &W,
    ) {
        if let Some(f) = self.store.get(id) {
            let p = self.steer.lure_point(world, f, bobber);
            self.set_lure(id, p);
        }
    }

    /// Let fish `id` wander again. No-op for an unknown id.
    pub fn clear_lure(&mut self, id: EntityId) {
        if let Some(f) = self.store.iter_mut().find(|f| f.id == id) {
            f.lure = None;
        }
    }

    /// True if fish `id` is lured and within `lure_distance` of the point.
    pub fn lure_arrived(&self, id: EntityId) -> bool {
        let Some(f) = self.store.get(id) else {
            return false;
        };
        let Some(p) = f.lure else {
            return false;
        };
        let c = [f.pos[0], f.pos[1] + f.box_size[1] / 2.0, f.pos[2]];
        let d2: f64 = (0..3).map(|i| (p[i] - c[i]).powi(2)).sum();
        d2.sqrt() <= self.steer.config().lure_distance
    }

    /// Fish left in today's stock.
    pub fn stock(&self) -> u32 {
        self.spawner.stock()
    }

    /// True on the first tick of a phase, when fish may spawn.
    pub fn is_phase_change(&self, tick: u64) -> bool {
        self.spawner.is_phase_change(tick)
    }

    /// One sim tick: despawn and spawn around the raft centre `raft` (x, z),
    /// then swim every fish in `world`. Fish scatter from `player` (the
    /// player's box) when it is in water. `column` is the spawn column
    /// accessor (`Streamer::fish_column`); `None` means no water.
    pub fn tick<W: CollisionWorld + ?Sized>(
        &mut self,
        tick: u64,
        raft: [f64; 2],
        player: &Aabb,
        world: &W,
        column: impl FnMut(i32, i32) -> Option<FishColumn>,
    ) {
        self.spawner.tick(tick, raft, &mut self.store, column);
        let swimmer = world.body_in_water(player).then(|| {
            let [x0, y, z0] = player.min;
            [(x0 + player.max[0]) / 2.0, y, (z0 + player.max[2]) / 2.0]
        });
        self.steer
            .tick(&mut self.store, world, swimmer, &mut self.rng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use terrain::Block;

    fn open_sea(x: i32, z: i32) -> Option<FishColumn> {
        let habitats = Habitats::shipped();
        let h = (x.wrapping_mul(73) ^ z.wrapping_mul(151)).rem_euclid(97) as u32;
        let names = ["warm_shallows", "deep_water", "cold_open", "temperate_open"];
        Some(FishColumn {
            habitat: habitats.id_of(names[(h % 4) as usize]).unwrap(),
            temperature: f64::from(h % 10) / 10.0,
            depth: 2 + h % 40,
        })
    }

    #[test]
    fn hit_and_catch_remove_without_refilling() {
        let mut w = FishWorld::default_from_data(3, 5000).unwrap();
        let a = w.spawn("mackerel", [0.0, 40.0, 0.0], 0.0).unwrap();
        let b = w.spawn("tuna", [3.0, 40.0, 0.0], 0.0).unwrap();
        let c = w.spawn("salmon", [6.0, 40.0, 0.0], 0.0).unwrap();
        let stock = w.stock();
        let sword = crate::items::Items::default_items().damage(Some("sword"));
        let got = w.hit(a, sword, [0.0; 3]).unwrap();
        assert_eq!(
            (got.species.as_str(), got.raw_item.as_str()),
            ("mackerel", "raw_mackerel")
        );
        assert_eq!(w.hit(a, sword, [0.0; 3]), None);
        assert_eq!(w.hit(9, sword, [0.0; 3]), None);
        assert_eq!(
            w.hit(b, 1, [0.0; 3]),
            None,
            "a wounded tuna survives or dies by health"
        );
        let tuna = w.catch(b);
        assert_eq!(tuna.map(|t| t.raw_item), Some("raw_tuna".to_string()));
        assert_eq!(w.catch(b), None);
        assert_eq!(w.catch(c).unwrap().species, "salmon");
        assert_eq!((w.stock(), w.store().len()), (stock, 0));
    }

    fn pool() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -13..=12 {
            for z in -13..=12 {
                let wall = !(-12..12).contains(&x) || !(-12..12).contains(&z);
                for y in 50..=58 {
                    let stone = y == 50 || wall;
                    w.set(x, y, z, if stone { Block::Stone } else { Block::Water });
                }
            }
        }
        w
    }

    /// Hit a tuna from the west at tick 10, then tick 200 times; the
    /// distance from the attacker and the fish positions at each tick.
    fn replay() -> Vec<Vec<[f64; 3]>> {
        let world = pool();
        let mut w = FishWorld::default_from_data(3, 1).unwrap();
        let id = w.spawn("tuna", [0.5, 54.0, 0.5], 90.0).unwrap();
        assert!(w.store().get(id).unwrap().health > 1);
        let far = Aabb::new([100.0, 80.0, 100.0], [100.6, 81.8, 100.6]);
        let mut out = Vec::new();
        for tick in 0..200 {
            if tick == 10 {
                assert_eq!(w.hit(id, 1, [-1.5, 54.0, 0.5]), None);
            }
            w.tick(tick, [0.0, 0.0], &far, &world, |_, _| None);
            out.push(w.store().iter().map(|f| f.pos).collect());
        }
        out
    }

    #[test]
    fn a_surviving_hit_scatters_for_the_configured_ticks_then_wanders() {
        let run = replay();
        let ticks = FishSpawnConfig::default_from_data()
            .unwrap()
            .hit_scatter_ticks as usize;
        let speed = |t: usize| {
            let (a, b) = (run[t - 1][0], run[t][0]);
            (b[0] - a[0]).hypot(b[2] - a[2])
        };
        let first = speed(10);
        let wander = speed(9);
        assert!(first > wander * 1.5, "{first} vs {wander}");
        // It ends well clear of the attacker at x = -1.5.
        assert!(run[10 + ticks][0][0] > 0.5 + 2.0, "{:?}", run[10 + ticks]);
        // After the timer it moves at wander speed again.
        let tuna = FishBodies::default_from_data()
            .unwrap()
            .get("tuna")
            .unwrap()
            .swim_speed;
        assert!(speed(10 + ticks + 5) <= tuna * crate::clock::TICK_SECONDS + 1e-9);
    }

    #[test]
    fn set_lure_brings_the_fish_and_a_hit_or_clear_drops_it() {
        let world = pool();
        let far = Aabb::new([100.0, 80.0, 100.0], [100.6, 81.8, 100.6]);
        let mut w = FishWorld::default_from_data(3, 1).unwrap();
        let id = w.spawn("tuna", [-8.5, 54.0, 0.5], 90.0).unwrap();
        assert!(!w.lure_arrived(id));
        w.set_lure(id, [3.5, 55.0, 0.5]);
        for tick in 0..400 {
            w.tick(tick, [0.0, 0.0], &far, &world, |_, _| None);
        }
        assert!(w.lure_arrived(id));
        w.clear_lure(id);
        assert!(!w.lure_arrived(id));
        w.set_lure(id, [3.5, 55.0, 0.5]);
        assert_eq!(w.hit(id, 1, [0.0; 3]), None);
        assert_eq!(w.store().get(id).unwrap().lure, None);
        w.set_lure(99, [0.0; 3]);
        w.clear_lure(99);
    }

    #[test]
    fn the_same_hit_replays_to_the_same_positions() {
        assert_eq!(replay(), replay());
    }

    #[test]
    fn stock_plus_fish_stays_within_the_daily_total() {
        let daily = FishSpawnConfig::default_from_data().unwrap().daily_stock;
        let mut w = FishWorld::default_from_data(11, 1).unwrap();
        let player = Aabb::new([0.0, 40.0, 0.0], [0.6, 41.8, 0.6]);
        let mut caught = 0;
        for tick in 0..9600 {
            w.tick(tick, [0.0, 0.5], &player, &MemoryWorld::new(), open_sea);
            if w.is_phase_change(tick) && tick < 4800 {
                let first = w.store().iter().next().map(|f| f.id);
                if let Some(id) = first {
                    assert!(w.catch(id).is_some());
                    caught += 1;
                }
            }
            if tick < 4800 {
                assert!(w.stock() + w.store().len() as u32 <= daily, "{tick}");
            }
        }
        assert!(caught >= 1);
    }
}
