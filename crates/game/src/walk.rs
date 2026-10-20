//! The walking player as the client drives it: a `Player` stepped at 20 Hz
//! over loaded terrain regions, plus the eye pose between ticks.
//! See docs/design/basic-gameplay.md sections 1 (rule 1) and 3.

use crate::aliases::Aliases;
use crate::animal_entities::{AnimalStore, DroppedItem};
use crate::animals::Animals;
use crate::collision::{block_selection_boxes_in, Aabb, CollisionWorld, RegionWorld};
use crate::commands::{run_line, GameState, Registry};
use crate::crops::Crops;
use crate::events::GameEvent;
use crate::farm::{FarmTile, FarmWorld};
use crate::forage::{ForageConfig, Forager};
use crate::growth::grow_crops;
use crate::harvest::{harvest, tile_crop_state};
use crate::input::TickInput;
use crate::interactions::{
    resolve, ActionKind, Button, Dispatcher, HeldThing, Interactions, ResolveCtx, Resolved,
    TargetFacts,
};
use crate::item_entities::{Deck, ItemEntityConfig, ItemStore};
use crate::items::{ItemInstance, Items, PlayerState};
use crate::physics::{PhysicsError, PlayerPhysics};
use crate::plant::{crop_for_item, plant};
use crate::player::Player;
use crate::raft::{Raft, RaftWorld, Swell, SwellPush};
use crate::rng::Rng;
use crate::scene::RaftScene;
use crate::stream::{StreamConfig, Streamer};
use crate::targeting::{cast_target, ray_vs_box, Target, TargetableEntity, TargetingConfig};
use crate::till::{TillConfig, Tiller};
use layout::Layout;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::sync::Arc;
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

