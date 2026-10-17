//! Events the sim emits for the game layer.
//! See docs/design/basic-gameplay.md section 11.
//!
//! Only the events built so far are listed; the rest arrive with their
//! mechanics.

/// Something that happened in a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    /// A forage succeeded at the block `position` and dropped `yields`.
    Foraged {
        position: [i32; 3],
        yields: Vec<String>,
    },
}
