//! Device events to `TickInput`: maps key names through the bindings and
//! feeds an `InputAccumulator`. The browser layer (`client/input.js`) only
//! forwards raw events here.
//! See docs/design/basic-gameplay.md sections 2.1 and 2.2.

use std::collections::HashSet;

use crate::bindings::{Action, Bindings};
use crate::input::{InputAccumulator, TickInput};

/// Prefix of the number-row pseudo-keys bound to `select_tool`.
const DIGIT_PREFIX: &str = "Digit";

pub struct Sampler {
    bindings: Bindings,
    acc: InputAccumulator,
    down: HashSet<String>,
    yaw: f32,
    pitch: f32,
    slot: u8,
    slot_count: u8,
}

impl Sampler {
    pub fn new(bindings: Bindings) -> Sampler {
        let slot_count = bindings
            .get(Action::SelectTool)
            .keys
            .iter()
            .filter(|k| k.starts_with(DIGIT_PREFIX))
            .count()
            .clamp(1, u8::MAX as usize) as u8;
        Sampler {
            bindings,
            acc: InputAccumulator::new(),
            down: HashSet::new(),
            yaw: 0.0,
            pitch: 0.0,
            slot: 0,
            slot_count,
        }
    }

    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    fn held(&self, action: Action) -> bool {
        self.bindings
            .get(action)
            .keys
            .iter()
            .any(|k| self.down.contains(k))
    }

    fn refresh_held(&mut self) {
        let axis = |plus: bool, minus: bool| f32::from(plus as u8) - f32::from(minus as u8);
        self.acc.set_move(
            axis(self.held(Action::MoveForward), self.held(Action::MoveBack)),
            axis(self.held(Action::MoveRight), self.held(Action::MoveLeft)),
        );
        self.acc.set_jump(self.held(Action::Jump));
        self.acc.set_sneak(self.held(Action::Sneak));
    }

    /// A key or mouse button went down. Repeats while already down are
    /// ignored, so one physical press gives one edge.
    pub fn key_down(&mut self, key: &str) {
        let Some(action) = self.bindings.action_for_key(key) else {
            return;
        };
        if !self.down.insert(key.to_owned()) {
            return;
        }
        match action {
            Action::Primary => self.acc.press_primary(),
            Action::Secondary => self.acc.press_secondary(),
            Action::Interact => self.acc.press_interact(),
            Action::Drop => self.acc.press_drop(),
            Action::SelectTool => {
                if let Some(n) = key
                    .strip_prefix(DIGIT_PREFIX)
                    .and_then(|d| d.parse::<u8>().ok())
                {
                    if n >= 1 && n <= self.slot_count {
                        self.select(n - 1);
                    }
                }
            }
            _ => self.refresh_held(),
        }
    }

    pub fn key_up(&mut self, key: &str) {
        if !self.down.remove(key) {
            return;
        }
        match self.bindings.action_for_key(key) {
            Some(Action::Secondary) => {
                if !self.held(Action::Secondary) {
                    self.acc.release_secondary();
                }
            }
            Some(_) => self.refresh_held(),
            None => {}
        }
    }

    fn select(&mut self, slot: u8) {
        self.slot = slot;
        self.acc.select_tool(slot);
    }

    /// Mouse wheel: `dy` > 0 is scrolling down, which moves to the next
    /// slot; scrolling up moves to the previous. Wraps around the belt.
    pub fn wheel(&mut self, dy: f32) {
        let key = if dy > 0.0 {
            "WheelDown"
        } else if dy < 0.0 {
            "WheelUp"
        } else {
            return;
        };
        if self.bindings.action_for_key(key) != Some(Action::SelectTool) {
            return;
        }
        let n = self.slot_count;
        let next = if dy > 0.0 {
            (self.slot + 1) % n
        } else {
            (self.slot + n - 1) % n
        };
        self.select(next);
    }

    /// Pointer-lock motion in pixels. Moving right turns right (yaw
    /// increases); moving down looks down (pitch decreases, up is positive).
    pub fn mouse_move(&mut self, dx: f32, dy: f32) {
        if self.bindings.action_for_key("MouseMove") != Some(Action::Look) {
            return;
        }
        let s = self.bindings.sensitivity;
        self.acc.set_look(self.yaw + dx * s, self.pitch - dy * s);
        // Read back through the accumulator so pitch clamping lives in one place.
        let t = self.acc.clone().drain();
        self.yaw = t.yaw;
        self.pitch = t.pitch;
    }

    /// Release everything held (pointer lock lost, window blur). Pending
    /// press edges and the look angles are kept.
    pub fn clear_held(&mut self) {
        self.down.clear();
        self.refresh_held();
        self.acc.release_secondary();
    }

    /// Current look angles, read every frame by the camera.
    pub fn look(&self) -> (f32, f32) {
        (self.yaw, self.pitch)
    }

    /// The sim's input for this tick (look is read here, at the boundary).
    pub fn drain(&mut self) -> TickInput {
        self.acc.drain()
    }
}

#[cfg(test)]
mod tests;
