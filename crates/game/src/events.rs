//! Events the sim emits for the game layer.
//! See docs/design/basic-gameplay.md section 11.
//!
//! Only the events built so far are listed; the rest arrive with their
//! mechanics.

/// Something that happened in a tick.
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    /// A forage succeeded at the block `position` and dropped `yields`.
    Foraged {
        position: [i32; 3],
        yields: Vec<String>,
    },
    /// An entity lost `damage` health. `attacker` is `"player"` (there is
    /// one player).
    EntityHurt {
        entity: u32,
        attacker: String,
        damage: u32,
    },
    /// The hit that took an entity's health to 0; follows its `EntityHurt`.
    /// `kind` is the species name.
    EntityKilled {
        entity: u32,
        kind: String,
        attacker: String,
    },
    /// A loose item was created by an action. `source` is `"player"`
    /// (Drop), `"animal"`, `"forage"`, `"till"` or `"harvest"`.
    ItemDropped {
        item: String,
        position: [f64; 3],
        source: String,
    },
    /// The player moved a loose item into their hands. `player` is
    /// `"player"` (there is one player).
    ItemPickedUp { item: String, player: String },
    /// The hoe tilled the block at `position`. `from` and `to` are block
    /// names before and after.
    BlockTilled {
        position: [i32; 3],
        from: String,
        to: String,
    },
    /// A seed went in at the farmland `position`; `crop` is the crop kind.
    CropPlanted { position: [i32; 3], crop: String },
    /// A mature crop at `position` was harvested and dropped `yields`.
    CropHarvested {
        position: [i32; 3],
        crop: String,
        yields: Vec<String>,
    },
    /// The player's body went into water. `player` is `"player"`.
    EnteredWater { player: String },
    /// The player's body left the water.
    ExitedWater { player: String },
    /// The player came to stand on the raft deck.
    SteppedOntoRaft { player: String },
    /// The player stopped standing on the raft deck.
    SteppedOffRaft { player: String },
    /// The player landed on the ground after `fall_distance` blocks of
    /// falling (the distance fall damage used). Water landings emit nothing.
    Landed { fall_distance: f64 },
}
