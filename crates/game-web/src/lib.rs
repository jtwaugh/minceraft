//! wasm bindings for the input sampler, the sim clock, the walking player and
//! the debug fly camera. The DOM side is `client/input.js`; the render loop is
//! `client/scene.js`.

use game::aliases::Aliases;
use game::animals::Animals;
use game::bindings::{Action, Bindings};
use game::camera::{Camera, Pose};
use game::clock::Clock;
use game::commands::default_registry;
use game::events::GameEvent;
use game::input::TickInput;
use game::interactions::{Button, HeldThing};
use game::items::{ItemInstance, Items};
use game::sampler::Sampler;
use game::scene::{raft_scene, RaftScene, FIELD_SIZE, FIELD_Y, SEA_MIN, SEA_SIZE};
use game::walk::Walker;
use layout::{Layout, RegionCoord};
use terrain::{Block, Region};
use wasm_bindgen::prelude::*;

/// One sim tick's input, as plain getters (see `TickInput`).
#[wasm_bindgen]
pub struct Tick(TickInput);

#[wasm_bindgen]
impl Tick {
    #[wasm_bindgen(getter)]
    pub fn move_forward(&self) -> f32 {
        self.0.move_forward
    }
    #[wasm_bindgen(getter)]
    pub fn move_strafe(&self) -> f32 {
        self.0.move_strafe
    }
    #[wasm_bindgen(getter)]
    pub fn yaw(&self) -> f32 {
        self.0.yaw
    }
    #[wasm_bindgen(getter)]
    pub fn pitch(&self) -> f32 {
        self.0.pitch
    }
    #[wasm_bindgen(getter)]
    pub fn jump(&self) -> bool {
        self.0.jump
    }
    #[wasm_bindgen(getter)]
    pub fn sneak(&self) -> bool {
        self.0.sneak
    }
    #[wasm_bindgen(getter)]
    pub fn primary(&self) -> bool {
        self.0.primary
    }
    #[wasm_bindgen(getter)]
    pub fn secondary(&self) -> bool {
        self.0.secondary
    }
    #[wasm_bindgen(getter)]
    pub fn secondary_held(&self) -> bool {
        self.0.secondary_held
    }
    #[wasm_bindgen(getter)]
    pub fn interact(&self) -> bool {
        self.0.interact
    }
    #[wasm_bindgen(getter)]
    pub fn drop(&self) -> bool {
        self.0.drop
    }
    /// -1 when no slot was selected this tick.
    #[wasm_bindgen(getter)]
    pub fn tool_slot(&self) -> i32 {
        self.0.tool_slot.map_or(-1, i32::from)
    }
}

/// Key names are `KeyboardEvent.code` values plus the mouse pseudo-keys of
/// `data/input.toml`.
#[wasm_bindgen]
pub struct InputSampler(Sampler);

#[wasm_bindgen]
impl InputSampler {
    /// Built from the default bindings in `data/input.toml`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<InputSampler, JsError> {
        let bindings = Bindings::default_from_data().map_err(|e| JsError::new(&e.to_string()))?;
        Ok(InputSampler(Sampler::new(bindings)))
    }

    pub fn key_down(&mut self, key: &str) {
        self.0.key_down(key);
    }
    pub fn key_up(&mut self, key: &str) {
        self.0.key_up(key);
    }
    pub fn wheel(&mut self, dy: f32) {
        self.0.wheel(dy);
    }
    pub fn mouse_move(&mut self, dx: f32, dy: f32) {
        self.0.mouse_move(dx, dy);
    }
    pub fn clear_held(&mut self) {
        self.0.clear_held();
    }
    /// Whether a `KeyboardEvent.code` is the inventory key, usable while
    /// events are not being forwarded to the sampler.
    pub fn is_inventory_key(&self, code: &str) -> bool {
        self.0.bindings().action_for_key(code) == Some(Action::Inventory)
    }
    #[wasm_bindgen(getter)]
    pub fn yaw(&self) -> f32 {
        self.0.look().0
    }
    #[wasm_bindgen(getter)]
    pub fn pitch(&self) -> f32 {
        self.0.look().1
    }
    #[wasm_bindgen(getter)]
    pub fn fov_degrees(&self) -> f32 {
        self.0.bindings().fov_degrees
    }
    pub fn drain(&mut self) -> Tick {
        Tick(self.0.drain())
    }
}

