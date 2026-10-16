//! Loose items in the world: the item entity type and the store that holds
//! them. See docs/design/basic-gameplay.md section 6.
//!
//! [`ItemStore::tick`] moves them (gravity, drag, landing, the deck push).
//! Pickup and throwing are separate tickets.

use crate::collision::{Aabb, CollisionWorld};
use crate::items::ItemInstance;
use crate::movement::move_box;
use crate::raft::{Raft, SwellPush};
use crate::targeting::{EntityId, TargetableEntity};
use std::fmt;

const DEFAULT_ITEMS: &str = include_str!("../../../data/items.toml");

/// The `kind` item entities report to rule resolution, so a rule target of
/// `entity:item` matches them and `entity:animal` does not.
pub const ITEM_KIND: &str = "item";

/// Ticks per second; item velocities are blocks per second.
pub const TICKS_PER_SECOND: f64 = 20.0;

/// Item entity numbers from the `[item_entity]` table of `data/items.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemEntityConfig {
    /// Edge length of the item's cubic box, in blocks.
    pub box_size: f64,
    /// Downward acceleration, blocks per second squared.
    pub gravity: f64,
    /// Fraction of velocity kept each tick, on every axis.
    pub drag: f64,
    /// Extra fraction of horizontal velocity kept each tick while resting.
    pub ground_friction: f64,
    /// Speed a dropped item leaves the eye at, blocks per second.
    pub throw_speed: f64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ItemEntityError {
    Parse(String),
    Missing(String),
    Invalid(String),
}

impl fmt::Display for ItemEntityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemEntityError::Parse(e) => write!(f, "items.toml: {e}"),
            ItemEntityError::Missing(k) => write!(f, "items.toml: missing `{k}`"),
            ItemEntityError::Invalid(e) => write!(f, "items.toml: {e}"),
        }
    }
}

impl std::error::Error for ItemEntityError {}

impl ItemEntityConfig {
    /// The shipped defaults from `data/items.toml`.
    pub fn default_from_data() -> ItemEntityConfig {
        ItemEntityConfig::parse(DEFAULT_ITEMS).expect("data/items.toml is valid")
    }

    /// Parse items text. `box_size`, `gravity` and `throw_speed` must be finite numbers
    /// above zero; `drag` and `ground_friction` must lie in `[0, 1]`.
    pub fn parse(text: &str) -> Result<ItemEntityConfig, ItemEntityError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| ItemEntityError::Parse(e.to_string()))?;
        let get = |key: &str| -> Result<f64, ItemEntityError> {
            let value = root
                .get("item_entity")
                .and_then(|t| t.get(key))
                .ok_or_else(|| ItemEntityError::Missing(format!("item_entity.{key}")))?;
            match value {
                toml::Value::Float(f) => Ok(*f),
                toml::Value::Integer(i) => Ok(*i as f64),
                _ => Err(ItemEntityError::Invalid(format!(
                    "item_entity.{key} must be a number"
                ))),
            }
        };
        let positive = |key: &str| -> Result<f64, ItemEntityError> {
            let n = get(key)?;
            if !n.is_finite() || n <= 0.0 {
                return Err(ItemEntityError::Invalid(format!(
                    "item_entity.{key} must be above zero"
                )));
            }
            Ok(n)
        };
        let fraction = |key: &str| -> Result<f64, ItemEntityError> {
            let n = get(key)?;
            if !(0.0..=1.0).contains(&n) {
                return Err(ItemEntityError::Invalid(format!(
                    "item_entity.{key} must be between 0 and 1"
                )));
            }
            Ok(n)
        };
        Ok(ItemEntityConfig {
            box_size: positive("box_size")?,
            gravity: positive("gravity")?,
            drag: fraction("drag")?,
            ground_friction: fraction("ground_friction")?,
            throw_speed: positive("throw_speed")?,
        })
    }
}

/// Horizontal speed below which a resting item stops, blocks per second.
const REST_SPEED: f64 = 1e-3;

/// Step used to lift an item out of a block, blocks.
const LIFT_STEP: f64 = 1.0 / 16.0;

