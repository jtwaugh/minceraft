//! Fish in the water: the fish entity type and the store that holds them.
//! See docs/design/fish-and-fishing.md sections 2 and 3.2.
//!
//! The store only holds fish. Spawning, swimming, scatter and catches are
//! separate tickets. Fish are never saved (3.1).

use crate::collision::Aabb;
use crate::fish_bodies::FishBodies;
use crate::targeting::EntityId;

/// Which school a fish belongs to; members steer towards its centre (3.2).
pub type SchoolId = u32;

/// One fish.
#[derive(Clone, Debug, PartialEq)]
pub struct FishEntity {
    pub id: EntityId,
    /// Key into `data/fish.toml`, e.g. `mackerel`.
    pub species: String,
    /// Centre of the bottom face of the box, in world coordinates.
    pub pos: [f64; 3],
    /// Yaw in degrees.
    pub heading: f64,
    /// The height the fish is swimming towards; starts at the spawn height.
    pub depth_target: f64,
    pub school: SchoolId,
    /// Box width (x and z) and height (y), from `data/fish_bodies.toml`.
    pub box_size: [f64; 2],
    /// Half-hearts of damage left; the fish is removed at 0.
    pub health: u32,
}

impl FishEntity {
    pub fn aabb(&self) -> Aabb {
        let h = self.box_size[0] / 2.0;
        Aabb::new(
            [self.pos[0] - h, self.pos[1], self.pos[2] - h],
            [
                self.pos[0] + h,
                self.pos[1] + self.box_size[1],
                self.pos[2] + h,
            ],
        )
    }
}

/// All fish. Ids are never reused within a store. They share one space with
/// every other entity the eye ray can target; `first_id` lets the caller
/// start this store above the others.
#[derive(Clone, Debug)]
pub struct FishStore {
    bodies: FishBodies,
    next_id: EntityId,
    // Kept sorted by id (ids only grow), so lookups can bisect.
    fish: Vec<FishEntity>,
}

impl FishStore {
    pub fn new(bodies: FishBodies, first_id: EntityId) -> FishStore {
        FishStore {
            bodies,
            next_id: first_id,
            fish: Vec::new(),
        }
    }

    /// Add a fish of `species` in `school`, or `None` if the species has no
    /// body in `data/fish_bodies.toml`.
    pub fn spawn(
        &mut self,
        species: &str,
        pos: [f64; 3],
        heading: f64,
        school: SchoolId,
    ) -> Option<EntityId> {
        let body = self.bodies.get(species)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("fish ids exhausted");
        self.fish.push(FishEntity {
            id,
            species: species.to_string(),
            pos,
            heading,
            depth_target: pos[1],
            school,
            box_size: body.box_size,
            health: body.health,
        });
        Some(id)
    }

    /// Remove every fish. Ids are not reused.
    pub fn clear(&mut self) {
        self.fish.clear();
    }

    pub fn get(&self, id: EntityId) -> Option<&FishEntity> {
        let i = self.fish.binary_search_by_key(&id, |f| f.id).ok()?;
        Some(&self.fish[i])
    }

    pub fn remove(&mut self, id: EntityId) -> Option<FishEntity> {
        let i = self.fish.binary_search_by_key(&id, |f| f.id).ok()?;
        Some(self.fish.remove(i))
    }

    /// Hit fish `id` for `damage` half-hearts. Returns the damage it took
    /// (capped at its health) and whether it died, in which case it is
    /// removed. `None` for an unknown id.
    pub fn hit(&mut self, id: EntityId, damage: u32) -> Option<(u32, bool)> {
        let i = self.fish.binary_search_by_key(&id, |f| f.id).ok()?;
        let taken = damage.min(self.fish[i].health);
        self.fish[i].health -= taken;
        let died = self.fish[i].health == 0;
        if died {
            self.fish.remove(i);
        }
        Some((taken, died))
    }

    pub fn len(&self) -> usize {
        self.fish.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fish.is_empty()
    }

    /// Fish in id order.
    pub fn iter(&self) -> impl Iterator<Item = &FishEntity> {
        self.fish.iter()
    }

