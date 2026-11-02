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
