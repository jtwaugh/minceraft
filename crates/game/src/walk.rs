//! The walking player as the client drives it: a `Player` stepped at 20 Hz
//! over loaded terrain regions, plus the eye pose between ticks.
//! See docs/design/basic-gameplay.md sections 1 (rule 1) and 3.

use crate::collision::{block_selection_boxes_in, Aabb, CollisionWorld, RegionWorld};
use crate::input::TickInput;
use crate::interactions::{
    resolve, ActionKind, Button, Dispatcher, HeldThing, Interactions, ResolveCtx, Resolved,
    TargetFacts,
};
use crate::item_entities::{ItemEntityConfig, ItemStore};
use crate::items::{ItemInstance, Items, PlayerState};
use crate::physics::{PhysicsError, PlayerPhysics};
use crate::player::Player;
use crate::raft::{Raft, RaftWorld, Swell, SwellPush};
use crate::scene::RaftScene;
use crate::stream::{StreamConfig, Streamer};
use crate::targeting::{cast_target, ray_vs_box, Target, TargetableEntity, TargetingConfig};
use layout::Layout;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use terrain::{Block, Region, HEIGHT};

/// Side of the square column cells the lookup index is split into.
const CELL: i64 = 16;

fn cell_of(v: i64) -> i64 {
    v.div_euclid(CELL)
}

/// Collision over several generated regions, keyed by region origin. A column
/// is found through a grid of cells, each listing the regions that touch it,
/// so a lookup does not scan all regions. Where regions overlap, the one
/// inserted first answers (regions generated from one seed agree anyway), and
/// air is returned outside all of them. Inserting at an origin already loaded
/// replaces that region.
#[derive(Default)]
pub struct RegionSet {
    regions: HashMap<(i32, i32), (u64, Region)>,
    cells: HashMap<(i64, i64), Vec<(i32, i32)>>,
    next_seq: u64,
}

impl RegionSet {
    fn cell_range(r: &Region) -> (RangeInclusive<i64>, RangeInclusive<i64>) {
        let lo = |o: i32| cell_of(o as i64);
        let hi = |o: i32, n: u32| cell_of(o as i64 + (n as i64).max(1) - 1);
        (lo(r.x0)..=hi(r.x0, r.width), lo(r.z0)..=hi(r.z0, r.depth))
    }

    /// Load `region`, replacing any region with the same origin.
    pub fn insert(&mut self, region: Region) {
        self.remove(region.x0, region.z0);
        let key = (region.x0, region.z0);
        let (xs, zs) = Self::cell_range(&region);
        for cx in xs {
            for cz in zs.clone() {
                self.cells.entry((cx, cz)).or_default().push(key);
            }
        }
        self.regions.insert(key, (self.next_seq, region));
        self.next_seq += 1;
    }

    /// Same as `insert`.
    pub fn push(&mut self, region: Region) {
        self.insert(region);
    }

    /// Unload the region whose origin is `(x0, z0)`; its columns read as air.
    pub fn remove(&mut self, x0: i32, z0: i32) -> Option<Region> {
        let (_, region) = self.regions.remove(&(x0, z0))?;
        let (xs, zs) = Self::cell_range(&region);
        for cx in xs {
            for cz in zs.clone() {
                if let Some(list) = self.cells.get_mut(&(cx, cz)) {
                    list.retain(|k| *k != (x0, z0));
                    if list.is_empty() {
                        self.cells.remove(&(cx, cz));
                    }
                }
            }
        }
        Some(region)
    }

    /// Origins `(x0, z0)` of the loaded regions, in no particular order.
    pub fn loaded(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.regions.keys().copied()
    }

    /// The loaded region that holds world column `(x, z)`.
    pub fn region_at(&self, x: i32, z: i32) -> Option<&Region> {
        let (x, z) = (x as i64, z as i64);
        let list = self.cells.get(&(cell_of(x), cell_of(z)))?;
        list.iter()
            .filter_map(|k| self.regions.get(k))
            .filter(|(_, r)| {
                let (lx, lz) = (x - r.x0 as i64, z - r.z0 as i64);
                lx >= 0 && lz >= 0 && lx < r.width as i64 && lz < r.depth as i64
            })
            .min_by_key(|(seq, _)| *seq)
            .map(|(_, r)| r)
    }

    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

impl CollisionWorld for RegionSet {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        if y < 0 || y >= HEIGHT as i32 {
            return Block::Air;
        }
        match self.region_at(x, z) {
            Some(r) => RegionWorld::new(r).block_at(x, y, z),
            None => Block::Air,
        }
    }
}

/// Item entity ids start here, above the ids of the stub entities.
const FIRST_ITEM_ID: crate::targeting::EntityId = 1 << 20;

/// A raft under the scripted swell, and the swell's clock.
struct Floating {
    raft: Raft,
    swell: Swell,
    push: SwellPush,
    tick: u64,
    prev_offset: [f64; 3],
}