    /// Fish in id order, for the swim tick. Callers must not change `id`.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut FishEntity> {
        self.fish.iter_mut()
    }

    pub fn bodies(&self) -> &FishBodies {
        &self.bodies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(first_id: EntityId) -> FishStore {
        FishStore::new(FishBodies::default_from_data().unwrap(), first_id)
    }

    fn species() -> Vec<String> {
        let b = FishBodies::default_from_data().unwrap();
        b.bodies.iter().map(|b| b.species.clone()).collect()
    }

    fn ids(s: &FishStore) -> Vec<EntityId> {
        s.iter().map(|f| f.id).collect()
    }

    #[test]
    fn spawn_fills_in_the_body() {
        let bodies = FishBodies::default_from_data().unwrap();
        let mut s = store(100);
        let sp = &species()[0];
        let id = s.spawn(sp, [1.5, 50.0, -3.5], 90.0, 7).unwrap();
        assert_eq!(id, 100);
        let f = s.get(id).unwrap();
        assert_eq!(&f.species, sp);
        assert_eq!((f.pos, f.heading, f.school), ([1.5, 50.0, -3.5], 90.0, 7));
        assert_eq!(f.depth_target, 50.0);
        assert_eq!(f.box_size, bodies.get(sp).unwrap().box_size);
        assert_eq!(f.health, bodies.get(sp).unwrap().health);
        let b = f.aabb();
        assert_eq!(b.min[1], 50.0);
        assert!((b.max[1] - 50.0 - f.box_size[1]).abs() < 1e-12);
        assert_eq!((s.len(), s.is_empty()), (1, false));
    }

    #[test]
    fn unknown_species_is_refused() {
        let mut s = store(0);
        assert_eq!(s.spawn("whale", [0.0; 3], 0.0, 0), None);
        assert_eq!(s.spawn("", [0.0; 3], 0.0, 0), None);
        assert!(s.is_empty());
        // A refused spawn uses no id.
        assert_eq!(s.spawn(&species()[0], [0.0; 3], 0.0, 0), Some(0));
    }

    #[test]
    fn ids_follow_spawn_order_and_survive_remove() {
        let all = species();
        let mut s = store(10);
        for (i, sp) in all.iter().cycle().take(6).enumerate() {
            let id = s.spawn(sp, [i as f64, 40.0, 0.0], 0.0, i as u32 / 3);
            assert_eq!(id, Some(10 + i as EntityId));
        }
        assert_eq!(ids(&s), [10, 11, 12, 13, 14, 15]);
        let gone = s.remove(12).unwrap();
        assert_eq!(gone.pos[0], 2.0);
        assert_eq!(s.remove(12), None);
        assert_eq!(s.get(12), None);
        assert_eq!(s.get(13).unwrap().pos[0], 3.0);
        assert_eq!(ids(&s), [10, 11, 13, 14, 15]);
        // Removed ids are not reused.
        assert_eq!(s.spawn(&all[0], [0.0; 3], 0.0, 9), Some(16));
        assert_eq!(ids(&s), [10, 11, 13, 14, 15, 16]);
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.iter().count(), 0);
        assert_eq!(s.spawn(&all[0], [0.0; 3], 0.0, 9), Some(17));
    }

    #[test]
    fn same_spawns_give_same_store() {
        let run = || {
            let mut s = store(0);
            for (i, sp) in species().iter().enumerate() {
                s.spawn(sp, [i as f64, 30.0, 1.0], 45.0, 2);
            }
            s.remove(1);
            s.iter().cloned().collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn hits_wound_then_kill() {
        let mut s = store(5);
        let (a, b) = (species()[0].clone(), species()[1].clone());
        let (id, other) = (
            s.spawn(&a, [0.0; 3], 0.0, 0).unwrap(),
            s.spawn(&b, [1.0; 3], 0.0, 0).unwrap(),
        );
        let health = s.get(id).unwrap().health;
        assert!(health >= 2);
        assert_eq!(s.hit(id, 1), Some((1, false)));
        assert_eq!(s.get(id).unwrap().health, health - 1);
        assert_eq!(s.hit(id, 0), Some((0, false)));
        assert_eq!(s.hit(id, 100), Some((health - 1, true)));
        assert_eq!(s.get(id), None);
        assert_eq!(s.hit(id, 1), None);
        assert_eq!(s.hit(99, 1), None);
        assert_eq!(ids(&s), [other]);
    }
}