/// The raft test scene: a flat sea with the `raft_start` structure in it.
#[wasm_bindgen]
pub struct Scene(RaftScene);

#[wasm_bindgen]
impl Scene {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Scene {
        Scene(raft_scene())
    }

    /// World x and z of the scene's first column.
    #[wasm_bindgen(getter)]
    pub fn min(&self) -> i32 {
        SEA_MIN
    }

    /// Columns along x and z; the block arrays are `size * size * HEIGHT`.
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> u32 {
        SEA_SIZE
    }

    /// World x and z of the first column of the hoe-able field.
    #[wasm_bindgen(getter)]
    pub fn field(&self) -> Vec<i32> {
        self.0.field.to_vec()
    }

    /// Side of the field in columns.
    #[wasm_bindgen(getter)]
    pub fn field_size(&self) -> i32 {
        FIELD_SIZE
    }

    /// World y of the field's grass.
    #[wasm_bindgen(getter)]
    pub fn field_y(&self) -> i32 {
        FIELD_Y
    }

    /// World x and z of each planter, flattened; they sit at `field_y`.
    #[wasm_bindgen(getter)]
    pub fn planters(&self) -> Vec<i32> {
        self.0.planters.iter().flatten().copied().collect()
    }

    /// Sea floor and water, in `generate_region_decorated` layout.
    pub fn sea_blocks(&self) -> Vec<u8> {
        self.0.sea.blocks.clone()
    }

    /// The raft's blocks alone, in the same layout, for drawing it apart.
    pub fn raft_blocks(&self) -> Vec<u8> {
        self.0.raft.blocks.clone()
    }
}

impl Default for Scene {
    fn default() -> Scene {
        Scene::new()
    }
}

/// The walking player: stepped from sampled ticks, collides with the terrain
/// regions the scene hands over, and reports the interpolated eye.
/// One event as `name key=value ...`; lists are comma-joined, positions `x,y,z`.
fn describe_event(e: &GameEvent) -> String {
    fn pos<T: std::fmt::Display>(p: &[T; 3]) -> String {
        format!("{},{},{}", p[0], p[1], p[2])
    }
    match e {
        GameEvent::Swing { player, tool } => {
            format!(
                "swing player={player} tool={}",
                tool.as_deref().unwrap_or("")
            )
        }
        GameEvent::Foraged { position, yields } => {
            format!(
                "foraged position={} yields={}",
                pos(position),
                yields.join(",")
            )
        }
        GameEvent::EntityHurt {
            entity,
            attacker,
            damage,
        } => {
            format!("entity_hurt entity={entity} attacker={attacker} damage={damage}")
        }
        GameEvent::EntityKilled {
            entity,
            kind,
            attacker,
        } => {
            format!("entity_killed entity={entity} kind={kind} attacker={attacker}")
        }
        GameEvent::ItemDropped {
            item,
            position,
            source,
        } => {
            format!(
                "item_dropped item={item} position={} source={source}",
                pos(position)
            )
        }
        GameEvent::ItemPickedUp { item, player } => {
            format!("item_picked_up item={item} player={player}")
        }
        GameEvent::BlockTilled { position, from, to } => {
            format!(
                "block_tilled position={} from={from} to={to}",
                pos(position)
            )
        }
        GameEvent::CropPlanted { position, crop } => {
            format!("crop_planted position={} crop={crop}", pos(position))
        }
        GameEvent::CropHarvested {
            position,
            crop,
            yields,
        } => {
            format!(
                "crop_harvested position={} crop={crop} yields={}",
                pos(position),
                yields.join(",")
            )
        }
        GameEvent::EnteredWater { player } => format!("entered_water player={player}"),
        GameEvent::ExitedWater { player } => format!("exited_water player={player}"),
        GameEvent::SteppedOntoRaft { player } => format!("stepped_onto_raft player={player}"),
        GameEvent::SteppedOffRaft { player } => format!("stepped_off_raft player={player}"),
        GameEvent::Landed { fall_distance } => format!("landed fall_distance={fall_distance}"),
    }
}

#[wasm_bindgen]
pub struct Walk(Walker);