/// Animal entity ids start here, above the item ids.
const FIRST_ANIMAL_ID: crate::targeting::EntityId = 1 << 24;
/// Fixed seed for animal wandering until the world seed reaches the walker.
const ANIMAL_SEED: u64 = 0;
const FORAGE_SEED: u64 = 1;
const TILL_SEED: u64 = 2;
const HARVEST_SEED: u64 = 3;
const SPAWN_SEED: u64 = 4;
const GROWTH_SEED: u64 = 5;

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
    world: FarmWorld<RegionSet>,
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
    animals: AnimalStore,
    forager: Forager,
    tiller: Tiller,
    crops: Crops,
    harvest_rng: Rng,
    /// Scatter for the `spawn` command.
    spawn_rng: Rng,
    /// Yaw of the last ticked input, so commands can place things ahead.
    look_yaw: f32,
    /// Events emitted since the last `drain_events`.
    events: Vec<GameEvent>,
    /// Ticks stepped so far, for cooldowns.
    sim_tick: u64,
    item_table: Items,
    state: PlayerState,
    /// Test override of the derived held thing.
    held_override: Option<HeldThing>,
    facts: TargetFacts,
    target_box: Option<(Aabb, bool)>,
    /// Command lines submitted since the last tick, in order.
    command_queue: Vec<String>,
    /// Output lines from commands run, until `drain_command_output`.
    command_output: Vec<String>,
    commands: Arc<Registry<Walker>>,
    cheated: bool,
    aliases: Aliases,
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
            world: FarmWorld::new(RegionSet::default()),
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
            animals: AnimalStore::new(
                Animals::default_animals(),
                ItemEntityConfig::default_from_data(),
                FIRST_ANIMAL_ID,
                ANIMAL_SEED,
            ),
            forager: Forager::new(ForageConfig::default_from_data(), FORAGE_SEED),
            tiller: Tiller::new(TillConfig::default_from_data(), TILL_SEED),
            crops: Crops::default_crops(),
            harvest_rng: Rng::new(HARVEST_SEED),
            spawn_rng: Rng::new(SPAWN_SEED),
            look_yaw: 0.0,
            events: Vec::new(),
            sim_tick: 0,
            item_table: Items::default_items(),
            state: PlayerState::new(&Items::default_items())
                .map_err(|e| PhysicsError::Invalid(e.to_string()))?,
            held_override: None,
            facts: TargetFacts::default(),
            target_box: None,
            command_queue: Vec::new(),
            command_output: Vec::new(),
            commands: Arc::new(
                Registry::new(Vec::new()).map_err(|e| PhysicsError::Invalid(e.to_string()))?,
            ),
            cheated: false,
            aliases: Aliases {
                aliases: Vec::new(),
            },
        })
    }

    /// The commands this walker understands (none until a host sets them).
    pub fn set_commands(&mut self, registry: Registry<Walker>) {
        self.commands = Arc::new(registry);
    }

    /// The alias table typed names are expanded from (none until a host sets it).
    pub fn set_aliases(&mut self, aliases: Aliases) {
        self.aliases = aliases;
    }

    /// The alias table, for `help`.
    pub fn aliases(&self) -> &Aliases {
        &self.aliases
    }

    /// Queue a command line; it runs at the start of the next `tick`. An alias
    /// name queues all its lines together, so they run in the same tick, in
    /// order. Expanded lines are plain commands and are not expanded again.
    pub fn submit_command(&mut self, line: &str) {
        let mut words = line.split_whitespace();
        let alias = match (words.next(), words.next()) {
            (Some(name), None) => self.aliases.get(name),
            _ => None,
        };
        match alias {
            Some(a) => self.command_queue.extend(a.expands.iter().cloned()),
            None => self.command_queue.push(line.to_string()),
        }
    }

    /// The output lines of commands run since the last call.
    pub fn drain_command_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.command_output)
    }

    /// Whether a cheat command has run this session.
    pub fn cheated(&self) -> bool {
        self.cheated
    }

    pub fn add_region(&mut self, region: Region) {
        self.world.base_mut().insert(region);
    }

    /// Unload the region whose origin is `(x0, z0)`.
    pub fn remove_region(&mut self, x0: i32, z0: i32) -> Option<Region> {
        self.world.base_mut().remove(x0, z0)
    }

    /// Origins `(x0, z0)` of the loaded regions, in no particular order.
    pub fn loaded_regions(&self) -> Vec<(i32, i32)> {
        self.world.base().loaded().collect()
    }

    /// A walker with its feet at `feet` over a streamed world: the player's
    /// region and its neighbours are loaded now, and regions load and unload
    /// as the player crosses borders (radii from `data/streaming.toml`).
    pub fn with_layout(layout: Layout, feet: [f64; 3]) -> Result<Walker, String> {
        let mut w = Walker::new(feet).map_err(|e| e.to_string())?;
        let mut streamer = Streamer::new(layout, StreamConfig::default_from_data()?);
        streamer.update(feet, w.world.base_mut());
        w.streamer = Some(streamer);
        Ok(w)
    }

    /// Number of regions currently loaded for collision.
    pub fn loaded_region_count(&self) -> usize {
        self.world.base().len()
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
        !self.world.base().is_empty()
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    /// The yaw of the last ticked input.
    pub fn look_yaw(&self) -> f32 {
        self.look_yaw
    }

    /// The sim RNG that `spawn` scatters with.
    pub fn spawn_rng_mut(&mut self) -> &mut Rng {
        &mut self.spawn_rng
    }

    /// Whether `name` is a loose-item name in the item table.
    pub fn knows_item(&self, name: &str) -> bool {
        self.item_table.has_item(name)
    }

    /// Whether `name` is a tool in the item table.
    pub fn knows_tool(&self, name: &str) -> bool {
        self.item_table.has_tool(name)
    }

    fn run_queued_commands(&mut self) {
        let lines = std::mem::take(&mut self.command_queue);
        let registry = Arc::clone(&self.commands);
        for line in lines {
            let mut cheated = self.cheated;
            let out = run_line(&registry, self, GameState::Walking, &line, &mut cheated);
            self.cheated = cheated;
            self.command_output.extend(out);
        }
    }

    /// One 20 Hz step.
    pub fn tick(&mut self, input: &TickInput) {
        self.run_queued_commands();
        self.look_yaw = input.yaw;
        self.sim_tick += 1;
        grow_crops(&mut self.world, &self.crops, GROWTH_SEED, self.sim_tick);
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
                let deck = Deck {
                    raft: &f.raft,
                    tilt: frame.tilt,
                    push: &f.push,
                };
                self.items.tick(&world, Some(deck));
                let dropped = self.animals.tick(
                    &world,
                    Some(Deck {
                        raft: &f.raft,
                        tilt: frame.tilt,
                        push: &f.push,
                    }),
                );
                Self::spawn_drops(&mut self.items, dropped);
                let aim = aim(&world, &entities, &self.targeting, self.cur_eye, input);
                self.take_aim(aim, input);
            }
            None => {
                self.player.tick(input, &self.physics, &self.world);
                self.prev_eye = self.cur_eye;
                self.cur_eye = eye_above(self.player.pos, &self.physics, input.sneak);
                self.items.tick(&self.world, None);
                let dropped = self.animals.tick(&self.world, None);
                Self::spawn_drops(&mut self.items, dropped);
                let aim = aim(&self.world, &entities, &self.targeting, self.cur_eye, input);
                self.take_aim(aim, input);
            }
        }
        if let Some(s) = &mut self.streamer {
            s.update(self.player.pos, self.world.base_mut());
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

    /// Put an animal of `species` in the world, returning its entity id, or
    /// `None` for an unknown species.
    pub fn spawn_animal(
        &mut self,
        species: &str,
        pos: [f64; 3],
        facing: f64,
    ) -> Option<crate::targeting::EntityId> {
        self.animals.spawn(species, pos, facing)
    }

    pub fn animals(&self) -> &AnimalStore {
        &self.animals
    }

    /// Put the belt, selection and hands back to the v0 start (`clear belt`
    /// and `clear hands` use the parts they name).
    pub fn reset_belt(&mut self) -> Result<(), crate::items::ItemsError> {
        let fresh = PlayerState::new(&self.item_table)?;
        self.state.tool_belt = fresh.tool_belt;
        self.state.selected = fresh.selected;
        Ok(())
    }

    pub fn clear_hands(&mut self) {
        self.state.hands = None;
    }

    pub fn clear_items(&mut self) {
        self.items.clear();
    }

    pub fn clear_animals(&mut self) {
        self.animals.clear();
    }

    /// Loose items for what dying animals dropped.
    fn spawn_drops(items: &mut ItemStore, dropped: Vec<DroppedItem>) {
        for d in dropped {
            items.spawn(ItemInstance { item: d.item }, d.pos, d.velocity);
        }
    }

    /// Take the events emitted since the last call, oldest first.
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// The carried item, if any.
    pub fn hands(&self) -> Option<&ItemInstance> {
        self.state.hands.as_ref()
    }

    /// Stub entities plus the loose items and animals, as the eye ray sees them.
    fn all_entities(&self) -> Vec<TargetableEntity> {
        let mut all = self.entities.clone();
        all.extend(self.items.targetables());
        all.extend(self.animals.targetables());
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
        // Holding secondary with the hoe tills again on a newly targeted block.
        if let (true, HeldThing::Tool(tool), Target::Block { pos, .. }) =
            (input.secondary_held, &ctx.held, ctx.target)
        {
            pressed.secondary |= tool == "hoe" && self.tiller.repeat_due(pos, self.sim_tick);
        }
        let damage = match &ctx.held {
            HeldThing::Tool(name) => self.item_table.damage(Some(name)),
            _ => self.item_table.damage(None),
        };
        let ran = self
            .dispatcher
            .dispatch(&self.rules, &pressed, &ctx, damage);
        if let (true, Target::Block { pos, .. }) = (ran.contains(&ActionKind::Forage), ctx.target) {
            if let Some(item) = self.forager.forage(pos, self.sim_tick) {
                let item = item.to_string();
                let at = pos.map(|c| f64::from(c) + 0.5);
                self.items
                    .spawn(ItemInstance { item: item.clone() }, at, [0.0; 3]);
                self.events.push(GameEvent::Foraged {
                    position: pos,
                    yields: vec![item],
                });
            }
        }
        if let (true, Target::Block { pos, .. }) = (ran.contains(&ActionKind::Till), ctx.target) {
            if let Some(Some(item)) = self.tiller.till(&mut self.world, pos, self.sim_tick) {
                let at = [
                    pos[0] as f64 + 0.5,
                    pos[1] as f64 + 1.0,
                    pos[2] as f64 + 0.5,
                ];
                let item = ItemInstance {
                    item: item.to_string(),
                };
                self.items.spawn(item, at, [0.0; 3]);
            }
        }
        if let (true, Target::Block { pos, face }) = (ran.contains(&ActionKind::Plant), ctx.target)
        {
            // The seed leaves the hands only if a crop went in.
            let kind = self
                .state
                .hands
                .as_ref()
                .and_then(|h| crop_for_item(&h.item));
            if kind.is_some_and(|kind| plant(&mut self.world, pos, face, kind)) {
                self.state.hands = None;
            }
        }
        if let (true, Target::Block { pos, .. }) = (ran.contains(&ActionKind::Harvest), ctx.target)
        {
            let drops = harvest(&mut self.world, &self.crops, &mut self.harvest_rng, pos);
            let at = pos.map(|c| f64::from(c) + 0.5);
            for item in drops.into_iter().flatten() {
                self.items.spawn(ItemInstance { item }, at, [0.0; 3]);
            }
        }
        for hit in &self.dispatcher.hits {
            self.animals.hit(hit.id, hit.damage, self.player.pos);
        }
        self.facts = ctx.facts;
        if input.interact {
            self.interact();
        } else if input.drop {
            self.drop_carried(input);
        }
    }

    /// Drop throws the carried item from the eye along the look direction.
    /// With empty hands it does nothing.
    fn drop_carried(&mut self, input: &TickInput) {
        let Some(carried) = self.state.hands.take() else {
            return;
        };
        let speed = self.items.config().throw_speed;
        let velocity = look_dir(input).map(|c| c * speed);
        self.items.spawn(carried, self.cur_eye, velocity);
    }

    /// Interact picks up the targeted loose item if the hands are empty.
    /// With full hands it does nothing: put-down arrives with stations
    /// and counters (decided in #215); only Drop releases the item.
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

    /// The block at a world position as the game sees it: generated terrain
    /// with the farm overlay on top.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        self.world.block_at(x, y, z)
    }

    /// The farm overlay's blocks and positions, sorted by position.
    pub fn farm_tiles(&self) -> Vec<([i32; 3], Block)> {
        self.world.tiles()
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

    /// Put the player standing on the highest non-air block of column
    /// `(x, z)`, streaming that column's region in first. Errors, changing
    /// nothing, if the column is not in a loaded region.
    pub fn teleport_to_surface(&mut self, x: i32, z: i32) -> Result<[f64; 3], String> {
        let (cx, cz) = (x as f64 + 0.5, z as f64 + 0.5);
        if let Some(s) = &mut self.streamer {
            // Load the destination without moving the player yet.
            s.update([cx, 0.0, cz], self.world.base_mut());
        }
        if self.world.base().region_at(x, z).is_none() {
            return Err(format!("no world loaded at {x} {z}"));
        }
        let top = (0..HEIGHT as i32)
            .rev()
            .find(|&y| self.world.block_at(x, y, z) != Block::Air)
            .ok_or_else(|| format!("nothing to stand on at {x} {z}"))?;
        let feet = [cx, top as f64 + 1.0, cz];
        self.teleport(feet);
        Ok(feet)
    }

    /// Put the player at `feet` with no velocity, e.g. when leaving fly mode.
    pub fn teleport(&mut self, feet: [f64; 3]) {
        self.player = Player::new(feet, &self.physics);
        if let Some(s) = &mut self.streamer {
            s.update(feet, self.world.base_mut());
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

/// Unit look vector, the same as `Pose::forward` in camera.rs, in f64.
fn look_dir(input: &TickInput) -> [f64; 3] {
    let (sy, cy) = (input.yaw as f64).sin_cos();
    let (sp, cp) = (input.pitch as f64).sin_cos();
    [sy * cp, sp, -cy * cp]
}

/// The name rules use for a block: both farmland blocks are `farmland`.
fn rule_block_name(block: Block) -> String {
    match FarmTile::from_block(block) {
        Some(FarmTile::Farmland { .. }) => "farmland".to_string(),
        Some(FarmTile::Crop { .. }) => "crop".to_string(),
        _ => block.info().name.to_string(),
    }
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
    let dir = look_dir(input);
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
                    block: Some(rule_block_name(world.block_at(pos[0], pos[1], pos[2]))),
                    air_above: world.block_at(pos[0], pos[1] + 1, pos[2]) == Block::Air,
                    crop_state: FarmTile::from_block(world.block_at(pos[0], pos[1], pos[2]))
                        .and_then(tile_crop_state)
                        .map(String::from),
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

    #[test]
    fn the_eye_ray_targets_a_spawned_animal_and_the_sword_attacks() {
        let mut w = settled();
        w.player_state_mut().select(1);
        let id = w.spawn_animal("cow", [0.5, 4.0, -2.0], 0.0).unwrap();
        assert!(w.spawn_animal("nope", [0.5, 4.0, -2.0], 0.0).is_none());
        // The eye is above the animal's back: look down at it.
        w.tick(&look(-0.4, false));
        assert_eq!(w.target(), Target::Entity { id });
        assert_eq!(
            w.preview(Button::Primary).map(|r| r.action),
            Some(ActionKind::Attack)
        );
    }

    fn cow_health(w: &Walker, id: crate::targeting::EntityId) -> u32 {
        w.animals().get(id).unwrap().health
    }

    fn click() -> TickInput {
        TickInput {
            primary: true,
            ..look(-0.4, false)
        }
    }

    #[test]
    fn one_press_hits_with_the_sword_for_four_and_the_hand_for_one() {
        let mut w = settled();
        let id = w.spawn_animal("cow", [0.5, 4.0, -2.0], 0.0).unwrap();
        let full = cow_health(&w, id);
        w.tick(&look(-0.4, false));
        assert_eq!(w.target(), Target::Entity { id });
        w.tick(&click());
        assert_eq!(cow_health(&w, id), full - 1);
        // Let the invulnerability window close, then use the sword.
        w.player_state_mut().select(1);
        for _ in 0..11 {
            w.tick(&look(-0.4, false));
        }
        // The hit knocked the cow back; step after it to keep the aim.
        let (cow, me) = (w.animals().get(id).unwrap().pos, w.player().pos);
        w.teleport([me[0], me[1], cow[2] + 2.0]);
        w.tick(&look(-0.4, false));
        w.tick(&click());
        assert_eq!(cow_health(&w, id), full - 5);
    }

    #[test]
    fn holding_does_not_repeat_and_a_latched_click_hits_once() {
        let mut w = settled();
        w.player_state_mut().select(1);
        let id = w.spawn_animal("cow", [0.5, 4.0, -2.0], 0.0).unwrap();
        let full = cow_health(&w, id);
        w.tick(&look(-0.4, false));
        // A click latched between ticks arrives as one edge on the next tick.
        w.tick(&click());
        assert_eq!(cow_health(&w, id), full - 4);
        for _ in 0..40 {
            w.tick(&look(-0.4, false));
        }
        assert_eq!(cow_health(&w, id), full - 4);
    }

    #[test]
    fn primary_on_a_block_or_nothing_changes_no_animal() {
        let mut w = settled();
        w.player_state_mut().select(1);
        let id = w.spawn_animal("cow", [0.5, 4.0, -2.0], 0.0).unwrap();
        let full = cow_health(&w, id);
        for pitch in [-1.5, 0.0] {
            w.tick(&TickInput {
                primary: true,
                ..look(pitch, false)
            });
            assert!(!matches!(w.target(), Target::Entity { .. }));
            assert_eq!(cow_health(&w, id), full);
        }
    }

    #[test]
    fn animal_ids_do_not_collide_with_items_or_stubs() {
        let mut w = settled();
        w.set_entities(vec![stub_animal()]);
        let item = w.spawn_item(seeds(), [3.5, 4.0, 3.5]);
        let a = w.spawn_animal("pig", [-3.5, 4.0, 3.5], 0.0).unwrap();
        let b = w.spawn_animal("pig", [-3.5, 4.0, -3.5], 0.0).unwrap();
        let mut ids: Vec<_> = w.all_entities().iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), 4);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4);
        assert!(a > item && b > a && item > 1);
    }

    #[test]
    fn animals_settle_on_the_ground() {
        let mut w = settled();
        let id = w.spawn_animal("cow", [3.5, 7.0, 3.5], 0.0).unwrap();
        for _ in 0..60 {
            w.tick(&TickInput::default());
        }
        assert_eq!(w.animals().get(id).unwrap().pos[1], 4.0);
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
    fn a_killed_chicken_drops_items_that_are_picked_up_one_at_a_time() {
        let mut w = settled();
        w.player_state_mut().select(1);
        let id = w.spawn_animal("chicken", [0.5, 4.0, -2.0], 0.0).unwrap();
        w.tick(&look(-0.4, false));
        w.tick(&click());
        assert!(w.animals().get(id).is_some(), "plays the death animation");
        assert!(w.items().is_empty());
        for _ in 0..25 {
            w.tick(&look(0.0, false));
        }
        assert!(w.animals().get(id).is_none());
        let ids: Vec<_> = w.items().iter().map(|e| e.id).collect();
        assert!(!ids.is_empty());
        assert!(w.items().iter().any(|e| e.item.item == "raw_chicken"));
        // Park the first drop in front of the eye and take it.
        let first = ids[0];
        let e = w.items.get_mut(first).unwrap();
        e.pos = [0.5, 5.62, -1.0];
        e.velocity = [0.0; 3];
        let name = e.item.item.clone();
        w.tick(&look(0.0, false));
        w.tick(&interact());
        assert_eq!(w.hands().map(|h| h.item.clone()), Some(name));
        assert_eq!(w.items().len(), ids.len() - 1);
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

    /// Decided in #215: with stations absent, interact while carrying is a
    /// no-op whatever is aimed at; the item is neither put down nor lost.
    #[test]
    fn interact_while_carrying_never_puts_down_or_loses_the_item() {
        let mut w = settled();
        let far = w.spawn_item(seeds(), [0.5, 5.62, -6.0]);
        w.state.hands = Some(seeds());
        let mut aims = vec![interact()];
        let mut up = interact();
        up.pitch = 1.2;
        aims.push(up);
        let mut down = interact();
        down.pitch = -1.5;
        aims.push(down);
        for input in &aims {
            w.tick(input);
            assert_eq!(w.hands(), Some(&seeds()));
            assert_eq!(w.items().len(), 1);
            assert!(w.items().get(far).is_some());
        }
        assert_ne!(w.target(), Target::None, "last aim targets a block");
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

    fn drop_input(yaw: f32, pitch: f32) -> TickInput {
        TickInput {
            drop: true,
            yaw,
            pitch,
            ..TickInput::default()
        }
    }

    #[test]
    fn drop_throws_the_item_along_the_look_direction() {
        let speed = ItemEntityConfig::default_from_data().throw_speed;
        for (yaw, pitch) in [(0.0f32, 0.0f32), (1.0, 0.4), (-2.5, -0.7), (3.0, 1.2)] {
            let mut w = settled();
            w.state.hands = Some(seeds());
            w.tick(&drop_input(yaw, pitch));
            assert_eq!(w.hands(), None);
            assert_eq!(w.items().len(), 1);
            let e = w.items().iter().next().unwrap();
            assert_eq!(e.item, seeds());
            assert_eq!(e.pos, w.eye(1.0));
            let (sy, cy) = (f64::from(yaw).sin(), f64::from(yaw).cos());
            let (sp, cp) = (f64::from(pitch).sin(), f64::from(pitch).cos());
            let want = [sy * cp, sp, -cy * cp].map(|c| c * speed);
            for i in 0..3 {
                assert!((e.velocity[i] - want[i]).abs() < 1e-9, "{yaw} {pitch}");
            }
        }
    }

    #[test]
    fn drop_with_empty_hands_does_nothing() {
        let mut w = settled();
        w.tick(&drop_input(0.0, 0.0));
        assert_eq!(w.hands(), None);
        assert!(w.items().is_empty());
    }

    #[test]
    fn dropped_item_lands_and_can_be_picked_up_again() {
        let mut w = settled();
        w.state.hands = Some(seeds());
        w.tick(&drop_input(0.0, -0.3));
        for _ in 0..80 {
            w.tick(&TickInput::default());
        }
        let e = w.items().iter().next().unwrap().clone();
        assert!(e.velocity.iter().all(|v| *v == 0.0), "came to rest");
        // Look at the landed item from the eye.
        let eye = w.eye(1.0);
        let d = [0, 1, 2].map(|i| e.pos[i] - eye[i]);
        let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
        let aim = TickInput {
            interact: true,
            yaw: d[0].atan2(-d[2]) as f32,
            pitch: d[1].atan2(flat) as f32,
            ..TickInput::default()
        };
        w.tick(&aim);
        assert_eq!(w.hands(), Some(&seeds()));
        assert!(w.items().is_empty());
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

    fn grass_walker(chance: u32) -> Walker {
        let mut region = flat(-8, -8, 16, 3);
        let i = region.index(8, 5, 6); // world (0, 5, -2), straight ahead
        region.blocks[i] = Block::TallGrass as u8;
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.add_region(region);
        for _ in 0..40 {
            w.tick(&TickInput::default());
        }
        let cfg = ForageConfig {
            chance,
            cooldown_ticks: 100,
            ..ForageConfig::default_from_data()
        };
        w.forager = Forager::new(cfg, 3);
        w
    }

    fn press_secondary() -> TickInput {
        TickInput {
            secondary: true,
            ..TickInput::default()
        }
    }

    #[test]
    fn forage_drops_seeds_once_per_cooldown_and_keeps_the_grass() {
        let mut w = grass_walker(1);
        w.tick(&look(0.0, false));
        assert_eq!(
            w.target(),
            Target::Block {
                pos: [0, 5, -2],
                face: crate::targeting::Face::South
            }
        );
        w.tick(&press_secondary());
        assert_eq!(w.items().len(), 1);
        assert_eq!(w.items().iter().next().unwrap().item.item, "seeds");
        assert_eq!(
            w.target(),
            Target::Block {
                pos: [0, 5, -2],
                face: crate::targeting::Face::South
            }
        );
        w.tick(&press_secondary());
        assert_eq!(w.items().len(), 1, "inside the cooldown");
        for _ in 0..100 {
            w.tick(&look(0.0, false));
        }
        w.tick(&press_secondary());
        assert_eq!(w.items().len(), 2, "after the cooldown");
    }

    #[test]
    fn forage_needs_empty_hands() {
        let mut w = grass_walker(1);
        w.set_held(HeldThing::Tool("hoe".into()));
        w.tick(&look(0.0, false));
        w.tick(&press_secondary());
        assert_eq!(w.items().len(), 0);
        w.clear_held_override();
        w.state.hands = Some(seeds());
        w.tick(&press_secondary());
        assert_eq!(w.items().len(), 0);
    }

    #[test]
    fn forage_replays_identically() {
        let run = || {
            let mut w = grass_walker(2);
            for _ in 0..600 {
                w.tick(&look(0.0, false));
                w.tick(&press_secondary());
            }
            w.items().len()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn forage_emits_foraged_only_on_success() {
        let mut w = grass_walker(1);
        w.tick(&look(0.0, false));
        assert!(w.drain_events().is_empty());
        w.tick(&press_secondary());
        assert_eq!(
            w.drain_events(),
            vec![GameEvent::Foraged {
                position: [0, 5, -2],
                yields: vec!["seeds".to_string()],
            }]
        );
        w.tick(&press_secondary());
        assert!(w.drain_events().is_empty(), "blocked by the cooldown");

        // A failed roll: chance so low that the first rolls miss.
        let mut w = grass_walker(1_000_000);
        w.tick(&look(0.0, false));
        w.tick(&press_secondary());
        assert_eq!(w.items().len(), 0);
        assert!(w.drain_events().is_empty(), "failed roll");
    }

    #[test]
    fn forage_events_replay_identically() {
        let run = || {
            let mut w = grass_walker(2);
            let mut seen = Vec::new();
            for _ in 0..600 {
                w.tick(&look(0.0, false));
                w.tick(&press_secondary());
                seen.extend(w.drain_events());
            }
            (seen, w.items().len())
        };
        let (a, n) = run();
        assert!(!a.is_empty());
        assert_eq!(a.len(), n, "one event per drop");
        assert_eq!((a, n), run());
    }

    fn slot(n: u8) -> TickInput {
        TickInput {
            tool_slot: Some(n),
            ..TickInput::default()
        }
    }

    fn belt_cmd(name: &'static str, cheat: bool) -> crate::commands::Command<Walker> {
        use crate::commands::*;
        Command {
            name,
            aliases: &[],
            args: &[ArgSpec {
                name: "slot",
                kind: ArgKind::Int,
                optional: false,
            }],
            help: "select a belt slot",
            states: &[GameState::Walking],
            cheat,
            handler: |ctx, args| {
                let Some(Arg::Int(n)) = args.first() else {
                    return Err("missing".into());
                };
                ctx.world.player_state_mut().select(*n as u8);
                Ok(vec![format!("selected {n}")])
            },
        }
    }

    fn with_cmds() -> Walker {
        let mut w = settled();
        w.set_commands(
            Registry::new(vec![belt_cmd("sel", false), belt_cmd("csel", true)]).unwrap(),
        );
        w
    }

    #[test]
    fn commands_wait_for_the_tick_and_run_before_input() {
        let mut w = with_cmds();
        w.submit_command("sel 1");
        assert_eq!(w.player_state().selected, 0);
        assert!(w.drain_command_output().is_empty());
        w.submit_command("sel 2");
        w.tick(&slot(1));
        // Commands ran in order, then the input's slot won.
        assert_eq!(w.player_state().selected, 1);
        assert_eq!(w.drain_command_output(), vec!["selected 1", "selected 2"]);
        assert!(w.drain_command_output().is_empty());
        w.tick(&TickInput::default());
        assert!(w.drain_command_output().is_empty());
    }

    #[test]
    fn errors_are_output_and_cheats_are_flagged() {
        let mut w = with_cmds();
        w.submit_command("nope");
        w.submit_command("sel 1");
        w.tick(&TickInput::default());
        assert_eq!(w.drain_command_output().len(), 2);
        assert!(!w.cheated());
        w.submit_command("csel 2");
        w.tick(&TickInput::default());
        assert!(w.cheated());
    }

    fn alias_walker(toml: &str) -> Walker {
        let mut w = with_cmds();
        let reg = Registry::new(vec![belt_cmd("sel", false), belt_cmd("csel", true)]).unwrap();
        let aliases = Aliases::from_toml(toml, &reg).unwrap();
        w.set_aliases(aliases);
        w
    }

    const TWO: &str = "[[alias]]\nname = \"two\"\nexpands = [\"sel 1\", \"sel 2\"]\n\
        [[alias]]\nname = \"cheaty\"\nexpands = [\"nope\", \"csel 3\", \"sel 1\"]\n\
        [[alias]]\nname = \"plain\"\nexpands = [\"nope\", \"sel 4\"]\n";

    #[test]
    fn alias_runs_its_lines_in_order_before_input() {
        let mut w = alias_walker(TWO);
        w.submit_command("two");
        assert!(w.drain_command_output().is_empty());
        w.tick(&slot(4));
        assert_eq!(w.drain_command_output(), vec!["selected 1", "selected 2"]);
        assert_eq!(w.player_state().selected, 4);
        assert!(!w.cheated());
    }

    #[test]
    fn alias_matches_typing_its_lines() {
        let mut a = alias_walker(TWO);
        a.submit_command("cheaty");
        a.tick(&TickInput::default());
        let mut b = alias_walker(TWO);
        for l in ["nope", "csel 3", "sel 1"] {
            b.submit_command(l);
        }
        b.tick(&TickInput::default());
        assert_eq!(a.drain_command_output(), b.drain_command_output());
        assert_eq!(a.player_state().selected, b.player_state().selected);
        assert_eq!(a.cheated(), b.cheated());
    }

    #[test]
    fn alias_error_is_reported_and_later_lines_run() {
        let mut w = alias_walker(TWO);
        w.submit_command("plain");
        w.tick(&TickInput::default());
        let out = w.drain_command_output();
        assert_eq!(out.len(), 2);
        assert!(out[0].contains("unknown command"));
        assert_eq!(out[1], "selected 4");
        assert!(!w.cheated(), "no cheat line succeeded");
        w.submit_command("cheaty");
        w.tick(&TickInput::default());
        assert!(w.cheated());
    }

    #[test]
    fn same_inputs_and_commands_give_the_same_state() {
        let run = || {
            let mut w = with_cmds();
            w.submit_command("sel 2");
            w.tick(&TickInput::default());
            w.submit_command("csel 1");
            w.tick(&slot(2));
            (
                w.player_state().selected,
                w.cheated(),
                w.drain_command_output(),
            )
        };
        assert_eq!(run(), run());
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
    fn an_animal_rides_the_raft_deck() {
        let mut w = boarded();
        let c = w.floating.as_ref().unwrap().raft.centre();
        let y = w.player().pos[1];
        let id = w.spawn_animal("pig", [c[0], y + 1.0, c[1]], 0.0).unwrap();
        for _ in 0..60 {
            w.tick(&TickInput::default());
        }
        let f = w.floating.as_ref().unwrap();
        let e = w.animals().get(id).unwrap();
        assert!(f.raft.supports(&e.aabb()), "{:?}", e.pos);
        assert!((e.pos[1] - y).abs() < 1.0, "{:?} vs {y}", e.pos);
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

    fn grass_floor_walker(chance: u32) -> Walker {
        let mut region = flat(-8, -8, 16, 3);
        for z in 0..16 {
            for x in 0..16 {
                let i = region.index(x, 3, z);
                region.blocks[i] = Block::Grass as u8;
            }
        }
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.add_region(region);
        for _ in 0..40 {
            w.tick(&TickInput::default());
        }
        let cfg = TillConfig {
            chance,
            ..TillConfig::default_from_data()
        };
        w.tiller = Tiller::new(cfg, 5);
        w.set_held(HeldThing::Tool("hoe".into()));
        w
    }

    fn down(secondary: bool, held: bool) -> TickInput {
        TickInput {
            pitch: -std::f32::consts::FRAC_PI_2,
            secondary,
            secondary_held: held,
            ..TickInput::default()
        }
    }

    fn tilled(w: &Walker, x: i32) -> bool {
        w.world.get(x, 3, 0).is_some()
    }

    #[test]
    fn hoe_tills_the_targeted_grass_and_nothing_else() {
        let mut w = grass_floor_walker(1);
        w.tick(&down(false, false));
        assert!(!tilled(&w, 0), "no press, no till");
        w.tick(&down(true, true));
        assert_eq!(w.world.block_at(0, 3, 0), Block::FarmlandDry);
        assert_eq!(w.world.block_at(1, 3, 0), Block::Grass);
        assert_eq!(w.world.block_at(0, 2, 0), Block::Stone);
        assert_eq!(w.items().len(), 1, "chance 1 always drops seeds");
    }

    #[test]
    fn other_tools_and_empty_hands_do_not_till() {
        for held in [HeldThing::Empty, HeldThing::Tool("sword".into())] {
            let mut w = grass_floor_walker(1);
            w.set_held(held);
            w.tick(&down(true, true));
            assert!(!tilled(&w, 0));
            assert_eq!(w.items().len(), 0);
        }
    }

    #[test]
    fn holding_repeats_every_four_ticks_on_new_blocks_only() {
        let mut w = grass_floor_walker(8);
        w.tick(&down(true, true));
        assert!(tilled(&w, 0));
        // Same block while held: no repeat, however long.
        for _ in 0..8 {
            w.tick(&down(false, true));
        }
        assert!(!tilled(&w, 1));
        // A new block is tilled once the interval since the last till is up.
        w.teleport([1.5, 4.0, 0.5]);
        w.tick(&down(false, true));
        assert!(tilled(&w, 1));
        w.teleport([2.5, 4.0, 0.5]);
        for _ in 0..3 {
            w.tick(&down(false, true));
        }
        assert!(!tilled(&w, 2), "inside the interval");
        w.tick(&down(false, true));
        assert!(tilled(&w, 2), "after the interval");
    }

    #[test]
    fn seeds_plant_on_farmland_top_and_leave_the_hands() {
        let mut w = grass_floor_walker(8);
        w.tick(&down(true, true));
        assert_eq!(w.world.block_at(0, 3, 0), Block::FarmlandDry);
        w.clear_held_override();
        w.state.hands = Some(ItemInstance {
            item: "seeds".into(),
        });
        w.tick(&down(true, false));
        assert_eq!(w.world.block_at(0, 4, 0), Block::Wheat0);
        assert!(w.hands().is_none());
        assert_eq!(w.items().len(), 0);
    }

    #[test]
    fn seeds_stay_when_nothing_can_be_planted() {
        let seeds = || {
            Some(ItemInstance {
                item: "seeds".into(),
            })
        };
        // Untilled grass.
        let mut w = grass_floor_walker(8);
        w.clear_held_override();
        w.state.hands = seeds();
        w.tick(&down(true, false));
        assert_eq!(w.world.block_at(0, 4, 0), Block::Air);
        assert!(w.hands().is_some());
        // An already planted tile: the crop is targeted, not the farmland.
        let mut w = grass_floor_walker(8);
        w.tick(&down(true, true));
        w.clear_held_override();
        w.state.hands = seeds();
        w.tick(&down(true, false));
        w.state.hands = seeds();
        w.tick(&down(true, false));
        assert_eq!(w.world.block_at(0, 4, 0), Block::Wheat0);
        assert!(w.hands().is_some(), "second seed is kept");
    }

    fn crop_walker(stage: u32) -> Walker {
        let mut w = grass_floor_walker(8);
        w.tick(&down(true, true));
        let kind = crate::farm::CropKind::Wheat;
        w.world.set(0, 4, 0, FarmTile::Crop { kind, stage });
        w
    }

    fn mature() -> u32 {
        crate::farm::CropKind::Wheat.stages() - 1
    }

    #[test]
    fn mature_crop_drops_yields_and_resets_with_any_tool_or_empty_hands() {
        for held in [
            HeldThing::Empty,
            HeldThing::Tool("hoe".into()),
            HeldThing::Tool("sword".into()),
        ] {
            let mut w = crop_walker(mature());
            w.held_override = Some(held);
            w.tick(&down(true, false));
            assert_eq!(w.world.block_at(0, 4, 0), Block::Wheat0);
            let names: Vec<_> = w.items().iter().map(|e| e.item.item.clone()).collect();
            assert_eq!(names[0], "wheat");
            assert!(names.len() <= 3 && names[1..].iter().all(|n| n == "seeds"));
        }
    }

    #[test]
    fn immature_crop_is_left_alone() {
        let mut w = crop_walker(mature() - 1);
        w.tick(&down(true, false));
        assert_eq!(w.world.block_at(0, 4, 0), Block::Wheat6);
        assert_eq!(w.items().len(), 0);
    }

    #[test]
    fn carrying_an_item_blocks_harvest() {
        let mut w = crop_walker(mature());
        w.clear_held_override();
        w.state.hands = Some(ItemInstance {
            item: "wheat".into(),
        });
        w.tick(&down(true, false));
        assert_eq!(w.world.block_at(0, 4, 0), Block::Wheat7);
        assert_eq!(w.items().len(), 0);
    }

    #[test]
    fn harvest_replays_identically() {
        let run = || {
            let mut w = crop_walker(mature());
            w.tick(&down(true, false));
            w.items()
                .iter()
                .map(|e| e.item.item.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn non_seed_items_are_not_planted() {
        let mut w = grass_floor_walker(8);
        w.tick(&down(true, true));
        w.clear_held_override();
        w.state.hands = Some(ItemInstance {
            item: "leather".into(),
        });
        w.tick(&down(true, false));
        assert_eq!(w.world.block_at(0, 4, 0), Block::Air);
        assert!(w.hands().is_some());
    }
}