/// The player plus the eye positions of the last two ticks.
pub struct Walker {
    player: Player,
    physics: PlayerPhysics,
    world: RegionSet,
    floating: Option<Floating>,
    streamer: Option<Streamer>,
    prev_eye: [f64; 3],
    cur_eye: [f64; 3],
    targeting: TargetingConfig,
    target: Target,
    entities: Vec<TargetableEntity>,
    rules: Interactions,
    dispatcher: Dispatcher,
    items: ItemStore,
    state: PlayerState,
    /// Test override of the derived held thing.
    held_override: Option<HeldThing>,
    facts: TargetFacts,
    target_box: Option<(Aabb, bool)>,
}

impl Walker {
    /// A player with its feet at `feet`, standing on nothing until regions
    /// are added.
    pub fn new(feet: [f64; 3]) -> Result<Walker, PhysicsError> {
        let physics = PlayerPhysics::default_from_data()?;
        let player = Player::new(feet, &physics);
        let eye = eye_above(feet, &physics, false);
        let targeting = TargetingConfig::default_from_data()
            .map_err(|e| PhysicsError::Invalid(e.to_string()))?;
        Ok(Walker {
            player,
            physics,
            world: RegionSet::default(),
            floating: None,
            streamer: None,
            prev_eye: eye,
            cur_eye: eye,
            targeting,
            target: Target::None,
            entities: Vec::new(),
            rules: Interactions::default_rules(),
            dispatcher: Dispatcher::default(),
            items: ItemStore::new(ItemEntityConfig::default_from_data(), FIRST_ITEM_ID),
            state: PlayerState::new(&Items::default_items())
                .map_err(|e| PhysicsError::Invalid(e.to_string()))?,
            held_override: None,
            facts: TargetFacts::default(),
            target_box: None,
        })
    }

    pub fn add_region(&mut self, region: Region) {
        self.world.insert(region);
    }

    /// Unload the region whose origin is `(x0, z0)`.
    pub fn remove_region(&mut self, x0: i32, z0: i32) -> Option<Region> {
        self.world.remove(x0, z0)
    }

    /// Origins `(x0, z0)` of the loaded regions, in no particular order.
    pub fn loaded_regions(&self) -> Vec<(i32, i32)> {
        self.world.loaded().collect()
    }

    /// A walker with its feet at `feet` over a streamed world: the player's
    /// region and its neighbours are loaded now, and regions load and unload
    /// as the player crosses borders (radii from `data/streaming.toml`).
    pub fn with_layout(layout: Layout, feet: [f64; 3]) -> Result<Walker, String> {
        let mut w = Walker::new(feet).map_err(|e| e.to_string())?;
        let mut streamer = Streamer::new(layout, StreamConfig::default_from_data()?);
        streamer.update(feet, &mut w.world);
        w.streamer = Some(streamer);
        Ok(w)
    }

    /// Number of regions currently loaded for collision.
    pub fn loaded_region_count(&self) -> usize {
        self.world.len()
    }

    /// The region the stream is centred on, if this walker streams.
    pub fn streamed_centre(&self) -> Option<layout::RegionCoord> {
        self.streamer.as_ref().and_then(|s| s.centre())
    }

    /// The region cache behind the stream, if this walker streams.
    pub fn streamer_cache(&self) -> Option<&layout::RegionCache> {
        self.streamer.as_ref().map(|s| s.cache())
    }

    /// A walker in the raft test scene: the sea, the raft moving under the
    /// scripted swell, and the player in the water in front of it.
    pub fn in_raft_scene(scene: RaftScene) -> Result<Walker, String> {
        let mut w = Walker::new(scene.spawn).map_err(|e| e.to_string())?;
        w.floating = Some(Floating {
            raft: Raft::new(&scene.cells),
            swell: Swell::default_from_data()?,
            push: SwellPush::default_from_data()?,
            tick: 0,
            prev_offset: [0.0; 3],
        });
        w.add_region(scene.sea);
        w.entities = scene.entities;
        Ok(w)
    }

    /// How far the raft is from its rest pose, `alpha` of the way between
    /// the last two ticks. Zero when there is no raft.
    pub fn raft_offset(&self, alpha: f64) -> [f64; 3] {
        let Some(f) = &self.floating else {
            return [0.0; 3];
        };
        between_ticks(f.prev_offset, f.raft.offset(), alpha)
    }