#[wasm_bindgen]
impl Walk {
    /// A player with its feet at the given position.
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64, z: f64) -> Result<Walk, JsError> {
        let walker = Walker::new([x, y, z]).map_err(|e| JsError::new(&e.to_string()))?;
        with_default_commands(walker)
    }

    /// Add generated blocks (`generate_region_decorated` output) covering
    /// `w` by `d` columns from world `(x0, z0)`.
    pub fn add_region(&mut self, x0: i32, z0: i32, w: u32, d: u32, blocks: Vec<u8>) {
        self.0.add_region(Region {
            blocks,
            width: w,
            depth: d,
            height: terrain::HEIGHT as u32,
            x0,
            z0,
        });
    }

    /// A player over a streamed world of the given seed: regions around the
    /// player load and unload as they move (`data/streaming.toml`).
    pub fn streamed(seed: u64, x: f64, y: f64, z: f64) -> Result<Walk, JsError> {
        let layout = Layout {
            seed,
            ..Layout::default()
        };
        let walker = Walker::with_layout(layout, [x, y, z]).map_err(|e| JsError::new(&e))?;
        with_default_commands(walker)
    }

    /// How many regions each way from the player's are loaded
    /// (`data/streaming.toml`); the renderer's fog distance follows it.
    pub fn load_radius() -> Result<i32, JsError> {
        let config =
            game::stream::StreamConfig::default_from_data().map_err(|e| JsError::new(&e))?;
        Ok(config.load)
    }

    /// Regions loaded or unloaded since the last call, as `[kind, rx, rz]`
    /// triples: kind 1 loaded (unloads come first), 0 unloaded.
    pub fn drain_stream(&mut self) -> Vec<i32> {
        let change = self.0.drain_stream_changes();
        let triple = |kind: i32| move |c: &RegionCoord| [kind, c.rx, c.rz];
        let unloaded = change.unloaded.iter().map(triple(0));
        let loaded = change.loaded.iter().map(triple(1));
        unloaded.chain(loaded).flatten().collect()
    }

    /// The blocks of the loaded region `(rx, rz)` (`REGION_SIZE` columns a
    /// side from its origin, x then z then y), or empty if it isn't loaded.
    pub fn region_blocks(&self, rx: i32, rz: i32) -> Vec<u8> {
        self.0
            .region_blocks(RegionCoord::new(rx, rz))
            .unwrap_or_default()
    }

    /// Regions an edit changed since the last call, as flat `rx, rz` pairs.
    pub fn drain_edited_regions(&mut self) -> Vec<i32> {
        let regions = self.0.drain_edited_regions();
        regions.iter().flat_map(|c| [c.rx, c.rz]).collect()
    }

    /// A player in the water in front of the scene's raft, which moves under
    /// the scripted swell. The scene's sea is already added.
    pub fn in_scene(scene: &Scene) -> Result<Walk, JsError> {
        let walker = Walker::in_raft_scene(scene.0.clone()).map_err(|e| JsError::new(&e))?;
        with_default_commands(walker)
    }

    /// Queue a console line; it runs at the start of the next `tick`.
    pub fn submit_command(&mut self, line: &str) {
        self.0.submit_command(line);
    }

    /// Run queued commands without ticking (for a paused game).
    pub fn run_commands(&mut self) {
        self.0.run_commands();
    }

    /// Whether `pause` is in effect; the client then stops calling `tick`.
    pub fn paused(&self) -> bool {
        self.0.paused()
    }

    /// Ticks `step` asked for since the last call.
    pub fn take_steps(&mut self) -> u32 {
        self.0.take_steps()
    }

    /// Ticks run so far.
    pub fn sim_tick(&self) -> f64 {
        self.0.sim_tick() as f64
    }

    /// Output lines of commands run since the last call, oldest first.
    pub fn drain_command_output(&mut self) -> Vec<String> {
        self.0.drain_command_output()
    }

    /// Events emitted since the last call, oldest first, one string each:
    /// `name key=value ...` (docs/design/basic-gameplay.md section 11).
    pub fn drain_events(&mut self) -> Vec<String> {
        self.0.drain_events().iter().map(describe_event).collect()
    }

    /// The day phase name, `gathering` or `service`.
    pub fn phase(&self) -> String {
        self.0.phase().0.name().to_string()
    }

    /// Whole seconds left in the day phase.
    pub fn phase_seconds_left(&self) -> u32 {
        self.0.phase().1
    }

    /// The player's health in half-hearts.
    pub fn health(&self) -> u32 {
        self.0.health()
    }

    /// The player's maximum health in half-hearts.
    pub fn max_health(&self) -> u32 {
        self.0.max_health()
    }

    /// Whether a cheat command has run this session.
    pub fn cheated(&self) -> bool {
        self.0.cheated()
    }

    /// How far the raft is from its rest pose `[x, y, z]`, `alpha` of the
    /// way between the last two ticks.
    pub fn raft_offset(&self, alpha: f64) -> Vec<f64> {
        self.0.raft_offset(alpha).to_vec()
    }

    pub fn has_world(&self) -> bool {
        self.0.has_world()
    }

    pub fn tick(&mut self, input: &Tick) {
        self.0.tick(&input.0);
    }

    /// Eye position `[x, y, z]` between the last two ticks, `alpha` in 0..1.
    pub fn eye(&self, alpha: f64) -> Vec<f64> {
        self.0.eye(alpha).to_vec()
    }

    /// What the eye ray hit on the latest tick, as `[kind, x, y, z, face]`:
    /// kind 0 none, 1 block (position and face 0..5 in the order bottom,
    /// top, north, south, west, east), 2 entity (`x` is the id).
    pub fn target(&self) -> Vec<i32> {
        use game::targeting::{Face, Target};
        match self.0.target() {
            Target::None => vec![0, 0, 0, 0, 0],
            Target::Block { pos, face } => {
                let f = match face {
                    Face::Bottom => 0,
                    Face::Top => 1,
                    Face::North => 2,
                    Face::South => 3,
                    Face::West => 4,
                    Face::East => 5,
                };
                vec![1, pos[0], pos[1], pos[2], f]
            }
            Target::Entity { id } => vec![2, id as i32, 0, 0, 0],
        }
    }

    /// The tool in belt slot `slot` (0-based), or empty for an empty slot.
    pub fn belt_slot(&self, slot: u32) -> String {
        usize::try_from(slot)
            .ok()
            .and_then(|i| self.0.player_state().tool_belt.get(i))
            .and_then(Option::as_ref)
            .map_or_else(String::new, |t| t.tool.clone())
    }

    /// Number of belt slots.
    pub fn belt_len(&self) -> u32 {
        game::items::BELT_SLOTS as u32
    }

    /// The selected belt slot (0-based).
    pub fn selected_slot(&self) -> u32 {
        u32::from(self.0.player_state().selected)
    }

    /// The carried item's name, or empty when the hands are empty.
    pub fn hands(&self) -> String {
        self.0.hands().map_or_else(String::new, |i| i.item.clone())
    }

    /// What interaction rules see, as `set_held` takes it: `empty`,
    /// `tool:<name>` or `item:<name>`.
    pub fn held(&self) -> String {
        match self.0.held() {
            HeldThing::Tool(n) => format!("tool:{n}"),
            HeldThing::Item(n) => format!("item:{n}"),
            _ => "empty".into(),
        }
    }

    /// The block id at a world position, farm overlay included.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> u32 {
        self.0.block_at(x, y, z) as u32
    }

    /// Put the block with id `id` at a world position (tests); false when
    /// the position is outside the world's height.
    pub fn place_block(&mut self, x: i32, y: i32, z: i32, id: u8) -> bool {
        self.0.set_block(x, y, z, Block::from_u8(id))
    }

    /// The farm overlay as `x, y, z, block id` quadruples, sorted by position.
    pub fn farm_tiles(&self) -> Vec<i32> {
        self.0
            .farm_tiles()
            .iter()
            .flat_map(|(p, b)| [p[0], p[1], p[2], *b as i32])
            .collect()
    }

    /// Put a loose item called `name` at `(x, y, z)`; returns its entity id.
    pub fn spawn_item(&mut self, name: &str, x: f64, y: f64, z: f64) -> i32 {
        self.0
            .spawn_item(ItemInstance { item: name.into() }, [x, y, z]) as i32
    }

    /// Put an animal of `species` (`cow`, `pig`, `chicken`) at `(x, y, z)`
    /// facing `facing` degrees; returns its entity id, or -1 for an unknown
    /// species.
    pub fn spawn_animal(&mut self, species: &str, x: f64, y: f64, z: f64, facing: f64) -> i32 {
        self.0
            .spawn_animal(species, [x, y, z], facing)
            .map_or(-1, |id| id as i32)
    }

    /// The animals, nine numbers each: `id, x, y, z, facing, w, h, hurt,
    /// dying`. `(x, y, z)` is the centre of the bottom face, `facing` is in
    /// degrees (0 looks along -z), `w` and `h` are the box width and height,
    /// and `hurt` and `dying` are 0 or 1. Species names are in
    /// [`Walk::animal_species`], in the same order.
    pub fn animals(&self) -> Vec<f64> {
        self.0
            .animals()
            .iter()
            .flat_map(|e| {
                [
                    e.id as f64,
                    e.pos[0],
                    e.pos[1],
                    e.pos[2],
                    e.facing,
                    e.box_size[0],
                    e.box_size[1],
                    f64::from(e.hurt_ticks_left > 0),
                    f64::from(e.death_ticks_left.is_some()),
                ]
            })
            .collect()
    }

    /// The species of each animal in [`Walk::animals`], in the same order.
    pub fn animal_species(&self) -> Vec<String> {
        self.0.animals().iter().map(|e| e.species.clone()).collect()
    }

    /// How many loose items lie in the world.
    pub fn loose_items(&self) -> u32 {
        self.0.items().len() as u32
    }

    /// Override the held thing, whatever the belt and hands say (tests):
    /// `empty`, `tool:<name>` or `item:<name>`.
    pub fn set_held(&mut self, held: &str) {
        self.0.set_held(match held.split_once(':') {
            Some(("tool", n)) => HeldThing::Tool(n.into()),
            Some(("item", n)) => HeldThing::Item(n.into()),
            _ => HeldThing::Empty,
        });
    }

    /// Index of the rule a press of the primary (or secondary) button would
    /// match against the current target and held thing, or -1.
    pub fn preview_rule(&self, secondary: bool) -> i32 {
        let button = if secondary {
            Button::Secondary
        } else {
            Button::Primary
        };
        self.0.preview(button).map_or(-1, |r| r.rule_index as i32)
    }

    /// Rule index of the latest press that matched a rule, or -1.
    pub fn last_rule(&self) -> i32 {
        self.0.last_rule().map_or(-1, |i| i as i32)
    }

    /// The `action` of rule `index` (`attack`, `till`, ...), or empty.
    pub fn rule_action(&self, index: i32) -> String {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.0.rule_action(i))
            .map_or_else(String::new, |a| format!("{a:?}").to_lowercase())
    }

    /// The box to outline for the current target, `[minx, miny, minz, maxx,
    /// maxy, maxz]` in world coordinates, or empty with no target. For a
    /// block on the raft, the box is moved to where the raft is drawn at
    /// `alpha` between ticks, so it sticks to the interpolated mesh.
    pub fn target_box(&self, alpha: f64) -> Vec<f64> {
        let Some((b, on_raft)) = self.0.target_box() else {
            return Vec::new();
        };
        let (now, drawn) = (self.0.raft_offset(1.0), self.0.raft_offset(alpha));
        let shift = |i: usize| if on_raft { drawn[i] - now[i] } else { 0.0 };
        (0..3)
            .map(|i| b.min[i] + shift(i))
            .chain((0..3).map(|i| b.max[i] + shift(i)))
            .collect()
    }

    /// Whether the block cell containing `(x, y, z)` is a liquid, e.g. the
    /// drawn eye being under water.
    pub fn liquid_at(&self, x: f64, y: f64, z: f64) -> bool {
        self.0.liquid_at([x, y, z])
    }

    /// Feet position `[x, y, z]` after the latest tick.
    pub fn feet(&self) -> Vec<f64> {
        self.0.player().pos.to_vec()
    }

    pub fn teleport(&mut self, x: f64, y: f64, z: f64) {
        self.0.teleport([x, y, z]);
    }
}