/// The raft deck and the swell acting on items that rest on it.
#[derive(Clone, Copy)]
pub struct Deck<'a> {
    pub raft: &'a Raft,
    /// Deck tilt this tick, degrees about the x and z axes.
    pub tilt: [f64; 2],
    pub push: &'a SwellPush,
}

/// Raise an item whose box overlaps a block until it is clear, so it never
/// rests inside a full block. Gives up after 64 blocks of lift.
/// `boxes` is scratch space, reused across calls.
fn lift_out_of_blocks(
    world: &impl CollisionWorld,
    bb: &mut Aabb,
    pos: &mut [f64; 3],
    boxes: &mut Vec<Aabb>,
) {
    for _ in 0..(64.0 / LIFT_STEP) as usize {
        boxes.clear();
        world.boxes_in(bb, boxes);
        if boxes.is_empty() {
            return;
        }
        *bb = bb.offset([0.0, LIFT_STEP, 0.0]);
        pos[1] += LIFT_STEP;
    }
}

/// A loose item lying in or flying through the world.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemEntity {
    pub id: EntityId,
    pub item: ItemInstance,
    /// Centre of the box, in world coordinates.
    pub pos: [f64; 3],
    /// Blocks per second.
    pub velocity: [f64; 3],
}

impl ItemEntity {
    /// The item's box at its current position.
    pub fn aabb(&self, cfg: &ItemEntityConfig) -> Aabb {
        let h = cfg.box_size / 2.0;
        Aabb::new(
            [0, 1, 2].map(|i| self.pos[i] - h),
            [0, 1, 2].map(|i| self.pos[i] + h),
        )
    }
}

/// All loose items. Ids are never reused within a store, so a stale
/// `Target::Entity { id }` cannot end up pointing at a newer item.
///
/// Ids share one space with every other entity the eye ray can target;
/// `first_id` lets the caller start this store above the others.
#[derive(Clone, Debug)]
pub struct ItemStore {
    cfg: ItemEntityConfig,
    next_id: EntityId,
    // Kept sorted by id (ids only grow), so lookups can bisect.
    items: Vec<ItemEntity>,
}

impl ItemStore {
    pub fn new(cfg: ItemEntityConfig, first_id: EntityId) -> ItemStore {
        ItemStore {
            cfg,
            next_id: first_id,
            items: Vec::new(),
        }
    }

    pub fn config(&self) -> &ItemEntityConfig {
        &self.cfg
    }

    /// Add a loose item and return its id.
    pub fn spawn(&mut self, item: ItemInstance, pos: [f64; 3], velocity: [f64; 3]) -> EntityId {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("item ids exhausted");
        self.items.push(ItemEntity {
            id,
            item,
            pos,
            velocity,
        });
        id
    }

    /// Remove an item, returning it, or `None` if the id is not in the store.
    pub fn remove(&mut self, id: EntityId) -> Option<ItemEntity> {
        let i = self.items.binary_search_by_key(&id, |e| e.id).ok()?;
        Some(self.items.remove(i))
    }

