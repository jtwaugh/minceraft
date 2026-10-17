//! Animals in the world: the animal entity type and the store that holds
//! them. See docs/design/basic-gameplay.md section 8.3.
//!
//! [`AnimalStore::tick`] moves them (gravity, landing, the deck push) and
//! makes idle animals wander: pause, pick a heading, walk, repeat (8.2). A
//! hit animal flees along a random heading for a few seconds, then wanders.
//! Damage, drops and spawning policy are separate tickets.

use crate::animals::{AnimalDef, Animals};
use crate::collision::{Aabb, CollisionWorld};
use crate::item_entities::{lift_out_of_blocks, Deck, ItemEntityConfig, TICKS_PER_SECOND};
use crate::movement::move_with_step;
use crate::physics::PlayerPhysics;
use crate::rng::Rng;
use crate::targeting::{EntityId, TargetableEntity};

/// The `kind` animal entities report to rule resolution, so a rule target of
/// `entity:animal` matches them. The species is [`AnimalEntity::species`].
pub const ANIMAL_KIND: &str = "animal";

/// Horizontal speed below which a resting animal stops, blocks per second.
const REST_SPEED: f64 = 1e-3;

/// Where an animal is in its pause/walk cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wander {
    pub walking: bool,
    pub ticks_left: u32,
}

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
    pub wander: Wander,
    /// Ticks left in the invulnerability window after a hit; 0 when open.
    /// The red flash lasts as long.
    pub hurt_ticks_left: u32,
    /// Damage of the hit that opened the window.
    pub last_damage: u32,
    /// Ticks of fleeing left after a hit, along `facing`; 0 when not fleeing.
    pub flee_ticks_left: u32,
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

/// Count down the pause or walk; when it runs out, switch. Starting a walk
/// picks a heading and a duration. Returns whether the animal walks this tick.
fn advance_wander(w: &mut Wander, facing: &mut f64, def: &AnimalDef, rng: &mut Rng) -> bool {
    if w.ticks_left == 0 {
        w.walking = !w.walking;
        let [lo, hi] = if w.walking {
            *facing = f64::from(rng.angle()).to_degrees();
            def.walk_ticks
        } else {
            def.pause_ticks
        };
        w.ticks_left = rng.range(lo as i32, hi as i32) as u32;
    }
    w.ticks_left -= 1;
    w.walking
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
    rng: Rng,
    /// Step-up height for walking, as for the player (4.4).
    step_height: f64,
    // Kept sorted by id (ids only grow), so lookups can bisect.
    animals: Vec<AnimalEntity>,
}

impl AnimalStore {
    /// `seed` drives every wander choice, so a run replays from it.
    pub fn new(
        kinds: Animals,
        physics: ItemEntityConfig,
        first_id: EntityId,
        seed: u64,
    ) -> AnimalStore {
        AnimalStore {
            kinds,
            physics,
            next_id: first_id,
            rng: Rng::new(seed),
            step_height: f64::from(
                PlayerPhysics::default_from_data()
                    .expect("data/player.toml is valid")
                    .step_height,
            ),
            animals: Vec::new(),
        }
    }