fn with_default_commands(mut walker: Walker) -> Result<Walk, JsError> {
    let registry = default_registry().map_err(|e| JsError::new(&e.to_string()))?;
    let aliases = Aliases::default_aliases(&registry).map_err(|e| JsError::new(&e.to_string()))?;
    walker.set_commands(registry);
    walker.set_aliases(aliases);
    Ok(Walk(walker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_include_aliases() {
        let names = command_names();
        assert!(names.contains(&"help".to_string()));
        assert!(names.contains(&"kitchen".to_string()));
    }

    #[test]
    fn client_stream_region_size_matches_layout() {
        let js = include_str!("../../../client/stream.js");
        let line = format!("export const REGION_SIZE = {};", layout::REGION_SIZE);
        assert!(js.contains(&line), "client/stream.js needs `{line}`");
    }

    fn walk() -> Walk {
        with_default_commands(Walker::new([0.5, 5.0, 0.5]).unwrap()).unwrap_or_else(|_| panic!())
    }

    #[test]
    fn help_answers_after_a_tick_only() {
        let mut w = walk();
        w.submit_command("help");
        assert!(
            w.drain_command_output().is_empty(),
            "rule 1: waits for tick"
        );
        w.0.tick(&TickInput::default());
        let out = w.drain_command_output();
        assert!(out.iter().any(|l| l.starts_with("give ")));
        assert!(out.iter().any(|l| l.starts_with("spawn ")));
        assert!(out.iter().any(|l| l.starts_with("clear ")));
        assert!(!w.cheated());
        w.submit_command("spawn seeds 2");
        w.0.tick(&TickInput::default());
        assert_eq!(w.drain_command_output(), vec!["spawned 2 seeds"]);
        assert!(w.cheated());
    }
}

/// The sim's fixed-step clock (`game::clock`), shared by walking and flying.
#[wasm_bindgen]
pub struct SimClock {
    clock: Clock,
    alpha: f32,
}

#[wasm_bindgen]
impl SimClock {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SimClock {
        SimClock {
            clock: Clock::new(),
            alpha: 0.0,
        }
    }

    /// Advance by one frame of `dt_seconds`; returns how many sim ticks to
    /// run before rendering at [`SimClock::alpha`].
    pub fn advance(&mut self, dt_seconds: f64) -> u32 {
        let step = self.clock.advance(dt_seconds);
        self.alpha = step.alpha;
        step.ticks
    }

    /// Fraction of the way from the previous tick to the current one.
    #[wasm_bindgen(getter)]
    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

impl Default for SimClock {
    fn default() -> SimClock {
        SimClock::new()
    }
}

/// The debug fly camera (`game::camera`), so the client runs the same sim
/// code the tests check.
#[wasm_bindgen]
pub struct FlyCamera(Camera);

#[wasm_bindgen]
impl FlyCamera {
    #[wasm_bindgen(constructor)]
    pub fn new(x: f32, y: f32, z: f32) -> FlyCamera {
        FlyCamera(Camera::new(at(x, y, z), 0.0))
    }

    pub fn tick(&mut self, input: &Tick) {
        self.0.fly_tick(&input.0);
    }

    /// Eye position `[x, y, z]` between the last two ticks, `alpha` in 0..1.
    pub fn eye(&self, alpha: f32) -> Vec<f32> {
        self.0.view(alpha).pos.to_vec()
    }

    /// Move to `(x, y, z)` with no interpolation from the old position.
    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        self.0 = Camera::new(at(x, y, z), 0.0);
    }
}

fn at(x: f32, y: f32, z: f32) -> Pose {
    Pose {
        pos: [x, y, z],
        ..Pose::default()
    }
}

/// Column-major view matrix for an eye at `(x, y, z)` with this frame's look,
/// in the sim's convention (`game::camera`), for walking and flying alike.
#[wasm_bindgen]
pub fn view_matrix(x: f32, y: f32, z: f32, yaw: f32, pitch: f32) -> Vec<f32> {
    Pose {
        pos: [x, y, z],
        yaw,
        pitch: pitch.clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2),
    }
    .view_matrix()
    .to_vec()
}

/// Command names for Tab completion: every command of the debug console's
/// registry, `help` included (docs/design/debug-console.md section 8).
#[wasm_bindgen]
pub fn command_names() -> Vec<String> {
    let registry = default_registry().expect("built-in commands are valid");
    let aliases = Aliases::default_aliases(&registry).expect("built-in aliases are valid");
    registry
        .iter()
        .map(|c| c.name.to_string())
        .chain(aliases.aliases.into_iter().map(|a| a.name))
        .collect()
}

/// Tool names from `data/items.toml`, in file order.
#[wasm_bindgen]
pub fn tool_names() -> Vec<String> {
    Items::default_items()
        .tools
        .into_iter()
        .map(|d| d.name)
        .collect()
}

/// Item names from `data/items.toml`, in file order.
#[wasm_bindgen]
pub fn item_names() -> Vec<String> {
    Items::default_items()
        .items
        .into_iter()
        .map(|d| d.name)
        .collect()
}

/// Animal species names from `data/animals.toml`, sorted.
#[wasm_bindgen]
pub fn species_names() -> Vec<String> {
    Animals::default_animals()
        .kinds
        .into_iter()
        .map(|a| a.kind)
        .collect()
}
