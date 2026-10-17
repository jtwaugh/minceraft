//! Animals in the world: the animal entity type and the store that holds
//! them. See docs/design/basic-gameplay.md section 8.3.
//!
//! [`AnimalStore::tick`] moves them (gravity, landing, the deck push).
//! Wandering, damage, drops and spawning policy are separate tickets.

use crate::animals::Animals;
use crate::collision::{Aabb, CollisionWorld};
use crate::item_entities::{lift_out_of_blocks, Deck, ItemEntityConfig, TICKS_PER_SECOND};
use crate::movement::move_box;
use crate::targeting::{EntityId, TargetableEntity};

/// The `kind` animal entities report to rule resolution, so a rule target of
/// `entity:animal` matches them. The species is [`AnimalEntity::species`].
pub const ANIMAL_KIND: &str = "animal";

/// Horizontal speed below which a resting animal stops, blocks per second.
const REST_SPEED: f64 = 1e-3;

/// A cow, pig or chicken.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimalEntity {
    pub id: EntityId,
    /// Key into `data/animals.toml`, e.g. `cow`.
    pub species: String,
    pub health: u32,
    /// Centre of the bottom face of the box, in world coordinates.
    pub pos: [f64; 3],
    /// Blocks per second.
    pub velocity: [f64; 3],
    /// Yaw in degrees.
    pub facing: f64,
    /// Box width (x and z) and height (y), from the species.
    pub box_size: [f64; 2],
}

impl AnimalEntity {
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

/// All animals. Ids are never reused within a store. They share one space
/// with every other entity the eye ray can target; `first_id` lets the
/// caller start this store above the others.
#[derive(Clone, Debug)]
pub struct AnimalStore {
    kinds: Animals,
    /// Gravity, drag and ground friction are shared with item entities.
    physics: ItemEntityConfig,
    next_id: EntityId,
    // Kept sorted by id (ids only grow), so lookups can bisect.
    animals: Vec<AnimalEntity>,
}

impl AnimalStore {
    pub fn new(kinds: Animals, physics: ItemEntityConfig, first_id: EntityId) -> AnimalStore {
        AnimalStore {
            kinds,
            physics,
            next_id: first_id,
            animals: Vec::new(),
        }
    }

    /// Add an animal of `species` at full health, or `None` if there is no
    /// such species in `data/animals.toml`.
    pub fn spawn(&mut self, species: &str, pos: [f64; 3], facing: f64) -> Option<EntityId> {
        let def = self.kinds.get(species)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("animal ids exhausted");
        self.animals.push(AnimalEntity {
            id,
            species: species.to_string(),
            health: def.health,
            pos,
            velocity: [0.0; 3],
            facing,
            box_size: def.box_size,
        });
        Some(id)
    }

    pub fn get(&self, id: EntityId) -> Option<&AnimalEntity> {
        let i = self.animals.binary_search_by_key(&id, |e| e.id).ok()?;
        Some(&self.animals[i])
    }

    pub fn remove(&mut self, id: EntityId) -> Option<AnimalEntity> {
        let i = self.animals.binary_search_by_key(&id, |e| e.id).ok()?;
        Some(self.animals.remove(i))
    }

    pub fn len(&self) -> usize {
        self.animals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.animals.is_empty()
    }

    /// Animals in id order.
    pub fn iter(&self) -> impl Iterator<Item = &AnimalEntity> {
        self.animals.iter()
    }

    /// Advance every animal one tick, in id order, against `world` (which
    /// should include the raft's boxes where there is one). Same motion as
    /// items: the deck push, gravity, a swept move, drag and ground friction.
    pub fn tick(&mut self, world: &impl CollisionWorld, deck: Option<Deck<'_>>) {
        let cfg = self.physics;
        let dt = 1.0 / TICKS_PER_SECOND;
        let mut scratch = Vec::new();
        for e in &mut self.animals {
            let mut bb = e.aabb();
            lift_out_of_blocks(world, &mut bb, &mut e.pos, &mut scratch);
            if let Some(d) = &deck {
                if d.raft.supports(&bb) {
                    let [px, pz] = d.push.item_push(d.tilt);
                    e.velocity[0] += px * TICKS_PER_SECOND;
                    e.velocity[2] += pz * TICKS_PER_SECOND;
                }
            }
            e.velocity[1] -= cfg.gravity * dt;
            let v = e.velocity.map(|c| c * dt);
            let m = move_box(world, &bb, v);
            for i in 0..3 {
                e.pos[i] += m.applied[i];
                if m.applied[i] != v[i] {
                    e.velocity[i] = 0.0;
                }
                e.velocity[i] *= cfg.drag;
            }
            if m.on_ground {
                for i in [0, 2] {
                    e.velocity[i] *= cfg.ground_friction;
                    if e.velocity[i].abs() < REST_SPEED {
                        e.velocity[i] = 0.0;
                    }
                }
            }
        }
    }