    /// Add an animal of `species` at full health, or `None` if there is no
    /// such species in `data/animals.toml`.
    pub fn spawn(&mut self, species: &str, pos: [f64; 3], facing: f64) -> Option<EntityId> {
        let def = self.kinds.get(species)?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("animal ids exhausted");
        let wander = Wander {
            walking: false,
            ticks_left: self
                .rng
                .range(def.pause_ticks[0] as i32, def.pause_ticks[1] as i32)
                as u32,
        };
        self.animals.push(AnimalEntity {
            id,
            species: species.to_string(),
            health: def.health,
            pos,
            velocity: [0.0; 3],
            facing,
            box_size: def.box_size,
            wander,
            hurt_ticks_left: 0,
            last_damage: 0,
            flee_ticks_left: 0,
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

    /// Hit animal `id` for `damage` half-hearts (8.1, 8.2). Outside the
    /// invulnerability window the hit applies in full and opens the window.
    /// Inside it, a hit is ignored unless it exceeds the last hit, and then
    /// only the difference applies. Returns the health taken off (0 if
    /// ignored), or `None` if there is no such animal. A hit that opens the
    /// window also knocks the animal away from `attacker` (a world position)
    /// with a small kick upward; the move itself happens in `tick`, through
    /// the collision resolver. No death handling here: health just stops at 0.
    pub fn hit(&mut self, id: EntityId, damage: u32, attacker: [f64; 3]) -> Option<u32> {
        let i = self.animals.binary_search_by_key(&id, |e| e.id).ok()?;
        let e = &mut self.animals[i];
        let applied = if e.hurt_ticks_left == 0 {
            let def = self
                .kinds
                .get(&e.species)
                .expect("species checked at spawn");
            e.hurt_ticks_left = def.hurt_ticks;
            let (dx, dz) = (e.pos[0] - attacker[0], e.pos[2] - attacker[2]);
            let len = dx.hypot(dz);
            // Attacker straight above or below: push along the facing.
            let (ux, uz) = if len > 1e-9 {
                (dx / len, dz / len)
            } else {
                let (sin, cos) = e.facing.to_radians().sin_cos();
                (sin, -cos)
            };
            e.velocity[0] = ux * def.knockback_speed;
            e.velocity[2] = uz * def.knockback_speed;
            e.velocity[1] = def.knockback_lift;
            damage
        } else {
            damage.saturating_sub(e.last_damage)
        };
        if applied > 0 {
            e.last_damage = damage;
            // Flee in a random direction; a new hit re-picks and restarts.
            // Wandering resumes after the flee with a pause.
            let def = self
                .kinds
                .get(&e.species)
                .expect("species checked at spawn");
            e.facing = f64::from(self.rng.angle()).to_degrees();
            e.flee_ticks_left = self
                .rng
                .range(def.flee_ticks[0] as i32, def.flee_ticks[1] as i32)
                as u32;
            e.wander = Wander {
                walking: true,
                ticks_left: 0,
            };
        }
        let taken = applied.min(e.health);
        e.health -= taken;
        Some(taken)
    }

    /// Advance every animal one tick, in id order, against `world` (which
    /// should include the raft's boxes where there is one). Same motion as
    /// items: the deck push, gravity, a swept move, drag and ground friction.
    pub fn tick(&mut self, world: &impl CollisionWorld, deck: Option<Deck<'_>>) {
        let cfg = self.physics;
        let dt = 1.0 / TICKS_PER_SECOND;
        let mut scratch = Vec::new();
        for e in &mut self.animals {
            e.hurt_ticks_left = e.hurt_ticks_left.saturating_sub(1);
            let def = self
                .kinds
                .get(&e.species)
                .expect("species checked at spawn");
            let fleeing = e.flee_ticks_left > 0;
            e.flee_ticks_left = e.flee_ticks_left.saturating_sub(1);
            let walking =
                !fleeing && advance_wander(&mut e.wander, &mut e.facing, def, &mut self.rng);
            let mut bb = e.aabb();
            lift_out_of_blocks(world, &mut bb, &mut e.pos, &mut scratch);
            if let Some(d) = &deck {
                if d.raft.supports(&bb) {
                    let [px, pz] = d.push.item_push(d.tilt);
                    e.velocity[0] += px * TICKS_PER_SECOND;
                    e.velocity[2] += pz * TICKS_PER_SECOND;
                }
            }
            // The wander drive would overwrite a knockback, so it waits.
            if (walking || fleeing) && e.hurt_ticks_left == 0 {
                let speed = if fleeing {
                    def.flee_speed
                } else {
                    def.wander_speed
                };
                // Facing 0 looks along -z, as for the player's yaw.
                let (sin, cos) = e.facing.to_radians().sin_cos();
                e.velocity[0] = sin * speed;
                e.velocity[2] = -cos * speed;
            }
            e.velocity[1] -= cfg.gravity * dt;
            let v = e.velocity.map(|c| c * dt);
            let on_ground = e.velocity[1] == 0.0;
            let m = move_with_step(world, &bb, v, self.step_height, on_ground);
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
            7,
        )
    }

    /// A stone floor at y=0 inside a two-high wall ring at +-3, so an
    /// animal can never leave.
    fn floor() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -8..8 {
            for z in -8..8 {
                w.set(x, 0, z, Block::Stone);
                if x.abs().max(z.abs()) == 3 {
                    w.set(x, 1, z, Block::Stone);
                    w.set(x, 2, z, Block::Stone);
                }
            }
        }
        w
    }

    const ORIGIN: [f64; 3] = [0.0; 3];

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
        assert_eq!(e.velocity[1], 0.0);
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

    fn wander_run(seed: u64, ticks: usize) -> Vec<([f64; 3], bool)> {
        let w = floor();
        let mut s = AnimalStore::new(
            Animals::default_animals(),
            ItemEntityConfig::default_from_data(),
            1,
            seed,
        );
        let id = s.spawn("cow", [0.5, 1.0, 0.5], 0.0).unwrap();
        let mut trace = Vec::new();
        for _ in 0..ticks {
            s.tick(&w, None);
            let e = s.get(id).unwrap();
            trace.push((e.pos, e.wander.walking));
        }
        trace
    }

    #[test]
    fn wandering_moves_pauses_and_stays_on_the_floor() {
        let trace = wander_run(3, 2000);
        assert_ne!(trace[0].0, trace[199].0, "moves within 200 ticks");
        assert!(trace.iter().any(|t| t.1) && trace.iter().any(|t| !t.1));
        for (p, _) in &trace {
            assert!((p[1] - 1.0).abs() < 1e-6, "stays on the floor: {p:?}");
            assert!(
                p[0].abs() < 3.0 && p[2].abs() < 3.0,
                "inside the ring: {p:?}"
            );
        }
    }

    #[test]
    fn pauses_leave_the_animal_still_part_of_the_time() {
        let trace = wander_run(5, 600);
        let moved = trace.windows(2).filter(|p| p[0].0 != p[1].0).count();
        assert!(moved > 0 && moved < 600);
    }

    #[test]
    fn wandering_never_enters_a_full_block() {
        let w = floor();
        let mut s = store(1);
        let id = s.spawn("pig", [0.5, 1.0, 0.5], 0.0).unwrap();
        let half = s.get(id).unwrap().box_size[0] / 2.0;
        for _ in 0..3000 {
            s.tick(&w, None);
            let p = s.get(id).unwrap().pos;
            assert!(
                p[0].abs() + half <= 3.0 && p[2].abs() + half <= 3.0,
                "{p:?}"
            );
        }
    }

    fn sword() -> u32 {
        crate::items::Items::default_items().damage(Some("sword"))
    }

    #[test]
    fn damage_comes_from_the_item_table() {
        let items = crate::items::Items::default_items();
        assert_eq!(items.damage(None), 1);
        assert_eq!(items.damage(Some("sword")), 4);
        assert_eq!(items.damage(Some("hoe")), 1, "no damage set: hand");
    }

    #[test]
    fn cow_takes_three_sword_hits_and_chicken_one() {
        let mut s = store(1);
        let cow = s.spawn("cow", [0.0; 3], 0.0).unwrap();
        let hen = s.spawn("chicken", [1.0, 0.0, 0.0], 0.0).unwrap();
        let w = MemoryWorld::new();
        let window = Animals::default_animals().get("cow").unwrap().hurt_ticks;
        for want in [6, 2, 0] {
            assert!(s.hit(cow, sword(), ORIGIN).is_some());
            assert_eq!(s.get(cow).unwrap().health, want);
            run(&mut s, &w, window as usize);
        }
        assert_eq!(s.hit(hen, sword(), ORIGIN), Some(4));
        assert_eq!(s.get(hen).unwrap().health, 0);
        assert_eq!(s.hit(999, 1, ORIGIN), None);
    }

    #[test]
    fn weaker_hit_in_the_window_is_ignored_and_stronger_applies_the_difference() {
        let mut s = store(1);
        let id = s.spawn("cow", [0.0; 3], 0.0).unwrap();
        assert_eq!(s.hit(id, 4, ORIGIN), Some(4));
        assert_eq!(s.hit(id, 4, ORIGIN), Some(0));
        assert_eq!(s.hit(id, 1, ORIGIN), Some(0));
        assert_eq!(s.get(id).unwrap().health, 6);
        assert_eq!(s.hit(id, 6, ORIGIN), Some(2));
        assert_eq!(s.get(id).unwrap().health, 4);
        assert_eq!(
            s.hit(id, 6, ORIGIN),
            Some(0),
            "difference is against the new last"
        );
    }

    #[test]
    fn the_window_lasts_exactly_n_ticks() {
        let w = MemoryWorld::new();
        let n = Animals::default_animals().get("pig").unwrap().hurt_ticks;
        let mut s = store(1);
        let id = s.spawn("pig", [0.0; 3], 0.0).unwrap();
        s.hit(id, 1, ORIGIN);
        run(&mut s, &w, n as usize - 1);
        assert_eq!(
            s.hit(id, 1, ORIGIN),
            Some(0),
            "still inside after n-1 ticks"
        );
        s.tick(&w, None);
        assert_eq!(s.get(id).unwrap().hurt_ticks_left, 0);
        assert_eq!(s.hit(id, 1, ORIGIN), Some(1), "open after n ticks");
    }

    #[test]
    fn health_stops_at_zero() {
        let mut s = store(1);
        let id = s.spawn("chicken", [0.0; 3], 0.0).unwrap();
        assert_eq!(s.hit(id, 100, ORIGIN), Some(4));
        assert_eq!(s.get(id).unwrap().health, 0);
    }

    #[test]
    fn same_seed_same_wander_and_other_seeds_differ() {
        assert_eq!(wander_run(9, 500), wander_run(9, 500));
        assert_ne!(wander_run(9, 500), wander_run(10, 500));
    }

    #[test]
    fn a_hit_pushes_away_and_up() {
        let w = floor();
        for (attacker, dir) in [
            ([-1.0, 1.0, 0.5], [1.0, 0.0]),
            ([0.5, 1.0, 2.0], [0.0, -1.0]),
        ] {
            let mut s = store(1);
            let id = s.spawn("cow", [0.5, 1.0, 0.5], 0.0).unwrap();
            run(&mut s, &w, 30);
            let before = s.get(id).unwrap().pos;
            s.hit(id, 1, attacker);
            assert!(s.get(id).unwrap().hurt_ticks_left > 0);
            let mut top = before[1];
            for _ in 0..6 {
                s.tick(&w, None);
                top = top.max(s.get(id).unwrap().pos[1]);
            }
            let after = s.get(id).unwrap().pos;
            assert!(top > before[1] + 0.05, "gains height");
            let moved = [after[0] - before[0], after[2] - before[2]];
            assert!(moved[0] * dir[0] + moved[1] * dir[1] > 0.1, "{moved:?}");
        }
    }

    #[test]
    fn an_ignored_hit_gives_no_knockback_and_the_window_closes() {
        let w = floor();
        let mut s = store(1);
        let id = s.spawn("cow", [0.5, 1.0, 0.5], 0.0).unwrap();
        run(&mut s, &w, 30);
        s.hit(id, 4, [-1.0, 1.0, 0.5]);
        s.tick(&w, None);
        let v = s.get(id).unwrap().velocity;
        s.hit(id, 4, [0.5, 1.0, 3.0]);
        assert_eq!(s.get(id).unwrap().velocity, v, "unchanged");
        let n = Animals::default_animals().get("cow").unwrap().hurt_ticks;
        run(&mut s, &w, n as usize);
        assert_eq!(s.get(id).unwrap().hurt_ticks_left, 0);
    }

    #[test]
    fn knockback_into_a_wall_stops_at_the_wall() {
        let w = floor();
        let mut s = store(1);
        // Wall block at x=3 starts at x=3.0; the cow is 0.9 wide.
        let id = s.spawn("cow", [2.4, 1.0, 0.5], 0.0).unwrap();
        run(&mut s, &w, 30);
        s.hit(id, 1, [0.0, 1.0, 0.5]);
        run(&mut s, &w, 30);
        let e = s.get(id).unwrap();
        assert!(e.pos[0] + 0.45 <= 3.0 + 1e-9, "inside: {}", e.pos[0]);
    }

    #[test]
    fn knockback_is_deterministic() {
        let go = || {
            let w = floor();
            let mut s = store(1);
            let id = s.spawn("pig", [0.5, 1.0, 0.5], 0.0).unwrap();
            s.hit(id, 1, [-1.0, 1.0, 0.0]);
            run(&mut s, &w, 40);
            s.get(id).unwrap().clone()
        };
        assert_eq!(go(), go());
    }

    /// Hit `id` (which stands alone on an open floor) and trace its position
    /// each tick for `n` ticks after the knockback window.
    fn flee_run(seed: u64, n: usize) -> (Vec<[f64; 3]>, AnimalStore, EntityId) {
        let w = floor();
        let mut s = AnimalStore::new(
            Animals::default_animals(),
            ItemEntityConfig::default_from_data(),
            1,
            seed,
        );
        let id = s.spawn("cow", [0.5, 1.0, 0.5], 0.0).unwrap();
        run(&mut s, &w, 30);
        s.hit(id, 1, [0.5, 1.0, 0.5]);
        let mut trace = Vec::new();
        for _ in 0..n {
            s.tick(&w, None);
            trace.push(s.get(id).unwrap().pos);
        }
        (trace, s, id)
    }

    #[test]
    fn flee_speeds_are_faster_than_wandering_and_slower_than_the_player() {
        // The player's walking speed is about 4.3 blocks per second.
        for a in &Animals::default_animals().kinds {
            assert!(a.flee_speed > a.wander_speed, "{}", a.kind);
            assert!(a.flee_speed < 4.0, "{}", a.kind);
        }
    }

    #[test]
    fn a_hit_animal_runs_one_way_at_flee_speed_then_wanders() {
        let def = Animals::default_animals().get("cow").unwrap().clone();
        let (trace, s, id) = flee_run(4, 200);
        let flee = s.get(id).unwrap().flee_ticks_left;
        assert_eq!(flee, 0, "flee ended");
        // Past the hurt window, early in the flee: speed is the flee speed.
        let step = |i: usize| {
            let (a, b) = (trace[i], trace[i + 1]);
            [b[0] - a[0], b[2] - a[2]]
        };
        let start = def.hurt_ticks as usize + 5;
        let v = step(start);
        let speed = v[0].hypot(v[1]) * TICKS_PER_SECOND;
        // The ring wall may be in the way, so only bound it from above, and
        // require it to beat the wander speed when unobstructed.
        assert!(speed <= def.flee_speed + 1e-6, "{speed}");
        let (mut peak, mut dirs) = (0.0f64, Vec::new());
        for i in start..start + 20 {
            let v = step(i);
            peak = peak.max(v[0].hypot(v[1]) * TICKS_PER_SECOND);
            if v[0].hypot(v[1]) > 1e-4 {
                dirs.push(v[1].atan2(v[0]));
            }
        }
        assert!(peak > def.wander_speed, "peak {peak}");
        // One direction: no turning while fleeing.
        assert!(dirs.windows(2).all(|d| (d[0] - d[1]).abs() < 1e-3));
    }

    #[test]
    fn the_flee_lasts_within_the_configured_range() {
        let def = Animals::default_animals().get("pig").unwrap().clone();
        let w = floor();
        let mut s = store(1);
        let id = s.spawn("pig", [0.5, 1.0, 0.5], 0.0).unwrap();
        s.hit(id, 1, ORIGIN);
        let left = s.get(id).unwrap().flee_ticks_left;
        assert!((def.flee_ticks[0]..=def.flee_ticks[1]).contains(&left));
        run(&mut s, &w, left as usize - 1);
        assert_eq!(s.get(id).unwrap().flee_ticks_left, 1);
        s.tick(&w, None);
        assert_eq!(s.get(id).unwrap().flee_ticks_left, 0);
    }

    #[test]
    fn a_new_hit_while_fleeing_restarts_the_timer() {
        let w = floor();
        let mut s = store(1);
        let id = s.spawn("cow", [0.5, 1.0, 0.5], 0.0).unwrap();
        s.hit(id, 1, ORIGIN);
        let first = s.get(id).unwrap().flee_ticks_left;
        run(&mut s, &w, first as usize - 20);
        s.hit(id, 1, ORIGIN);
        let e = s.get(id).unwrap();
        assert!(e.flee_ticks_left >= 60, "restarted: {}", e.flee_ticks_left);
    }

    #[test]
    fn an_ignored_hit_does_not_start_a_flee() {
        let mut s = store(1);
        let id = s.spawn("cow", [0.5, 1.0, 0.5], 0.0).unwrap();
        s.hit(id, 4, ORIGIN);
        let e = s.get(id).unwrap().clone();
        s.hit(id, 4, ORIGIN);
        assert_eq!(s.get(id).unwrap(), &e);
    }

    #[test]
    fn the_same_seed_and_hits_give_the_same_flee_path() {
        assert_eq!(flee_run(8, 150).0, flee_run(8, 150).0);
        assert_ne!(flee_run(8, 150).0, flee_run(9, 150).0);
    }
}