    pub fn has_world(&self) -> bool {
        !self.world.is_empty()
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    /// One 20 Hz step.
    pub fn tick(&mut self, input: &TickInput) {
        let entities = self.all_entities();
        if let Some(slot) = input.tool_slot {
            self.state.select(slot);
        }
        match &mut self.floating {
            Some(f) => {
                // Swell push, then the raft moves (carrying the player), then
                // the player's own movement against the moved raft.
                let frame = f.swell.frame(f.tick);
                f.tick += 1;
                f.prev_offset = f.raft.offset();
                f.raft.push_rider(
                    &mut self.player,
                    frame.tilt,
                    input.sneak,
                    &f.push,
                    &self.physics,
                );
                f.raft
                    .advance(&frame, &mut [&mut self.player], &self.physics);
                let world = RaftWorld::new(&self.world, &f.raft);
                self.player.tick(input, &self.physics, &world);
                self.prev_eye = self.cur_eye;
                self.cur_eye = eye_above(self.player.pos, &self.physics, input.sneak);
                let aim = aim(&world, &entities, &self.targeting, self.cur_eye, input);
                self.take_aim(aim, input);
            }
            None => {
                self.player.tick(input, &self.physics, &self.world);
                self.prev_eye = self.cur_eye;
                self.cur_eye = eye_above(self.player.pos, &self.physics, input.sneak);
                let aim = aim(&self.world, &entities, &self.targeting, self.cur_eye, input);
                self.take_aim(aim, input);
            }
        }
        if let Some(s) = &mut self.streamer {
            s.update(self.player.pos, &mut self.world);
        }
    }

    /// Stub targets for the test scene; `Target::Entity { id }` refers to these.
    pub fn set_entities(&mut self, entities: Vec<TargetableEntity>) {
        self.entities = entities;
    }

    /// Force the held thing, whatever the belt and hands say (tests and the
    /// debug overlay). `clear_held_override` goes back to the derived one.
    pub fn set_held(&mut self, held: HeldThing) {
        self.held_override = Some(held);
    }

    pub fn clear_held_override(&mut self) {
        self.held_override = None;
    }

    /// The belt, selection and hands.
    pub fn player_state(&self) -> &PlayerState {
        &self.state
    }

    pub fn player_state_mut(&mut self) -> &mut PlayerState {
        &mut self.state
    }

    /// What interaction rules see: the override if set, else derived from
    /// the player state (design section 6).
    pub fn held(&self) -> HeldThing {
        self.held_override
            .clone()
            .unwrap_or_else(|| self.state.held_thing())
    }

    /// Put a loose item in the world, returning its entity id.
    pub fn spawn_item(&mut self, item: ItemInstance, pos: [f64; 3]) -> crate::targeting::EntityId {
        self.items.spawn(item, pos, [0.0; 3])
    }

    pub fn items(&self) -> &ItemStore {
        &self.items
    }

    /// The carried item, if any.
    pub fn hands(&self) -> Option<&ItemInstance> {
        self.state.hands.as_ref()
    }

    /// Stub entities plus the loose items, as the eye ray sees them.
    fn all_entities(&self) -> Vec<TargetableEntity> {
        let mut all = self.entities.clone();
        all.extend(self.items.targetables());
        all
    }

    /// Record this tick's aim and run the presses in `input` against it.
    fn take_aim(&mut self, aim: Aim, input: &TickInput) {
        self.target = aim.target;
        self.target_box = aim.boxed;
        let ctx = ResolveCtx {
            held: self.held(),
            button: Button::Primary,
            target: aim.target,
            facts: aim.facts,
        };
        // Tools are stowed while carrying: primary does nothing.
        let mut pressed = *input;
        pressed.primary &= self.state.hands.is_none();
        self.dispatcher.dispatch(&self.rules, &pressed, &ctx);
        self.facts = ctx.facts;
        if input.interact {
            self.interact();
        }
    }

    /// Interact picks up the targeted loose item if the hands are empty.
    /// Stations and counters arrive later.
    fn interact(&mut self) {
        if self.state.hands.is_some() {
            return;
        }
        let Target::Entity { id } = self.target else {
            return;
        };
        if let Some(e) = self.items.remove(id) {
            self.state.hands = Some(e.item);
            self.target = Target::None;
            self.target_box = None;
            self.facts = TargetFacts::default();
        }
    }

    /// The box to outline for the current target (the block's selection box
    /// or the entity's box) in world coordinates as of the latest tick, and
    /// whether it belongs to the raft, so the caller can move it with the
    /// raft's interpolated offset instead of the latest tick's.
    pub fn target_box(&self) -> Option<(Aabb, bool)> {
        self.target_box
    }

    /// The rule a press of `button` would match against the current target
    /// and held thing, for the debug overlay.
    pub fn preview(&self, button: Button) -> Option<Resolved> {
        if button == Button::Primary && self.state.hands.is_some() {
            return None;
        }
        let ctx = ResolveCtx {
            held: self.held(),
            button,
            target: self.target,
            facts: self.facts.clone(),
        };
        resolve(&self.rules.rules, &ctx)
    }

    /// The `action` of rule `index`, as written in `data/interactions.toml`.
    pub fn rule_action(&self, index: usize) -> Option<ActionKind> {
        self.rules.rules.get(index).map(|r| r.action)
    }

    /// Rule index of the most recent press that matched, if any.
    pub fn last_rule(&self) -> Option<usize> {
        self.dispatcher.last_rule
    }

    /// What the eye ray pointed at after the latest tick (basic-gameplay.md
    /// section 5). `Target::None` before the first tick.
    pub fn target(&self) -> Target {
        self.target
    }

    /// The eye between the last two ticks, `alpha` in 0..1.
    pub fn eye(&self, alpha: f64) -> [f64; 3] {
        between_ticks(self.prev_eye, self.cur_eye, alpha)
    }

    /// Whether the block cell containing `pos` is a liquid. The client asks
    /// this of the drawn eye to switch to the underwater view. Water blocks
    /// fill their whole cell, so the cell alone decides; the raft is never
    /// liquid, so only the loaded regions are consulted.
    pub fn liquid_at(&self, pos: [f64; 3]) -> bool {
        let c = |v: f64| v.floor() as i32;
        self.world
            .block_at(c(pos[0]), c(pos[1]), c(pos[2]))
            .info()
            .liquid
    }

    /// Put the player at `feet` with no velocity, e.g. when leaving fly mode.
    pub fn teleport(&mut self, feet: [f64; 3]) {
        self.player = Player::new(feet, &self.physics);
        if let Some(s) = &mut self.streamer {
            s.update(feet, &mut self.world);
        }
        let eye = eye_above(feet, &self.physics, false);
        self.prev_eye = eye;
        self.cur_eye = eye;
    }
}

/// One tick's result of aiming the eye ray.
struct Aim {
    target: Target,
    facts: TargetFacts,
    /// The box to outline and whether the raft owns it.
    boxed: Option<(Aabb, bool)>,
}

/// What the ray from `eye` along the look angles of `input` hits, blocks and
/// entities both, with the names rules match on and the box to outline.
fn aim(
    world: &impl CollisionWorld,
    entities: &[TargetableEntity],
    cfg: &TargetingConfig,
    eye: [f64; 3],
    input: &TickInput,
) -> Aim {
    // Same forward vector as `Pose::forward` in camera.rs, in f64.
    let (sy, cy) = (input.yaw as f64).sin_cos();
    let (sp, cp) = (input.pitch as f64).sin_cos();
    let dir = [sy * cp, sp, -cy * cp];
    let target = cast_target(world, entities, cfg, eye, dir);
    match target {
        Target::Block { pos, .. } => {
            let end = [0, 1, 2].map(|i| eye[i] + dir[i] * f64::from(cfg.block_reach));
            let query = Aabb::new(
                [0, 1, 2].map(|i| eye[i].min(end[i])),
                [0, 1, 2].map(|i| eye[i].max(end[i])),
            );
            let mut all = Vec::new();
            world.selection_boxes_in(&query, &mut all);
            let mut base = Vec::new();
            block_selection_boxes_in(world, &query, &mut base);
            let boxed = all
                .iter()
                .filter(|b| b.pos == pos)
                .filter_map(|b| ray_vs_box(eye, dir, &b.aabb).map(|h| (h.distance, b.aabb)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, aabb)| (aabb, !base.iter().any(|b| b.aabb == aabb)));
            Aim {
                target,
                facts: TargetFacts {
                    block: Some(
                        world
                            .block_at(pos[0], pos[1], pos[2])
                            .info()
                            .name
                            .to_string(),
                    ),
                    air_above: world.block_at(pos[0], pos[1] + 1, pos[2]) == Block::Air,
                    ..TargetFacts::default()
                },
                boxed,
            }
        }
        Target::Entity { id } => {
            let e = entities.iter().find(|e| e.id == id);
            Aim {
                target,
                facts: TargetFacts {
                    entity: e.map(|e| e.kind.clone()),
                    ..TargetFacts::default()
                },
                boxed: e.map(|e| (e.aabb, false)),
            }
        }
        Target::None => Aim {
            target,
            facts: TargetFacts::default(),
            boxed: None,
        },
    }
}

/// `alpha` (clamped to 0..1) of the way from the previous tick's `prev` to
/// the current tick's `cur`.
fn between_ticks(prev: [f64; 3], cur: [f64; 3], alpha: f64) -> [f64; 3] {
    let a = alpha.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| prev[i] + (cur[i] - prev[i]) * a)
}

