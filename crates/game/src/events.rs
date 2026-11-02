//! Events the sim emits for the game layer.
//! See docs/design/basic-gameplay.md section 11.
//!
//! Only the events built so far are listed; the rest arrive with their
//! mechanics.

/// Something that happened in a tick.
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    /// A primary press that was not blocked by carrying, whatever it
    /// targets. `player` is `"player"`; `tool` is the held tool's name, or
    /// `None` for the bare hand.
    Swing {
        player: String,
        tool: Option<String>,
    },
    /// A forage succeeded at the block `position` and dropped `yields`.
    Foraged {
        position: [i32; 3],
        yields: Vec<String>,
    },
    /// A forage outside the block's cooldown at `position` dropped nothing.
    ForageMissed { position: [i32; 3] },
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
    /// A fish was taken out of the water. `species` is its `data/fish.toml`
    /// key, `item` the raw item now in the hands, `method` is `"sword"`
    /// (a punch counts as the sword) or `"rod"`.
    FishCaught {
        player: String,
        species: String,
        item: String,
        method: String,
    },
    /// A loose item was created by an action. `source` is `"player"`
    /// (Drop), `"animal"`, `"forage"`, `"till"`, `"harvest"` or `"death"`.
    ItemDropped {
        item: String,
        position: [f64; 3],
        source: String,
    },
    /// The player moved a loose item into their hands. `player` is
    /// `"player"` (there is one player).
    ItemPickedUp { item: String, player: String },
    /// The carried item went onto a station. `target` is the station's block
    /// name.
    ItemPutDown { item: String, target: String },
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
    /// The player's health reached 0, once, after that tick's `Landed`.
    /// `cause` is `"fall"` (the only way to die so far).
    PlayerDied { cause: String },
    /// A dead player came back at `position` (the start spawn) with full
    /// health.
    PlayerRespawned { position: [f64; 3] },
    /// An action the player tried was refused. `action` is `"swing"`,
    /// `"till"`, `"plant"`, `"pick"`, `"forage"` or `"pickup"`.
    ActionRefused { action: String, reason: Reason },
}

/// Why an action was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    TooFar,
    HandsFull,
    ToolsStowed,
    WrongTool,
    NothingHere,
}

impl Reason {
    /// The snake_case name used in event strings.
    pub fn name(self) -> &'static str {
        match self {
            Reason::TooFar => "too_far",
            Reason::HandsFull => "hands_full",
            Reason::ToolsStowed => "tools_stowed",
            Reason::WrongTool => "wrong_tool",
            Reason::NothingHere => "nothing_here",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_names() {
        let all = [
            (Reason::TooFar, "too_far"),
            (Reason::HandsFull, "hands_full"),
            (Reason::ToolsStowed, "tools_stowed"),
            (Reason::WrongTool, "wrong_tool"),
            (Reason::NothingHere, "nothing_here"),
        ];
        for (r, n) in all {
            assert_eq!(r.name(), n);
        }
    }
}
