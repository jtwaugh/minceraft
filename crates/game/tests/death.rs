//! Headless death and respawn: the measurable form of the Phase A done-when
//! in docs/design/player-death.md.
//!
//! A scripted session walks off a tall plateau, dies, tries to move and act
//! while dead, submits `respawn`, and walks again. It is recorded, and the
//! recording replays on a fresh walker to identical events and world hash.

use game::commands::default_registry;
use game::events::GameEvent;
use game::input::TickInput;
use game::replay::{replay, Recorder, Recording};
use game::walk::Walker;
use terrain::{Block, Region, HEIGHT};

const SIZE: u32 = 32;
const HALF: i32 = 16;
/// Top of the low floor and of the plateau (x < 0) the player starts on.
const LOW: u32 = 3;
const HIGH: u32 = 40;
const START: [f64; 3] = [-3.5, HIGH as f64 + 1.0, 0.5];

/// A low stone floor with a plateau on the `x < 0` half.
fn world() -> Region {
    let mut r = Region {
        blocks: Vec::new(),
        width: SIZE,
        depth: SIZE,
        height: HEIGHT as u32,
        x0: -HALF,
        z0: -HALF,
    };
    let mut blocks = vec![0u8; SIZE as usize * SIZE as usize * HEIGHT];
    for z in 0..SIZE {
        for x in 0..SIZE {
            let top = if (x as i32) < HALF { HIGH } else { LOW };
            for y in 0..=top {
                blocks[r.index(x, y, z)] = Block::Stone as u8;
            }
        }
    }
    r.blocks = blocks;
    r
}

fn fresh() -> Walker {
    let mut w = Walker::new(START).unwrap();
    w.set_commands(default_registry().unwrap());
    w.add_region(world());
    w
}

fn idle() -> TickInput {
    TickInput::default()
}

/// Walk toward +x, off the plateau edge.
fn east() -> TickInput {
    TickInput {
        move_forward: 1.0,
        yaw: std::f32::consts::FRAC_PI_2,
        ..idle()
    }
}

fn everything() -> TickInput {
    TickInput {
        primary: true,
        secondary: true,
        secondary_held: true,
        interact: true,
        drop: true,
        jump: true,
        ..east()
    }
}

struct Run {
    w: Walker,
    rec: Recorder,
    events: Vec<GameEvent>,
}

impl Run {
    fn step(&mut self, input: &TickInput, commands: &[&str]) {
        let commands: Vec<String> = commands.iter().map(|c| c.to_string()).collect();
        self.rec.step(&mut self.w, input, &commands);
        self.events.extend(self.w.drain_events());
    }

    fn steps(&mut self, input: &TickInput, n: usize) {
        for _ in 0..n {
            self.step(input, &[]);
        }
    }
}

struct Outcome {
    rec: Recording,
    events: Vec<GameEvent>,
    hash: u64,
}

fn session() -> Outcome {
    session_with(&[])
}

/// The session, with `setup` commands submitted on the first tick.
fn session_with(setup: &[&str]) -> Outcome {
    let mut run = Run {
        w: fresh(),
        rec: Recorder::new(),
        events: vec![],
    };
    run.step(&idle(), setup);
    run.steps(&idle(), 19);
    assert!(run.w.player().on_ground);
    assert!(!run.w.dead());

    // Walk off the edge and fall to the floor.
    run.steps(&east(), 300);
    assert!(run.w.dead());
    assert_eq!(run.w.health(), 0);

    // Dead: movement and actions change nothing horizontally.
    let at = run.w.player().pos;
    run.steps(&everything(), 40);
    let now = run.w.player().pos;
    assert_eq!((now[0], now[2]), (at[0], at[2]));
    assert!(run.w.dead());

    // Respawn at the start with full health.
    run.step(&idle(), &["respawn"]);
    assert!(!run.w.dead());
    assert_eq!(run.w.health(), run.w.max_health());
    assert_eq!(run.w.player().pos, START);
    run.steps(&idle(), 20);
    assert_eq!(run.w.player().pos, START);

    // Movement works again.
    run.steps(&east(), 10);
    assert!(run.w.player().pos[0] > START[0] + 0.5);

    let hash = run.w.world_hash();
    Outcome {
        rec: run.rec.finish(&run.w),
        events: run.events,
        hash,
    }
}

#[test]
fn a_fall_to_zero_emits_player_died_once_and_respawn_emits_once() {
    let o = session();
    let died: Vec<_> = o
        .events
        .iter()
        .filter(|e| matches!(e, GameEvent::PlayerDied { .. }))
        .collect();
    assert_eq!(
        died,
        vec![&GameEvent::PlayerDied {
            cause: "fall".into()
        }]
    );
    let respawned: Vec<_> = o
        .events
        .iter()
        .filter(|e| matches!(e, GameEvent::PlayerRespawned { .. }))
        .collect();
    assert_eq!(
        respawned,
        vec![&GameEvent::PlayerRespawned { position: START }]
    );
}

#[test]
fn the_recording_replays_to_identical_events_and_world_hash() {
    let o = session();
    assert!(!o.rec.cheated);
    for rec in [o.rec.clone(), Recording::parse(&o.rec.to_text()).unwrap()] {
        let mut w = fresh();
        let mut events = vec![];
        replay(&rec, &mut w).unwrap();
        events.extend(w.drain_events());
        assert_eq!(w.world_hash(), o.hash);
        // Draining once at the end sees the same events in the same order.
        assert_eq!(events, o.events);
    }
    assert_eq!(session().events, o.events);
    assert_eq!(session().hash, o.hash);
}

fn dropped(events: &[GameEvent]) -> Vec<&GameEvent> {
    events
        .iter()
        .filter(|e| matches!(e, GameEvent::ItemDropped { .. }))
        .collect()
}

#[test]
fn a_death_with_a_carried_item_drops_it_and_replays_identically() {
    let o = session_with(&["give item wheat"]);
    let died = o
        .events
        .iter()
        .position(|e| matches!(e, GameEvent::PlayerDied { .. }))
        .unwrap();
    let drops = dropped(&o.events);
    assert_eq!(drops.len(), 1, "{:?}", o.events);
    let GameEvent::ItemDropped { item, source, .. } = drops[0] else {
        unreachable!()
    };
    assert_eq!((item.as_str(), source.as_str()), ("wheat", "death"));
    assert!(matches!(o.events[died + 1], GameEvent::ItemDropped { .. }));

    assert!(o.rec.cheated);
    for rec in [o.rec.clone(), Recording::parse(&o.rec.to_text()).unwrap()] {
        let mut w = fresh();
        replay(&rec, &mut w).unwrap();
        assert_eq!(w.world_hash(), o.hash);
        assert_eq!(w.drain_events(), o.events);
        // Respawning did not give it back; the item is still on the ground.
        assert!(w.hands().is_none());
        assert_eq!(w.items().len(), 1);
    }
}

#[test]
fn a_death_with_empty_hands_drops_nothing() {
    assert!(dropped(&session().events).is_empty());
}