/// The eye position for feet at `feet`.
fn eye_above(feet: [f64; 3], physics: &PlayerPhysics, sneak: bool) -> [f64; 3] {
    [feet[0], feet[1] + physics.eye_offset(sneak) as f64, feet[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(x0: i32, z0: i32, n: u32, top: u32) -> Region {
        let mut blocks = vec![0u8; n as usize * n as usize * HEIGHT];
        let r = Region {
            blocks: Vec::new(),
            width: n,
            depth: n,
            height: HEIGHT as u32,
            x0,
            z0,
        };
        for z in 0..n {
            for x in 0..n {
                for y in 0..=top {
                    blocks[r.index(x, y, z)] = Block::Stone as u8;
                }
            }
        }
        Region { blocks, ..r }
    }

    fn settled() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.add_region(flat(-8, -8, 16, 3));
        for _ in 0..40 {
            w.tick(&TickInput::default());
        }
        w
    }

    #[test]
    fn eye_is_above_the_feet_and_lower_when_sneaking() {
        let mut w = settled();
        assert!(w.player().on_ground);
        let feet = w.player().pos[1];
        assert_eq!(feet, 4.0);
        assert!((w.eye(1.0)[1] - (feet + 1.62)).abs() < 1e-6);
        w.tick(&TickInput {
            sneak: true,
            ..TickInput::default()
        });
        assert!((w.eye(1.0)[1] - (feet + 1.62 - 0.08)).abs() < 1e-6);
    }

    fn look(pitch: f32, sneak: bool) -> TickInput {
        TickInput {
            pitch,
            sneak,
            ..TickInput::default()
        }
    }

    #[test]
    fn looking_down_targets_the_block_under_the_feet() {
        let mut w = settled();
        assert_eq!(w.target(), Target::None);
        w.tick(&look(-std::f32::consts::FRAC_PI_2, false));
        assert_eq!(
            w.target(),
            Target::Block {
                pos: [0, 3, 0],
                face: crate::targeting::Face::Top
            }
        );
    }

    fn stub_animal() -> TargetableEntity {
        TargetableEntity {
            id: 1,
            kind: "animal".into(),
            aabb: Aabb::new([0.2, 5.2, -2.0], [0.8, 6.0, -1.4]),
        }
    }

    #[test]
    fn a_stub_entity_is_targeted_outlined_and_matched_by_the_rules() {
        let mut w = settled();
        w.set_entities(vec![stub_animal()]);
        w.set_held(HeldThing::Tool("sword".into()));
        w.tick(&look(0.0, false));
        assert_eq!(w.target(), Target::Entity { id: 1 });
        assert_eq!(w.target_box(), Some((stub_animal().aabb, false)));
        let hit = w.preview(Button::Primary).unwrap();
        assert_eq!((hit.rule_index, hit.action), (0, ActionKind::Attack));
        assert!(w.preview(Button::Secondary).is_none());
        w.set_held(HeldThing::Empty);
        assert_eq!(w.preview(Button::Primary).unwrap().rule_index, 8);
    }

    fn seeds() -> ItemInstance {
        ItemInstance {
            item: "seeds".into(),
        }
    }

    fn interact() -> TickInput {
        TickInput {
            interact: true,
            ..look(0.0, false)
        }
    }

    /// A walker looking at seeds 1.5 blocks ahead (north) at eye height.
    fn facing_seeds() -> (Walker, crate::targeting::EntityId) {
        let mut w = settled();
        let id = w.spawn_item(seeds(), [0.5, 5.62, -1.0]);
        w.tick(&look(0.0, false));
        assert_eq!(w.target(), Target::Entity { id });
        (w, id)
    }

    #[test]
    fn interact_picks_up_a_targeted_item_with_empty_hands() {
        let (mut w, id) = facing_seeds();
        w.tick(&interact());
        assert_eq!(w.hands(), Some(&seeds()));
        assert!(w.items().get(id).is_none());
        assert_eq!(w.target(), Target::None);
    }

    #[test]
    fn interact_with_full_hands_does_nothing() {
        let (mut w, id) = facing_seeds();
        let other = ItemInstance {
            item: "other".into(),
        };
        w.state.hands = Some(other.clone());
        w.tick(&interact());
        assert_eq!(w.hands(), Some(&other));
        assert!(w.items().get(id).is_some());
    }

    #[test]
    fn interact_out_of_reach_or_on_nothing_picks_nothing_up() {
        let mut w = settled();
        let far = w.spawn_item(seeds(), [0.5, 5.62, -6.0]);
        w.tick(&interact());
        assert_eq!(w.hands(), None);
        assert!(w.items().get(far).is_some());
        // Looking at the sky with an item behind: nothing targeted.
        let mut up = interact();
        up.pitch = 1.2;
        w.tick(&up);
        assert_eq!(w.hands(), None);
        assert_eq!(w.items().len(), 1);
        // Targeting a block (not an item) picks nothing up either.
        let mut down = interact();
        down.pitch = -1.5;
        w.tick(&down);
        assert_eq!(w.hands(), None);
        assert_eq!(w.items().len(), 1);
    }

    #[test]
    fn walking_into_an_item_does_not_pick_it_up() {
        let mut w = settled();
        let id = w.spawn_item(seeds(), [0.5, 4.2, -1.0]);
        let walk = TickInput {
            move_forward: 1.0,
            ..look(0.0, false)
        };
        for _ in 0..30 {
            w.tick(&walk);
        }
        assert!(w.player().pos[2] < -1.5, "walked past the item");
        assert_eq!(w.hands(), None);
        assert!(w.items().get(id).is_some());
    }

    #[test]
    fn carrying_stows_the_tool() {
        let mut w = settled();
        w.set_entities(vec![stub_animal()]);
        w.player_state_mut().select(1);
        w.tick(&look(0.0, false));
        assert_eq!(
            w.preview(Button::Primary).map(|r| r.action),
            Some(ActionKind::Attack)
        );
        w.state.hands = Some(seeds());
        // Even with a sword selected and an animal in front: no attack.
        w.tick(&TickInput {
            primary: true,
            ..look(0.0, false)
        });
        assert!(w.preview(Button::Primary).is_none());
        assert_eq!(w.last_rule(), None);
    }

    #[test]
    fn carrying_resolves_secondary_with_the_item() {
        let mut w = settled();
        w.player_state_mut().select(2);
        w.state.hands = Some(seeds());
        // Looking straight down at stone: seeds have no rule on stone, and
        // the hoe's till rule must not apply.
        let mut down = look(0.0, false);
        down.pitch = -1.5;
        w.tick(&down);
        assert!(w.preview(Button::Secondary).is_none());
        // Seeds on farmland resolve to plant.
        let ctx = ResolveCtx {
            held: w.held(),
            button: Button::Secondary,
            target: Target::Block {
                pos: [0, 3, 0],
                face: crate::targeting::Face::Top,
            },
            facts: TargetFacts {
                block: Some("farmland".into()),
                air_above: true,
                ..TargetFacts::default()
            },
        };
        let r = resolve(&w.rules.rules, &ctx).unwrap();
        assert_eq!(r.action, ActionKind::Plant);
        assert_eq!(w.held(), HeldThing::Item("seeds".into()));
    }

    fn slot(n: u8) -> TickInput {
        TickInput {
            tool_slot: Some(n),
            ..TickInput::default()
        }
    }

    #[test]
    fn slot_selection_derives_the_held_thing() {
        let mut w = settled();
        assert_eq!(w.held(), HeldThing::Empty);
        w.tick(&slot(1));
        assert_eq!(w.player_state().selected, 1);
        assert_eq!(w.held(), HeldThing::Tool("sword".into()));
        w.tick(&slot(2));
        assert_eq!(w.held(), HeldThing::Tool("hoe".into()));
        w.tick(&TickInput::default());
        assert_eq!(w.player_state().selected, 2);
        w.tick(&slot(9));
        assert_eq!(w.player_state().selected, 2);
        w.tick(&slot(0));
        assert_eq!(w.held(), HeldThing::Empty);
    }

    #[test]
    fn selection_applies_to_the_same_ticks_press() {
        let mut w = settled();
        w.set_entities(vec![stub_animal()]);
        w.tick(&TickInput {
            primary: true,
            tool_slot: Some(1),
            ..look(0.0, false)
        });
        assert_eq!(w.last_rule(), Some(0));
    }

    #[test]
    fn carried_item_wins_and_override_can_be_cleared() {
        let mut w = settled();
        w.tick(&slot(1));
        w.player_state_mut().hands = Some(crate::items::ItemInstance {
            item: "seeds".into(),
        });
        assert_eq!(w.held(), HeldThing::Item("seeds".into()));
        w.set_held(HeldThing::Empty);
        assert_eq!(w.held(), HeldThing::Empty);
        w.clear_held_override();
        assert_eq!(w.held(), HeldThing::Item("seeds".into()));
    }

    #[test]
    fn a_press_records_the_matched_rule() {
        let mut w = settled();
        w.set_entities(vec![stub_animal()]);
        w.set_held(HeldThing::Tool("sword".into()));
        assert_eq!(w.last_rule(), None);
        w.tick(&TickInput {
            primary: true,
            ..look(0.0, false)
        });
        assert_eq!(w.last_rule(), Some(0));
        assert_eq!(w.rule_action(0), Some(ActionKind::Attack));
    }

    #[test]
    fn a_block_target_has_a_unit_box_off_the_raft() {
        let mut w = settled();
        w.tick(&look(-std::f32::consts::FRAC_PI_2, false));
        let (b, on_raft) = w.target_box().unwrap();
        assert!(!on_raft);
        assert_eq!((b.min, b.max), ([0.0, 3.0, 0.0], [1.0, 4.0, 1.0]));
    }

    #[test]
    fn horizon_over_open_air_targets_nothing() {
        let mut w = settled();
        w.tick(&look(0.0, false));
        assert_eq!(w.target(), Target::None);
    }

    #[test]
    fn sneaking_lowers_the_eye_and_the_target_follows() {
        // Feet at (0.5, 4, 0.5) on a floor whose top is y = 4. Looking down
        // at slope 0.632 the ray meets the floor 1.62 / 0.632 blocks away
        // when standing and 1.54 / 0.632 when sneaking: z = -2.06 or -1.94.
        let pitch = -(0.632f64.atan()) as f32;
        let mut w = settled();
        let top = |pos| Target::Block {
            pos,
            face: crate::targeting::Face::Top,
        };
        w.tick(&look(pitch, false));
        assert_eq!(w.target(), top([0, 3, -3]));
        w.tick(&look(pitch, true));
        assert_eq!(w.target(), top([0, 3, -2]));
    }

    #[test]
    fn same_inputs_give_the_same_targets() {
        let run = || {
            let mut w = settled();
            let mut out = Vec::new();
            for i in 0..30 {
                w.tick(&TickInput {
                    yaw: i as f32 * 0.3,
                    pitch: -1.0 + i as f32 * 0.05,
                    sneak: i % 3 == 0,
                    ..TickInput::default()
                });
                out.push(w.target());
            }
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn eye_interpolates_between_ticks() {
        let mut w = settled();
        let before = w.eye(1.0);
        w.tick(&TickInput {
            move_forward: 1.0,
            ..TickInput::default()
        });
        let after = w.eye(1.0);
        assert!(after[2] < before[2]);
        assert_eq!(w.eye(0.0), before);
        let mid = w.eye(0.5);
        assert!((mid[2] - (before[2] + after[2]) / 2.0).abs() < 1e-12);
    }

    #[test]
    fn region_set_answers_across_regions_and_is_air_outside() {
        let mut set = RegionSet::default();
        set.push(flat(0, 0, 4, 1));
        set.push(flat(4, 0, 4, 2));
        assert_eq!(set.block_at(1, 1, 1), Block::Stone);
        assert_eq!(set.block_at(5, 2, 1), Block::Stone);
        assert_eq!(set.block_at(1, 2, 1), Block::Air);
        assert_eq!(set.block_at(9, 0, 0), Block::Air);
        assert_eq!(set.block_at(1, -1, 1), Block::Air);
    }

    /// Two adjacent halves of a hilly strip, and one region covering both.
    fn halves() -> (Region, Region, Region) {
        let n = 8u32;
        let fill = |r: &mut Region| {
            for z in 0..r.depth {
                for x in 0..r.width {
                    let wx = r.x0 + x as i32;
                    let top = 3 + (wx.rem_euclid(3)) as u32;
                    for y in 0..=top {
                        let i = r.index(x, y, z);
                        r.blocks[i] = Block::Stone as u8;
                    }
                }
            }
        };
        let mk = |x0: i32, w: u32| {
            let mut r = Region {
                blocks: vec![0u8; w as usize * n as usize * HEIGHT],
                width: w,
                depth: n,
                height: HEIGHT as u32,
                x0,
                z0: -4,
            };
            fill(&mut r);
            r
        };
        (mk(-8, 8), mk(0, 8), mk(-8, 16))
    }

    #[test]
    fn adjacent_regions_match_one_covering_region() {
        let (a, b, both) = halves();
        let mut set = RegionSet::default();
        set.insert(a);
        set.insert(b);
        let one = RegionWorld::new(&both);
        for x in -10..10 {
            for y in -1..10 {
                for z in -6..6 {
                    assert_eq!(set.block_at(x, y, z), one.block_at(x, y, z), "{x} {y} {z}");
                }
            }
        }
        let q = crate::collision::Aabb {
            min: [-0.3, 3.5, -0.3],
            max: [0.3, 5.5, 0.3],
        };
        let (mut s, mut o) = (Vec::new(), Vec::new());
        set.boxes_in(&q, &mut s);
        one.boxes_in(&q, &mut o);
        assert_eq!(s, o);
        assert_eq!(set.body_in_water(&q), one.body_in_water(&q));
    }

    #[test]
    fn removing_a_region_makes_it_air_and_leaves_neighbours() {
        let (a, b, _) = halves();
        let mut set = RegionSet::default();
        set.insert(a);
        set.insert(b);
        assert_eq!(set.block_at(-1, 1, 0), Block::Stone);
        assert!(set.remove(-8, -4).is_some());
        assert!(set.remove(-8, -4).is_none());
        assert_eq!(set.block_at(-1, 1, 0), Block::Air);
        assert_eq!(set.block_at(-8, 1, 0), Block::Air);
        assert_eq!(set.block_at(0, 1, 0), Block::Stone);
        assert_eq!(set.len(), 1);
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        let (a, b, _) = halves();
        w.add_region(a);
        w.add_region(b);
        let mut origins = w.loaded_regions();
        origins.sort();
        assert_eq!(origins, vec![(-8, -4), (0, -4)]);
        w.remove_region(0, -4);
        assert_eq!(w.loaded_regions(), vec![(-8, -4)]);
    }

    #[test]
    fn walking_across_a_border_matches_one_region() {
        let walk = |regions: Vec<Region>| {
            let mut w = Walker::new([-6.5, 8.0, 0.5]).unwrap();
            for r in regions {
                w.add_region(r);
            }
            let east = TickInput {
                move_forward: 1.0,
                yaw: std::f32::consts::FRAC_PI_2,
                jump: true,
                ..TickInput::default()
            };
            let mut path = Vec::new();
            for _ in 0..60 {
                w.tick(&east);
                path.push(w.player().pos);
            }
            path
        };
        let (a, b, both) = halves();
        let split = walk(vec![a, b]);
        let whole = walk(vec![both]);
        assert_eq!(split, whole);
        assert!(split.last().unwrap()[0] > 0.0, "should cross x = 0");
    }

    #[test]
    fn sneaking_stops_at_the_region_edge() {
        let mut w = settled();
        // Walk toward -z (yaw 0) into the region edge at z = -8 while sneaking.
        let sneak = TickInput {
            move_forward: 1.0,
            sneak: true,
            ..TickInput::default()
        };
        for _ in 0..400 {
            w.tick(&sneak);
        }
        assert!(w.player().on_ground && w.player().pos[1] == 4.0);
        assert!(w.player().pos[2] > -8.0 - 0.31);
    }

    fn scene_walker() -> Walker {
        Walker::in_raft_scene(crate::scene::raft_scene()).unwrap()
    }

    #[test]
    fn liquid_at_is_the_water_cell_and_not_the_air_or_floor() {
        // The scene's water fills y = 49..64 (cells 49 to 63), stone below.
        let w = scene_walker();
        let [x, _, z] = w.player().pos;
        assert!(w.liquid_at([x, 63.99, z]));
        assert!(w.liquid_at([x, 49.0, z]));
        assert!(!w.liquid_at([x, 64.0, z]));
        assert!(!w.liquid_at([x, 48.5, z]));
        // Outside every loaded region is air.
        assert!(!w.liquid_at([1000.0, 60.0, 1000.0]));
        // The spawn's eye is above the surface; the feet are in the water.
        assert!(!w.liquid_at(w.eye(1.0)));
        assert!(w.liquid_at(w.player().pos));
    }

    /// Swim at the raft (towards -z) holding jump until standing on it.
    fn boarded() -> Walker {
        let mut w = scene_walker();
        let swim = TickInput {
            move_forward: 1.0,
            jump: true,
            ..TickInput::default()
        };
        for _ in 0..300 {
            w.tick(&swim);
            if w.player().on_ground {
                break;
            }
        }
        assert!(w.player().on_ground, "{:?}", w.player());
        w
    }

    #[test]
    fn scene_player_swims_and_climbs_onto_the_raft() {
        let w = boarded();
        assert!(w.player().pos[1] >= 64.0, "{:?}", w.player());
        let f = w.floating.as_ref().unwrap();
        assert!(f.raft.supports(&w.player().aabb(&w.physics)));
    }

    #[test]
    fn scene_player_stays_on_the_raft_and_sneaking_stops_the_drift() {
        let drift = |sneak: bool| {
            let mut w = boarded();
            let idle = TickInput {
                sneak,
                ..TickInput::default()
            };
            let rel = |w: &Walker| {
                let c = w.floating.as_ref().unwrap().raft.centre();
                [w.player().pos[0] - c[0], w.player().pos[2] - c[1]]
            };
            for _ in 0..20 {
                w.tick(&idle);
            }
            let start = rel(&w);
            let mut far = 0.0f64;
            for _ in 0..60 {
                w.tick(&idle);
                let r = rel(&w);
                far = far.max((r[0] - start[0]).hypot(r[1] - start[1]));
            }
            (far, w.player().on_ground)
        };
        let (moved, _) = drift(false);
        assert!(moved > 0.05, "standing still should drift: {moved}");
        let (still, on) = drift(true);
        assert!(still < 1e-6 && on, "sneaking should hold: {still}");
    }

    #[test]
    fn on_the_raft_looking_down_targets_the_moved_raft() {
        let mut w = boarded();
        w.tick(&look(-std::f32::consts::FRAC_PI_2, false));
        let Target::Block { pos, face } = w.target() else {
            panic!("expected a block, got {:?}", w.target());
        };
        assert_eq!(face, crate::targeting::Face::Top);
        let off = w.raft_offset(1.0);
        let f = w.floating.as_ref().unwrap();
        let feet = w.player().pos;
        assert_eq!(pos[0], feet[0].floor() as i32);
        assert_eq!(pos[2], feet[2].floor() as i32);
        assert!(f.raft.supports(&w.player().aabb(&w.physics)));
        assert!(off.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn raft_offset_interpolates_between_ticks() {
        let mut w = scene_walker();
        assert_eq!(w.raft_offset(1.0), [0.0; 3]);
        w.tick(&TickInput::default());
        let end = w.raft_offset(1.0);
        assert!(end[0] != 0.0 || end[2] != 0.0);
        assert_eq!(w.raft_offset(0.0), [0.0; 3]);
        let mid = w.raft_offset(0.5);
        assert!((mid[0] - end[0] / 2.0).abs() < 1e-12);
    }
}
