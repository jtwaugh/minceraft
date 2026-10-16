//! wasm bindings for the input sampler, the sim clock, the walking player and
//! the debug fly camera. The DOM side is `client/input.js`; the render loop is
//! `client/scene.js`.

use game::bindings::Bindings;
use game::camera::{Camera, Pose};
use game::clock::Clock;
use game::input::TickInput;
use game::interactions::{Button, HeldThing};
use game::sampler::Sampler;
use game::scene::{raft_scene, RaftScene, SEA_MIN, SEA_SIZE};
use game::walk::Walker;
use terrain::Region;
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
#[wasm_bindgen]
pub struct Walk(Walker);

#[wasm_bindgen]
impl Walk {
    /// A player with its feet at the given position.
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64, z: f64) -> Result<Walk, JsError> {
        Walker::new([x, y, z])
            .map(Walk)
            .map_err(|e| JsError::new(&e.to_string()))
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

    /// A player in the water in front of the scene's raft, which moves under
    /// the scripted swell. The scene's sea is already added.
    pub fn in_scene(scene: &Scene) -> Result<Walk, JsError> {
        Walker::in_raft_scene(scene.0.clone())
            .map(Walk)
            .map_err(|e| JsError::new(&e))
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

    /// Pick the held thing until the belt exists: `empty`, or `tool:<name>`
    /// (for example `tool:hoe`); `item:<name>` holds an item.
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

    /// Feet position `[x, y, z]` after the latest tick.
    pub fn feet(&self) -> Vec<f64> {
        self.0.player().pos.to_vec()
    }

    pub fn teleport(&mut self, x: f64, y: f64, z: f64) {
        self.0.teleport([x, y, z]);
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
