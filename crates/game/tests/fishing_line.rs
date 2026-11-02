//! Casting and reeling an empty line through `Walker::tick`: Phase D of
//! docs/design/fish-and-fishing.md (sections 4.2 and 4.6).

use game::events::GameEvent;
use game::input::TickInput;
use game::items::ItemInstance;
use game::walk::Walker;

fn walker() -> Walker {
    let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
    w.player_state_mut().selected = 3;
    w
}

fn press() -> TickInput {
    TickInput {
        secondary: true,
        ..TickInput::default()
    }
}

fn line_events(w: &mut Walker) -> Vec<GameEvent> {
    w.drain_events()
        .into_iter()
        .filter(|e| matches!(e, GameEvent::LineCast { .. } | GameEvent::LineReeled { .. }))
        .collect()
}

fn reeled() -> GameEvent {
    GameEvent::LineReeled {
        player: "player".into(),
        caught: false,
    }
}

#[test]
fn a_press_casts_and_a_second_press_reels_in() {
    let mut w = walker();
    w.tick(&press());
    let ev = line_events(&mut w);
    assert!(matches!(ev[..], [GameEvent::LineCast { .. }]), "{ev:?}");
    w.tick(&TickInput::default());
    w.tick(&press());
    assert_eq!(line_events(&mut w), vec![reeled()]);
    // And the next press casts again: only one bobber at a time.
    w.tick(&TickInput::default());
    w.tick(&press());
    assert!(matches!(
        line_events(&mut w)[..],
        [GameEvent::LineCast { .. }]
    ));
}

#[test]
fn switching_tools_reels_in() {
    let mut w = walker();
    w.tick(&press());
    line_events(&mut w);
    w.tick(&TickInput {
        tool_slot: Some(1),
        ..TickInput::default()
    });
    assert_eq!(line_events(&mut w), vec![reeled()]);
}

#[test]
fn walking_beyond_max_distance_reels_in() {
    let mut w = walker();
    w.tick(&press());
    line_events(&mut w);
    w.teleport([200.0, 5.0, 0.5]);
    w.tick(&TickInput::default());
    assert_eq!(line_events(&mut w), vec![reeled()]);
}

#[test]
fn full_hands_cast_nothing() {
    let mut w = walker();
    w.player_state_mut().hands = Some(ItemInstance {
        item: "seeds".into(),
    });
    w.tick(&press());
    assert!(line_events(&mut w).is_empty());
    w.tick(&press());
    assert!(line_events(&mut w).is_empty());
}

#[test]
fn replays_agree() {
    let run = || {
        let mut w = walker();
        for i in 0..40 {
            w.tick(&if i % 10 == 0 {
                press()
            } else {
                TickInput::default()
            });
        }
        w.world_hash()
    };
    assert_eq!(run(), run());
}
