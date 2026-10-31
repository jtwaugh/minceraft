//! Headless hands and tool belt session: the measurable form of the Phase 5
//! done-when in docs/design/basic-gameplay.md (section 6).
//!
//! A `Walker` on a flat stone floor is driven by scripted `TickInput`s:
//! select the sword, pick up an item, check the tools are stowed, drop the
//! item, pick it up again. Two runs of the script must end identically.

use game::input::TickInput;
use game::interactions::HeldThing;
use game::item_entities::ItemEntity;
use game::items::{ItemInstance, PlayerState};
use game::player::Player;
use game::targeting::Target;
use game::walk::Walker;
use terrain::{Block, Region, HEIGHT};

/// Top stone layer of the floor; the player stands at `y = TOP + 1`.
const TOP: u32 = 3;
const SIZE: u32 = 16;
const SWORD_SLOT: u8 = 1;

fn floor() -> Region {
    let (x0, z0) = (-8, -8);
    let mut r = Region {
        blocks: Vec::new(),
        width: SIZE,
        depth: SIZE,
        height: HEIGHT as u32,
        x0,
        z0,
    };
    let mut blocks = vec![0u8; SIZE as usize * SIZE as usize * HEIGHT];
    for z in 0..SIZE {
        for x in 0..SIZE {
            for y in 0..=TOP {
                blocks[r.index(x, y, z)] = Block::Stone as u8;
            }
        }
    }
    r.blocks = blocks;
    r
}

fn seeds() -> ItemInstance {
    ItemInstance {
        item: "seeds".into(),
    }
}

fn idle() -> TickInput {
    TickInput::default()
}

/// Everything the script can change.
#[derive(Debug, PartialEq)]
struct Final {
    player: Player,
    state: PlayerState,
    items: Vec<ItemEntity>,
    target: Target,
}

fn snapshot(w: &Walker) -> Final {
    Final {
        player: w.player().clone(),
        state: w.player_state().clone(),
        items: w.items().iter().cloned().collect(),
        target: w.target(),
    }
}

/// Aim straight at `at` from the eye.
fn aim_at(w: &Walker, at: [f64; 3]) -> TickInput {
    let eye = w.eye(1.0);
    let d = [0, 1, 2].map(|i| at[i] - eye[i]);
    let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
    TickInput {
        yaw: d[0].atan2(-d[2]) as f32,
        pitch: d[1].atan2(flat) as f32,
        ..idle()
    }
}

fn run_session() -> Final {
    let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
    w.add_region(floor());
    for _ in 0..40 {
        w.tick(&idle());
    }
    assert!(w.player().on_ground);

    // Select the sword.
    w.tick(&TickInput {
        tool_slot: Some(SWORD_SLOT),
        ..idle()
    });
    assert_eq!(w.held(), HeldThing::Tool("sword".into()));

    // Right-click picks the item up; the tools are stowed.
    let id = w.spawn_item(seeds(), [0.5, 5.62, -1.0]);
    w.tick(&idle());
    assert_eq!(w.target(), Target::Entity { id });
    w.tick(&TickInput {
        secondary: true,
        ..idle()
    });
    assert_eq!(w.hands(), Some(&seeds()));
    assert!(w.items().is_empty());
    assert_eq!(w.held(), HeldThing::Item("seeds".into()));

    // Primary with full hands does nothing.
    let before = snapshot(&w);
    let rule = w.last_rule();
    w.tick(&TickInput {
        primary: true,
        ..idle()
    });
    let after = snapshot(&w);
    assert_eq!(after.state, before.state);
    assert_eq!(after.items, before.items);
    assert_eq!(w.last_rule(), rule);
    assert_eq!(w.hands(), Some(&seeds()));

    // Drop throws it; let it land and come to rest.
    w.tick(&TickInput {
        drop: true,
        pitch: -0.3,
        ..idle()
    });
    assert_eq!(w.hands(), None);
    assert_eq!(w.held(), HeldThing::Tool("sword".into()));
    assert_eq!(w.items().len(), 1);
    for _ in 0..80 {
        w.tick(&idle());
    }
    let rest = w.items().iter().next().unwrap().clone();
    assert!(rest.velocity.iter().all(|v| *v == 0.0), "came to rest");

    // Pick it up again.
    let aim = aim_at(&w, rest.pos);
    w.tick(&TickInput {
        secondary: true,
        ..aim
    });
    assert_eq!(w.hands(), Some(&seeds()));
    assert!(w.items().is_empty());
    snapshot(&w)
}

#[test]
fn select_pickup_stow_drop_and_repickup() {
    let end = run_session();
    assert_eq!(end.state.selected, SWORD_SLOT);
    assert_eq!(end.state.hands, Some(seeds()));
    assert!(end.items.is_empty());
}

#[test]
fn identical_scripts_end_in_identical_state() {
    assert_eq!(run_session(), run_session());
}
