//! Fish spawning at phase changes, despawning, and the stock count.
//! See docs/design/fish-and-fishing.md section 3.1 and reconciliation notes
//! 3, 4 and 8.
//!
//! The daily stock reset, swimming and catches are separate tickets.

use crate::fish_bodies::FishBodies;
use crate::fish_entities::{FishStore, SchoolId};
use crate::fish_spawn_config::FishSpawnConfig;
use crate::phase::PhaseClock;
use crate::rng::Rng;
use layout::{Fish, FishColumn, HabitatId, Habitats};
use terrain::SEA_LEVEL;

/// Columns tried per phase change before the spawner gives up on the rest.
const MAX_TRIES: u32 = 64;

pub struct FishSpawner {
    config: FishSpawnConfig,
    clock: PhaseClock,
    fish: Fish,
    bodies: FishBodies,
    frozen: Option<HabitatId>,
    seed: u64,
    stock: u32,
    next_school: SchoolId,
}

/// The sim RNG for the phase change at `tick`, so its draws depend only on
/// the seed and the tick.
fn spawn_rng(seed: u64, tick: u64) -> Rng {
    let h = (seed ^ tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    Rng::new(h ^ (h >> 31))
}

/// Uniform in `[0, 1)`.
fn unit(rng: &mut Rng) -> f64 {
    f64::from(rng.next_u32()) / (f64::from(u32::MAX) + 1.0)
}

fn dist(pos: [f64; 3], raft: [f64; 2]) -> f64 {
    (pos[0] - raft[0]).hypot(pos[2] - raft[1])
}

impl FishSpawner {
    /// A spawner with a full stock.
    pub fn new(
        config: FishSpawnConfig,
        clock: PhaseClock,
        fish: Fish,
        bodies: FishBodies,
        habitats: &Habitats,
        seed: u64,
    ) -> FishSpawner {
        FishSpawner {
            stock: config.daily_stock,
            frozen: habitats.id_of("frozen"),
            config,
            clock,
            fish,
            bodies,
            seed,
            next_school: 0,
        }
    }

    /// Fish left in today's stock.
    pub fn stock(&self) -> u32 {
        self.stock
    }

    /// True on the first tick of a phase (tick 0 starts gathering).
    pub fn is_phase_change(&self, tick: u64) -> bool {
        tick == 0 || self.clock.at(tick).0 != self.clock.at(tick - 1).0
    }

    /// One sim tick: remove fish beyond the despawn radius of the raft centre
    /// `raft` (returning them to the stock), then, on a phase change, top the
    /// fish within the target radius up to the target count. `column` is the
    /// column accessor (`FishColumns::column`); `None` means no water.
    pub fn tick(
        &mut self,
        tick: u64,
        raft: [f64; 2],
        store: &mut FishStore,
        mut column: impl FnMut(i32, i32) -> Option<FishColumn>,
    ) {
        let far: Vec<_> = store
            .iter()
            .filter(|f| dist(f.pos, raft) > self.config.despawn_radius)
            .map(|f| f.id)
            .collect();
        for id in far {
            store.remove(id);
            self.stock = (self.stock + 1).min(self.config.daily_stock);
        }
        if !self.is_phase_change(tick) {
            return;
        }
        let near = store
            .iter()
            .filter(|f| dist(f.pos, raft) <= self.config.target_radius)
            .count() as u32;
        let mut need = self
            .config
            .target_count
            .saturating_sub(near)
            .min(self.stock);
        let mut rng = spawn_rng(self.seed, tick);
        for _ in 0..MAX_TRIES {
            if need == 0 {
                break;
            }
            let spawned = self.school(&mut rng, raft, need, store, &mut column);
            need -= spawned;
            self.stock -= spawned;
        }
    }

    /// Try to place one school of at most `max` fish at a column in the spawn
    /// ring. Returns the number spawned.
    fn school(
        &mut self,
        rng: &mut Rng,
        raft: [f64; 2],
        max: u32,
        store: &mut FishStore,
        column: &mut impl FnMut(i32, i32) -> Option<FishColumn>,
    ) -> u32 {
        let c = &self.config;
        let angle = f64::from(rng.angle());
        let r = c.ring_inner + (c.ring_outer - c.ring_inner) * unit(rng);
        let x = (raft[0] + r * angle.cos()).floor() as i32;
        let z = (raft[1] + r * angle.sin()).floor() as i32;
        let Some(col) = column(x, z).filter(|col| Some(col.habitat) != self.frozen) else {
            return 0;
        };
        let choices = self.fish.species_for(&col);
        let total: u32 = choices.iter().map(|(_, w)| w).sum();
        if total == 0 {
            return 0;
        }
        let mut roll = rng.range(0, total as i32 - 1) as u32;
        let Some(&(species, _)) = choices.iter().find(|(_, w)| {
            let hit = roll < *w;
            roll = roll.saturating_sub(*w);
            hit
        }) else {
            return 0;
        };
        let Some(body) = self.bodies.get(species) else {
            return 0;
        };
        let spec = self.fish.species.iter().find(|s| s.id == species);
        // Blocks from the surface down to the bottom of the box.
        let lo = (c.clearance + body.box_size[1])
            .max(spec.and_then(|s| s.depth_min).map_or(0.0, f64::from))
            .ceil();
        let hi = (f64::from(col.depth) - c.clearance)
            .min(spec.and_then(|s| s.depth_max).map_or(f64::MAX, f64::from))
            .floor();
        if lo > hi {
            return 0;
        }
        let depth = rng.range(lo as i32, hi as i32);
        let size = (rng.range(body.school[0] as i32, body.school[1] as i32) as u32).min(max);
        let half = body.box_size[0] / 2.0;
        let school = self.next_school;
        self.next_school += 1;
        let mut spawned = 0;
        for _ in 0..size {
            // Anywhere in the column with the box inside it.
            let px = f64::from(x) + half + (1.0 - 2.0 * half).max(0.0) * unit(rng);
            let pz = f64::from(z) + half + (1.0 - 2.0 * half).max(0.0) * unit(rng);
            let pos = [px, f64::from(SEA_LEVEL - depth), pz];
            let heading = f64::from(rng.angle()).to_degrees();
            if store.spawn(species, pos, heading, school).is_some() {
                spawned += 1;
            }
        }
        spawned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fish_columns::FishColumns;
    use crate::fish_entities::FishEntity;
    use layout::{Layout, RegionCache, RegionCoord};

    const DAY: u64 = 4800;

    fn spawner_with(config: FishSpawnConfig, seed: u64) -> (FishSpawner, FishStore) {
        let bodies = FishBodies::default_from_data().unwrap();
        let s = FishSpawner::new(
            config,
            PhaseClock::default_phases().unwrap(),
            Fish::shipped(),
            bodies.clone(),
            &Habitats::shipped(),
            seed,
        );
        (s, FishStore::new(bodies, 1000))
    }

    fn spawner(seed: u64) -> (FishSpawner, FishStore) {
        spawner_with(FishSpawnConfig::default_from_data().unwrap(), seed)
    }

    fn habitat(name: &str) -> HabitatId {
        Habitats::shipped().id_of(name).unwrap()
    }

    /// Open water everywhere, depth and temperature varying by column.
    fn open_sea(x: i32, z: i32) -> Option<FishColumn> {
        let h = (x.wrapping_mul(73) ^ z.wrapping_mul(151)).rem_euclid(97) as u32;
        let names = ["warm_shallows", "deep_water", "cold_open", "temperate_open"];
        Some(FishColumn {
            habitat: habitat(names[(h % 4) as usize]),
            temperature: f64::from(h % 10) / 10.0,
            depth: 2 + h % 40,
        })
    }

    fn ice(_: i32, _: i32) -> Option<FishColumn> {
        let habitat = habitat("frozen");
        Some(FishColumn {
            habitat,
            temperature: 0.0,
            depth: 30,
        })
    }

    /// Land on every third diagonal, ice west of x = 0, open sea elsewhere.
    fn patchy(x: i32, z: i32) -> Option<FishColumn> {
        if (x + z).rem_euclid(3) == 0 {
            return None;
        }
        if x < 0 {
            return ice(x, z);
        }
        open_sea(x, z)
    }

    fn near(store: &FishStore, raft: [f64; 2], r: f64) -> u32 {
        store.iter().filter(|f| dist(f.pos, raft) <= r).count() as u32
    }

    fn assert_placed(f: &FishEntity, mut columns: impl FnMut(i32, i32) -> Option<FishColumn>) {
        let (x, z) = (f.pos[0].floor() as i32, f.pos[2].floor() as i32);
        let col = columns(x, z).unwrap_or_else(|| panic!("{f:?} not in water"));
        assert_ne!(col.habitat, habitat("frozen"), "{f:?}");
        let fish = Fish::shipped();
        assert!(
            fish.species_for(&col).iter().any(|(s, _)| *s == f.species),
            "{f:?} in {col:?}"
        );
        let top = f.pos[1] + f.box_size[1];
        assert!(top <= f64::from(SEA_LEVEL) - 1.0, "{f:?}");
        assert!(
            f.pos[1] >= f64::from(SEA_LEVEL) - f64::from(col.depth) + 1.0,
            "{f:?}"
        );
    }

    #[test]
    fn spawns_only_on_phase_changes_within_target_and_stock() {
        let (mut sp, mut store) = spawner(7);
        let c = FishSpawnConfig::default_from_data().unwrap();
        let mut spawn_ticks = Vec::new();
        for tick in 0..3 * DAY {
            // Drift east so old fish fall behind and despawn.
            let raft = [tick as f64 * 0.02, 0.5];
            let before: Vec<_> = store.iter().map(|f| f.id).collect();
            sp.tick(tick, raft, &mut store, open_sea);
            let new = store.iter().filter(|f| !before.contains(&f.id)).count();
            if new > 0 {
                assert!(sp.is_phase_change(tick), "spawned at {tick}");
                spawn_ticks.push(tick);
            }
            assert!(near(&store, raft, c.target_radius) <= c.target_count);
            assert!(store.iter().all(|f| dist(f.pos, raft) <= c.despawn_radius));
            assert_eq!(sp.stock() + store.len() as u32, c.daily_stock, "{tick}");
        }
        assert_eq!(spawn_ticks[0], 0);
        assert!(spawn_ticks.len() >= 3, "{spawn_ticks:?}");
        assert!(spawn_ticks.iter().all(|t| t % (DAY / 2) == 0));
    }

    #[test]
    fn tops_up_to_target_and_no_more() {
        let (mut sp, mut store) = spawner(3);
        sp.tick(0, [0.0, 0.0], &mut store, open_sea);
        assert_eq!(store.len(), 12);
        assert_eq!(sp.stock(), 12);
        let ring = store
            .iter()
            .all(|f| (10.5..=25.5).contains(&dist(f.pos, [0.0; 2])));
        assert!(ring);
        let first = store.iter().next().unwrap().id;
        store.remove(first);
        sp.tick(2400, [0.0, 0.0], &mut store, open_sea);
        assert_eq!(store.len(), 12);
        assert_eq!(sp.stock(), 11, "a caught fish stays out of the stock");
    }

    #[test]
    fn stock_limits_spawns_and_despawns_return_to_it() {
        let text = include_str!("../../../data/fish_spawn.toml")
            .replace("daily_stock = 24", "daily_stock = 5");
        let (mut sp, mut store) = spawner_with(FishSpawnConfig::parse(&text).unwrap(), 3);
        sp.tick(0, [0.0, 0.0], &mut store, open_sea);
        assert_eq!((store.len(), sp.stock()), (5, 0));
        sp.tick(1, [100.0, 0.0], &mut store, open_sea);
        assert_eq!((store.len(), sp.stock()), (0, 5));
        sp.tick(2400, [100.0, 0.0], &mut store, open_sea);
        assert_eq!((store.len(), sp.stock()), (5, 0));
        sp.tick(4800, [100.0, 0.0], &mut store, open_sea);
        assert_eq!(store.len(), 5, "empty stock spawns nothing");
    }

    #[test]
    fn fish_only_in_open_water_of_an_allowed_species() {
        let mut total = 0;
        for seed in 0..20 {
            let (mut sp, mut store) = spawner(seed);
            sp.tick(0, [0.5, 0.5], &mut store, patchy);
            assert!(store.len() <= 12);
            total += store.len();
            for f in store.iter() {
                assert_placed(f, patchy);
            }
        }
        assert!(total > 100, "{total}");
        let (mut sp, mut store) = spawner(1);
        sp.tick(0, [0.5, 0.5], &mut store, ice);
        sp.tick(2400, [0.5, 0.5], &mut store, |_, _| None);
        assert!(store.is_empty());
        assert_eq!(sp.stock(), 24);
    }

    fn cache(coords: &[(i32, i32)]) -> RegionCache {
        let layout = Layout {
            seed: 25,
            ..Layout::default()
        };
        let mut cache = RegionCache::new(layout, false);
        for &(x, z) in coords {
            cache.get_or_generate(RegionCoord::new(x, z));
        }
        cache
    }

    fn run(
        cache: &RegionCache,
        cols: &mut FishColumns,
        raft: [f64; 2],
        seed: u64,
    ) -> Vec<FishEntity> {
        let (mut sp, mut store) = spawner(seed);
        for tick in [0, 2400, 4800] {
            sp.tick(tick, raft, &mut store, |x, z| cols.column(cache, x, z));
        }
        store.iter().cloned().collect()
    }

    #[test]
    fn real_water_fish_ignore_regions_beyond_the_raft_and_replay() {
        let a = cache(&[(0, 0), (1, 0), (0, 1), (1, 1)]);
        let b = cache(&[
            (0, 0),
            (1, 0),
            (0, 1),
            (1, 1),
            (2, 0),
            (2, 1),
            (-1, 1),
            (0, 2),
        ]);
        let (mut ca, mut cb) = (FishColumns::default(), FishColumns::default());
        let mut checked = 0;
        for z in (48..=208).step_by(40) {
            for x in (48..=208).step_by(40) {
                let raft = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let fish = run(&a, &mut ca, raft, 11);
                for f in &fish {
                    assert_placed(f, |x, z| ca.column(&a, x, z));
                }
                assert_eq!(fish, run(&b, &mut cb, raft, 11), "raft at {raft:?}");
                assert_eq!(fish, run(&a, &mut ca, raft, 11), "replay at {raft:?}");
                checked += usize::from(!fish.is_empty());
            }
        }
        assert!(checked > 0, "no raft position had fish");
    }
}
