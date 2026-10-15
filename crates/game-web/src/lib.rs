//! wasm bindings for the input sampler. The DOM side is `client/input.js`.

use game::bindings::Bindings;
use game::input::TickInput;
use game::sampler::Sampler;
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
