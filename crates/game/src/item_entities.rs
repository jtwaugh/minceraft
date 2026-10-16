//! Loose items in the world: the item entity type and the store that holds
//! them. See docs/design/basic-gameplay.md section 6.
//!
//! Motion and collision (gravity, drag, landing) and pickup are separate
//! tickets; this module only owns the records and exposes them to targeting.

use crate::collision::Aabb;
use crate::items::ItemInstance;
use crate::targeting::{EntityId, TargetableEntity};
use std::fmt;

const DEFAULT_ITEMS: &str = include_str!("../../../data/items.toml");

/// The `kind` item entities report to rule resolution, so a rule target of
/// `entity:item` matches them and `entity:animal` does not.
pub const ITEM_KIND: &str = "item";

/// Item entity numbers from the `[item_entity]` table of `data/items.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemEntityConfig {
    /// Edge length of the item's cubic box, in blocks.
    pub box_size: f64,
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

    /// Parse items text; `item_entity.box_size` must be a finite number above zero.
    pub fn parse(text: &str) -> Result<ItemEntityConfig, ItemEntityError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| ItemEntityError::Parse(e.to_string()))?;
        let value = root
            .get("item_entity")
            .and_then(|t| t.get("box_size"))
            .ok_or_else(|| ItemEntityError::Missing("item_entity.box_size".into()))?;
        let n = match value {
            toml::Value::Float(f) => *f,
            toml::Value::Integer(i) => *i as f64,
            _ => {
                return Err(ItemEntityError::Invalid(
                    "item_entity.box_size must be a number".into(),
                ))
            }
        };
        if !n.is_finite() || n <= 0.0 {
            return Err(ItemEntityError::Invalid(
                "item_entity.box_size must be above zero".into(),
            ));
        }
        Ok(ItemEntityConfig { box_size: n })
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
