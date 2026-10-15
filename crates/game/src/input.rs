//! Per-tick input record and the accumulator that feeds it.
//! See docs/design/basic-gameplay.md section 2.2.

use std::f32::consts::FRAC_PI_2;

/// One sim tick's worth of input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickInput {
    /// -1..1
    pub move_forward: f32,
    /// -1..1
    pub move_strafe: f32,
    /// Absolute, radians.
    pub yaw: f32,
    /// Absolute, clamped to +/- 90 degrees.
    pub pitch: f32,
    /// Held.
    pub jump: bool,
    /// Held.
    pub sneak: bool,
    /// Pressed since last tick.
    pub primary: bool,
    /// Pressed since last tick.
    pub secondary: bool,
    pub secondary_held: bool,
    /// Pressed since last tick.
    pub interact: bool,
    /// Pressed since last tick.
    pub drop: bool,
    pub tool_slot: Option<u8>,
}

/// A press edge that survives until the sim has consumed it.
///
/// Presses are counted, and each drain consumes one, so a physical click is
/// never lost and never acts twice. Two clicks inside one tick therefore act
/// on two consecutive ticks.
#[derive(Clone, Copy, Debug, Default)]
struct Latch(u32);

impl Latch {
    fn press(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    fn take(&mut self) -> bool {
        let fired = self.0 > 0;
        self.0 = self.0.saturating_sub(1);
        fired
    }
}

/// Collects device events between ticks; `drain` yields one `TickInput`.
#[derive(Clone, Debug, Default)]
pub struct InputAccumulator {
    move_forward: f32,
    move_strafe: f32,
    yaw: f32,
    pitch: f32,
    jump: bool,
    sneak: bool,
    secondary_held: bool,
    primary: Latch,
    secondary: Latch,
    interact: Latch,
    drop: Latch,
    tool_slot: Option<u8>,
}

fn axis(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

impl InputAccumulator {
    pub fn new() -> InputAccumulator {
        InputAccumulator::default()
    }

    /// Movement axes (held state), each clamped to -1..1.
    pub fn set_move(&mut self, forward: f32, strafe: f32) {
        self.move_forward = axis(forward);
        self.move_strafe = axis(strafe);
    }

    /// Absolute look angles in radians; pitch is clamped to +/- 90 degrees.
    pub fn set_look(&mut self, yaw: f32, pitch: f32) {
        if yaw.is_finite() {
            self.yaw = yaw;
        }
        if pitch.is_finite() {
            self.pitch = pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
        }
    }

    pub fn set_jump(&mut self, held: bool) {
        self.jump = held;
    }

    pub fn set_sneak(&mut self, held: bool) {
        self.sneak = held;
    }

    pub fn press_primary(&mut self) {
        self.primary.press();
    }

    /// Secondary button down: latches the edge and starts the hold.
    pub fn press_secondary(&mut self) {
        self.secondary.press();
        self.secondary_held = true;
    }

    pub fn release_secondary(&mut self) {
        self.secondary_held = false;
    }

    pub fn press_interact(&mut self) {
        self.interact.press();
    }

    pub fn press_drop(&mut self) {
        self.drop.press();
    }

    /// Select a tool slot; the latest selection before the tick wins.
    pub fn select_tool(&mut self, slot: u8) {
        self.tool_slot = Some(slot);
    }

    /// Produce this tick's input. Held values stay; edges are consumed.
    pub fn drain(&mut self) -> TickInput {
        TickInput {
            move_forward: self.move_forward,
            move_strafe: self.move_strafe,
            yaw: self.yaw,
            pitch: self.pitch,
            jump: self.jump,
            sneak: self.sneak,
            primary: self.primary.take(),
            secondary: self.secondary.take(),
            secondary_held: self.secondary_held,
            interact: self.interact.take(),
            drop: self.drop.take(),
            tool_slot: self.tool_slot.take(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_and_release_between_ticks_is_seen_once() {
        let mut a = InputAccumulator::new();
        a.press_primary();
        assert!(a.drain().primary);
        assert!(!a.drain().primary);
    }

    #[test]
    fn two_presses_give_one_edge_per_tick_and_none_are_lost() {
        let mut a = InputAccumulator::new();
        a.press_primary();
        a.press_primary();
        assert!(a.drain().primary);
        assert!(a.drain().primary);
        assert!(!a.drain().primary);
    }

    #[test]
    fn held_values_persist_and_secondary_edge_is_separate() {
        let mut a = InputAccumulator::new();
        a.set_jump(true);
        a.set_sneak(true);
        a.set_move(1.0, -0.5);
        a.press_secondary();
        let t = a.drain();
        assert!(t.jump && t.sneak && t.secondary && t.secondary_held);
        let t = a.drain();
        assert!(t.jump && t.sneak && !t.secondary && t.secondary_held);
        assert_eq!((t.move_forward, t.move_strafe), (1.0, -0.5));
        a.set_jump(false);
        a.release_secondary();
        let t = a.drain();
        assert!(!t.jump && !t.secondary_held && t.sneak);
    }

    #[test]
    fn click_released_before_tick_leaves_no_hold() {
        let mut a = InputAccumulator::new();
        a.press_secondary();
        a.release_secondary();
        let t = a.drain();
        assert!(t.secondary && !t.secondary_held);
    }

    #[test]
    fn pitch_and_axes_are_clamped() {
        let mut a = InputAccumulator::new();
        a.set_look(3.0, 10.0);
        a.set_move(5.0, -5.0);
        let t = a.drain();
        assert_eq!((t.yaw, t.pitch), (3.0, FRAC_PI_2));
        assert_eq!((t.move_forward, t.move_strafe), (1.0, -1.0));
        a.set_look(3.0, -10.0);
        assert_eq!(a.drain().pitch, -FRAC_PI_2);
    }

    #[test]
    fn interact_drop_and_tool_slot_latch() {
        let mut a = InputAccumulator::new();
        a.press_interact();
        a.press_drop();
        a.select_tool(2);
        a.select_tool(4);
        let t = a.drain();
        assert!(t.interact && t.drop);
        assert_eq!(t.tool_slot, Some(4));
        let t = a.drain();
        assert!(!t.interact && !t.drop);
        assert_eq!(t.tool_slot, None);
    }

    fn run() -> Vec<TickInput> {
        let mut a = InputAccumulator::new();
        let mut out = Vec::new();
        for i in 0..20u32 {
            a.set_look(i as f32 * 0.1, (i as f32 - 10.0) * 0.3);
            a.set_move((i % 3) as f32 - 1.0, 0.5);
            a.set_jump(i % 4 == 0);
            if i % 2 == 0 {
                a.press_primary();
                a.press_primary();
            }
            if i % 5 == 0 {
                a.press_secondary();
            } else if i % 5 == 2 {
                a.release_secondary();
            }
            if i % 7 == 0 {
                a.select_tool((i % 9) as u8);
            }
            out.push(a.drain());
        }
        out
    }

    #[test]
    fn same_events_give_same_ticks() {
        assert_eq!(run(), run());
    }
}