    pub fn get(&self, id: EntityId) -> Option<&ItemEntity> {
        let i = self.items.binary_search_by_key(&id, |e| e.id).ok()?;
        Some(&self.items[i])
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut ItemEntity> {
        let i = self.items.binary_search_by_key(&id, |e| e.id).ok()?;
        Some(&mut self.items[i])
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Items in id order.
    pub fn iter(&self) -> impl Iterator<Item = &ItemEntity> {
        self.items.iter()
    }

    /// Mutable access for the motion step.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ItemEntity> {
        self.items.iter_mut()
    }

    /// Advance every item one tick, in id order, against `world` (which
    /// should include the raft's boxes where there is one).
    ///
    /// Per item: the swell push if `deck` holds it up, gravity, a swept move
    /// that stops flush against blocks, then drag, and ground friction while
    /// resting. An item that starts inside a block is lifted out first.
    pub fn tick(&mut self, world: &impl CollisionWorld, deck: Option<Deck<'_>>) {
        let cfg = self.cfg;
        let dt = 1.0 / TICKS_PER_SECOND;
        let mut scratch = Vec::new();
        for e in &mut self.items {
            let mut bb = e.aabb(&cfg);
            lift_out_of_blocks(world, &mut bb, &mut e.pos, &mut scratch);
            if let Some(d) = &deck {
                if d.raft.supports(&bb) {
                    // The push is blocks per tick; velocities are per second.
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

    /// The items as targets, of kind `item`, for `cast_target`.
    pub fn targetables(&self) -> Vec<TargetableEntity> {
        self.items
            .iter()
            .map(|e| TargetableEntity {
                id: e.id,
                kind: ITEM_KIND.to_string(),
                aabb: e.aabb(&self.cfg),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::interactions::{Button, HeldThing, Interactions, ResolveCtx, TargetFacts};
    use crate::raft::RaftWorld;
    use crate::targeting::{cast_target, Target, TargetingConfig};
    use terrain::Block;

    fn seeds() -> ItemInstance {
        ItemInstance {
            item: "seeds".into(),
        }
    }

    fn store() -> ItemStore {
        ItemStore::new(ItemEntityConfig::default_from_data(), 1)
    }

    fn cast(w: &MemoryWorld, es: &[TargetableEntity]) -> Target {
        let cfg = TargetingConfig::default_from_data().unwrap();
        cast_target(w, es, &cfg, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0])
    }

    #[test]
    fn shipped_config_loads() {
        let c = ItemEntityConfig::default_from_data();
        assert!(c.box_size > 0.0 && c.box_size < 1.0);
        assert!(c.gravity > 0.0);
        assert!((0.0..=1.0).contains(&c.drag) && (0.0..=1.0).contains(&c.ground_friction));
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

    fn run(s: &mut ItemStore, w: &impl CollisionWorld, n: usize) {
        for _ in 0..n {
            s.tick(w, None);
        }
    }

    #[test]
    fn dropped_item_falls_and_rests_on_the_ground() {
        let w = floor();
        let mut s = store();
        let id = s.spawn(seeds(), [0.5, 4.0, 0.5], [0.0; 3]);
        s.tick(&w, None);
        assert!(s.get(id).unwrap().pos[1] < 4.0, "falls");
        run(&mut s, &w, 100);
        let e = s.get(id).unwrap();
        let half = s.config().box_size / 2.0;
        assert!(
            (e.pos[1] - (1.0 + half)).abs() < 1e-6,
            "rests on top: {}",
            e.pos[1]
        );
        assert_eq!(e.velocity, [0.0; 3]);
        let before = e.pos;
        run(&mut s, &w, 20);
        assert_eq!(s.get(id).unwrap().pos, before, "stays put");
    }

    #[test]
    fn thrown_item_lands_and_slides_to_a_stop() {
        let w = floor();
        let mut s = store();
        let id = s.spawn(seeds(), [0.5, 2.0, 0.5], [4.0, 3.0, 0.0]);
        run(&mut s, &w, 200);
        let e = s.get(id).unwrap();
        assert!(e.pos[0] > 1.0 && e.pos[0] < 7.0, "x = {}", e.pos[0]);
        assert_eq!(e.velocity, [0.0; 3]);
    }

    #[test]
    fn items_stop_at_walls() {
        let mut w = floor();
        for y in 1..3 {
            w.set(3, y, 0, Block::Stone);
        }
        let mut s = store();
        let id = s.spawn(seeds(), [0.5, 1.2, 0.5], [20.0, 0.0, 0.0]);
        run(&mut s, &w, 60);
        let e = s.get(id).unwrap();
        let half = s.config().box_size / 2.0;
        assert!(e.pos[0] <= 3.0 - half + 1e-9, "x = {}", e.pos[0]);
    }

    #[test]
    fn an_item_spawned_in_a_block_is_lifted_out() {
        let mut w = floor();
        for y in 1..3 {
            w.set(0, y, 0, Block::Stone);
        }
        let mut s = store();
        let id = s.spawn(seeds(), [0.5, 1.5, 0.5], [0.0; 3]);
        run(&mut s, &w, 100);
        let e = s.get(id).unwrap();
        let half = s.config().box_size / 2.0;
        assert!((e.pos[1] - (3.0 + half)).abs() < 1e-6, "y = {}", e.pos[1]);
        let mut boxes = Vec::new();
        w.boxes_in(&e.aabb(s.config()), &mut boxes);
        assert!(boxes.is_empty());
    }

    #[test]
    fn the_same_ticks_give_the_same_result() {
        let w = floor();
        let go = || {
            let mut s = store();
            s.spawn(seeds(), [0.5, 3.0, 0.5], [2.0, 5.0, -1.0]);
            s.spawn(seeds(), [2.5, 1.0, 1.5], [-3.0, 0.0, 2.0]);
            run(&mut s, &w, 80);
            s.iter().map(|e| (e.pos, e.velocity)).collect::<Vec<_>>()
        };
        assert_eq!(go(), go());
    }

    fn deck() -> (MemoryWorld, Raft) {
        let mut w = MemoryWorld::new();
        let mut cells = Vec::new();
        for x in 0..4 {
            for z in 0..4 {
                w.set(x, 64, z, Block::Stone);
                cells.push([x, 64, z]);
            }
        }
        (w, Raft::new(&cells))
    }

    #[test]
    fn deck_push_is_scaled_by_item_multiplier() {
        let (base, raft) = deck();
        let world = RaftWorld::new(&base, &raft);
        let push = SwellPush::default_from_data().unwrap();
        let mut s = store();
        let id = s.spawn(seeds(), [2.0, 65.0 + 0.125, 2.0], [0.0; 3]);
        run(&mut s, &world, 1);
        // Settled on the deck, no push yet.
        let tilt = [0.0, 3.0];
        s.tick(
            &world,
            Some(Deck {
                raft: &raft,
                tilt,
                push: &push,
            }),
        );
        let v = s.get(id).unwrap().velocity;
        let per_tick = push.per_degree * 3.0 * push.item_multiplier;
        let want = per_tick * TICKS_PER_SECOND * s.config().drag * s.config().ground_friction;
        assert!((v[0] - want).abs() < 1e-9, "{} vs {want}", v[0]);
        assert_eq!(v[2], 0.0);
        // A player-strength push would be a smaller, different number.
        assert!(push.item_multiplier > 1.0 && (v[0] - want / push.item_multiplier).abs() > 1e-6);
        // The item slides along the deck.
        for _ in 0..10 {
            s.tick(
                &world,
                Some(Deck {
                    raft: &raft,
                    tilt,
                    push: &push,
                }),
            );
        }
        assert!(s.get(id).unwrap().pos[0] > 2.0);
        assert_eq!(s.get(id).unwrap().pos[1], 65.125);
    }

    #[test]
    fn items_off_the_deck_get_no_push() {
        let (mut base, raft) = deck();
        for x in 10..14 {
            for z in 0..4 {
                base.set(x, 64, z, Block::Stone);
            }
        }
        let world = RaftWorld::new(&base, &raft);
        let push = SwellPush::default_from_data().unwrap();
        let mut s = store();
        let id = s.spawn(seeds(), [11.0, 65.125, 2.0], [0.0; 3]);
        for _ in 0..10 {
            s.tick(
                &world,
                Some(Deck {
                    raft: &raft,
                    tilt: [0.0, 3.0],
                    push: &push,
                }),
            );
        }
        assert_eq!(s.get(id).unwrap().velocity, [0.0; 3]);
        assert_eq!(s.get(id).unwrap().pos[0], 11.0);
    }

    #[test]
    fn config_rejects_bad_data() {
        assert!(matches!(
            ItemEntityConfig::parse("[[tool]]\nname='a'\nlabel='A'\n"),
            Err(ItemEntityError::Missing(_))
        ));
        for bad in ["box_size = 0", "box_size = -1", "box_size = 'x'"] {
            let t = format!("[item_entity]\n{bad}\n");
            assert!(
                matches!(
                    ItemEntityConfig::parse(&t),
                    Err(ItemEntityError::Invalid(_))
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            ItemEntityConfig::parse("not toml ["),
            Err(ItemEntityError::Parse(_))
        ));
    }

    #[test]
    fn spawn_gives_stable_ids_and_remove_frees() {
        let mut s = store();
        let a = s.spawn(seeds(), [1.0, 0.0, 0.0], [0.0; 3]);
        let b = s.spawn(seeds(), [2.0, 0.0, 0.0], [0.0; 3]);
        assert_ne!(a, b);
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(a).unwrap().pos, [1.0, 0.0, 0.0]);
        // Removing one leaves the other's id and data alone.
        assert_eq!(s.remove(a).unwrap().id, a);
        assert!(s.get(a).is_none());
        assert_eq!(s.get(b).unwrap().pos, [2.0, 0.0, 0.0]);
        assert!(s.remove(a).is_none());
        // A freed id is not handed out again.
        let c = s.spawn(seeds(), [3.0, 0.0, 0.0], [0.0; 3]);
        assert!(c != a && c != b);
        assert_eq!(s.iter().map(|e| e.id).collect::<Vec<_>>(), vec![b, c]);
        s.remove(b);
        s.remove(c);
        assert!(s.is_empty());
    }

    #[test]
    fn first_id_offsets_the_store() {
        let mut s = ItemStore::new(ItemEntityConfig::default_from_data(), 1000);
        assert_eq!(s.spawn(seeds(), [0.0; 3], [0.0; 3]), 1000);
    }

    #[test]
    fn box_is_centred_on_position() {
        let mut s = store();
        let id = s.spawn(seeds(), [1.0, 2.0, 3.0], [0.0; 3]);
        let b = s.get(id).unwrap().aabb(s.config());
        let h = s.config().box_size / 2.0;
        assert_eq!(b.min, [1.0 - h, 2.0 - h, 3.0 - h]);
        assert_eq!(b.max, [1.0 + h, 2.0 + h, 3.0 + h]);
    }

    #[test]
    fn eye_ray_targets_an_item_in_reach() {
        let mut s = store();
        let id = s.spawn(seeds(), [2.5, 0.5, 0.5], [0.0; 3]);
        let w = MemoryWorld::new();
        assert_eq!(cast(&w, &s.targetables()), Target::Entity { id });
        // Past entity reach (3.0 plus box and expansion) it is ignored.
        let mut far = store();
        far.spawn(seeds(), [5.5, 0.5, 0.5], [0.0; 3]);
        assert_eq!(cast(&w, &far.targetables()), Target::None);
        // Removed items are no longer targeted.
        s.remove(id);
        assert_eq!(cast(&w, &s.targetables()), Target::None);
    }

    #[test]
    fn nearest_hit_wins_against_a_block() {
        let mut w = MemoryWorld::new();
        w.set(3, 0, 0, Block::Stone);
        // Item in front of the block is targeted.
        let mut s = store();
        let id = s.spawn(seeds(), [2.0, 0.5, 0.5], [0.0; 3]);
        assert_eq!(cast(&w, &s.targetables()), Target::Entity { id });
        // Item behind the block is hidden by it.
        let mut s = store();
        s.spawn(seeds(), [3.5, 0.5, 0.5], [0.0; 3]);
        assert!(matches!(
            cast(&w, &s.targetables()),
            Target::Block { pos: [3, 0, 0], .. }
        ));
    }

    #[test]
    fn rules_tell_items_from_animals() {
        let mut s = store();
        let id = s.spawn(seeds(), [2.5, 0.5, 0.5], [0.0; 3]);
        let kind = s.targetables().remove(0).kind;
        assert_eq!(kind, ITEM_KIND);
        let rules = Interactions::from_toml(
            "[[rule]]\nheld='tool:sword'\nbutton='primary'\ntarget='entity:animal'\naction='attack'\n",
        )
        .unwrap();
        let ctx = ResolveCtx {
            held: HeldThing::Tool("sword".into()),
            button: Button::Primary,
            target: Target::Entity { id },
            facts: TargetFacts {
                entity: Some(kind),
                ..TargetFacts::default()
            },
        };
        assert_eq!(crate::interactions::resolve(&rules.rules, &ctx), None);
        let ctx = ResolveCtx {
            facts: TargetFacts {
                entity: Some("animal".into()),
                ..TargetFacts::default()
            },
            ..ctx
        };
        assert!(crate::interactions::resolve(&rules.rules, &ctx).is_some());
    }
}