    /// The animals as targets, of kind `animal`, for `cast_target`.
    pub fn targetables(&self) -> Vec<TargetableEntity> {
        self.animals
            .iter()
            .map(|e| TargetableEntity {
                id: e.id,
                kind: ANIMAL_KIND.to_string(),
                aabb: e.aabb(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::raft::{Raft, RaftWorld, SwellPush};
    use crate::targeting::{cast_target, Target, TargetingConfig};
    use terrain::Block;

    fn store(first_id: EntityId) -> AnimalStore {
        AnimalStore::new(
            Animals::default_animals(),
            ItemEntityConfig::default_from_data(),
            first_id,
        )
    }

    fn floor() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -8..8 {
            for z in -8..8 {
                w.set(x, 0, z, Block::Stone);
            }
        }
        w
    }

    fn run(s: &mut AnimalStore, w: &impl CollisionWorld, n: usize) {
        for _ in 0..n {
            s.tick(w, None);
        }
    }

    #[test]
    fn spawn_uses_the_species_numbers() {
        let mut s = store(1);
        let id = s.spawn("chicken", [0.5, 4.0, 0.5], 90.0).unwrap();
        let e = s.get(id).unwrap();
        assert_eq!((e.health, e.facing), (4, 90.0));
        assert_eq!(
            e.box_size,
            Animals::default_animals().get("chicken").unwrap().box_size
        );
        assert!(s.spawn("dragon", [0.0; 3], 0.0).is_none());
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn spawned_above_ground_settles_and_rests() {
        let w = floor();
        let mut s = store(1);
        let id = s.spawn("cow", [0.5, 5.0, 0.5], 0.0).unwrap();
        s.tick(&w, None);
        assert!(s.get(id).unwrap().pos[1] < 5.0, "falls");
        run(&mut s, &w, 100);
        let e = s.get(id).unwrap();
        assert!((e.pos[1] - 1.0).abs() < 1e-6, "rests on top: {}", e.pos[1]);
        assert_eq!(e.velocity, [0.0; 3]);
        let before = e.pos;
        run(&mut s, &w, 20);
        assert_eq!(s.get(id).unwrap().pos, before);
    }

    #[test]
    fn animal_on_the_deck_is_carried_like_an_item() {
        let mut base = MemoryWorld::new();
        let mut cells = Vec::new();
        for x in 0..4 {
            for z in 0..4 {
                base.set(x, 64, z, Block::Stone);
                cells.push([x, 64, z]);
            }
        }
        let raft = Raft::new(&cells);
        let world = RaftWorld::new(&base, &raft);
        let push = SwellPush::default_from_data().unwrap();
        let mut s = store(1);
        let id = s.spawn("pig", [2.0, 65.0, 2.0], 0.0).unwrap();
        run(&mut s, &world, 5);
        let y = s.get(id).unwrap().pos[1];
        assert!((y - 65.0).abs() < 1e-6, "rests on the deck: {y}");
        for _ in 0..10 {
            let deck = Deck {
                raft: &raft,
                tilt: [0.0, 3.0],
                push: &push,
            };
            s.tick(&world, Some(deck));
        }
        let e = s.get(id).unwrap();
        assert!(e.pos[0] > 2.0, "slides along the deck: {}", e.pos[0]);
        assert!((e.pos[1] - y).abs() < 1e-9);
    }

    #[test]
    fn eye_ray_targets_an_animal_and_the_attack_rule_resolves() {
        use crate::interactions::{Button, HeldThing, Interactions, ResolveCtx, TargetFacts};
        let mut s = store(1);
        let id = s.spawn("cow", [2.5, 0.0, 0.5], 0.0).unwrap();
        let es = s.targetables();
        assert_eq!(es[0].kind, ANIMAL_KIND);
        let cfg = TargetingConfig::default_from_data().unwrap();
        let w = MemoryWorld::new();
        let target = cast_target(&w, &es, &cfg, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0]);
        assert_eq!(target, Target::Entity { id });
        let rules = Interactions::from_toml(
            "[[rule]]\nheld='tool:sword'\nbutton='primary'\ntarget='entity:animal'\naction='attack'\n",
        )
        .unwrap();
        let ctx = ResolveCtx {
            held: HeldThing::Tool("sword".into()),
            button: Button::Primary,
            target,
            facts: TargetFacts {
                entity: Some(es[0].kind.clone()),
                ..TargetFacts::default()
            },
        };
        assert!(crate::interactions::resolve(&rules.rules, &ctx).is_some());
    }

    #[test]
    fn ids_start_at_first_id_and_are_not_reused() {
        let mut s = store(1000);
        let a = s.spawn("cow", [0.0; 3], 0.0).unwrap();
        let b = s.spawn("pig", [1.0, 0.0, 0.0], 0.0).unwrap();
        assert_eq!((a, b), (1000, 1001));
        s.remove(a);
        assert_eq!(s.spawn("cow", [0.0; 3], 0.0), Some(1002));
    }

    #[test]
    fn the_same_spawns_give_the_same_positions() {
        let w = floor();
        let go = || {
            let mut s = store(1);
            s.spawn("cow", [0.5, 3.0, 0.5], 10.0);
            s.spawn("chicken", [2.5, 6.0, 1.5], 20.0);
            run(&mut s, &w, 80);
            s.iter().map(|e| (e.pos, e.velocity)).collect::<Vec<_>>()
        };
        assert_eq!(go(), go());
    }
}
