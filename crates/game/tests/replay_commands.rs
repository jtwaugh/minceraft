//! Replay with commands: docs/design/debug-console.md sections 7 and 9.
//!
//! A session using `give`, `spawn`, `tp`, `pause`, `step` and an alias is
//! recorded live, then replayed twice on fresh walkers. All three runs must
//! agree on the world hash, and changing one command or tick must change it.

use game::aliases::Aliases;
use game::commands::default_registry;
use game::input::TickInput;
use game::replay::{replay, Recorder, Recording};
use game::walk::Walker;
use terrain::{Block, Region, HEIGHT};

const TOP: u32 = 3;
const SIZE: u32 = 32;

fn floor() -> Region {
    let mut r = Region {
        blocks: Vec::new(),
        width: SIZE,
        depth: SIZE,
        height: HEIGHT as u32,
        x0: -16,
        z0: -16,
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

fn fresh() -> Walker {
    let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
    let registry = default_registry().unwrap();
    w.set_aliases(Aliases::default_aliases(&registry).unwrap());
    w.set_commands(registry);
    w.add_region(floor());
    w
}

/// Ticks of input and the command lines submitted before each.
fn script() -> Vec<(TickInput, Vec<&'static str>)> {
    let walk = TickInput {
        move_forward: 1.0,
        yaw: 0.3,
        ..TickInput::default()
    };
    let idle = TickInput::default();
    vec![
        (idle, vec![]),
        (walk, vec!["give tool hoe"]),
        (walk, vec!["spawn seeds 3", "spawn chicken 2"]),
        (idle, vec!["tp 5 -4"]),
        (idle, vec!["pause"]),
        (idle, vec!["step 2"]),
        (idle, vec![]),
        (idle, vec!["resume", "kitchen"]),
        (walk, vec![]),
        (walk, vec![]),
        (idle, vec!["time set 0.75", "time freeze"]),
        (walk, vec![]),
        (idle, vec!["time resume"]),
        (idle, vec![]),
    ]
}

fn run_live(script: &[(TickInput, Vec<&'static str>)]) -> (Recording, u64) {
    let mut w = fresh();
    let mut rec = Recorder::new();
    for (input, cmds) in script {
        let cmds: Vec<String> = cmds.iter().map(|c| c.to_string()).collect();
        rec.step(&mut w, input, &cmds);
    }
    (rec.finish(&w), w.world_hash())
}

fn replayed(rec: &Recording) -> Walker {
    let mut w = fresh();
    replay(rec, &mut w).unwrap();
    w
}

#[test]
fn replays_match_the_live_run() {
    let (rec, live) = run_live(&script());
    assert!(rec.cheated);
    let (a, b) = (replayed(&rec), replayed(&rec));
    assert_eq!(a.world_hash(), live);
    assert_eq!(b.world_hash(), live);
    assert!(a.cheated() && b.cheated());
    assert!(!a.animals().is_empty(), "spawn chicken had no effect");
    assert!(!a.items().is_empty(), "spawn seeds had no effect");
}

#[test]
fn text_form_replays_the_same() {
    let (rec, live) = run_live(&script());
    let parsed = Recording::parse(&rec.to_text()).unwrap();
    assert_eq!(replayed(&parsed).world_hash(), live);
}

#[test]
fn session_uses_every_required_command() {
    let (rec, _) = run_live(&script());
    let text = rec.to_text();
    for c in ["give ", "spawn ", "tp ", "pause", "step "] {
        assert!(text.contains(c), "recording lacks {c:?}");
    }
}

#[test]
fn changing_a_command_or_tick_changes_the_hash() {
    let (_, live) = run_live(&script());

    let mut other_cmd = script();
    other_cmd[2].1 = vec!["spawn seeds 4", "spawn chicken 2"];
    assert_ne!(run_live(&other_cmd).1, live);

    let mut other_tick = script();
    other_tick.push((TickInput::default(), vec![]));
    assert_ne!(run_live(&other_tick).1, live);

    let mut other_input = script();
    other_input[8].0.yaw = -1.0;
    assert_ne!(run_live(&other_input).1, live);
}

#[test]
fn time_commands_replay_to_the_same_time_of_day() {
    let mut live = fresh();
    let mut rec = Recorder::new();
    let idle = TickInput::default();
    let lines = |l: &[&str]| l.iter().map(|c| c.to_string()).collect::<Vec<_>>();
    rec.step(&mut live, &idle, &lines(&["time set 0.75", "time freeze"]));
    rec.step(&mut live, &idle, &[]);
    let held = live.time_of_day();
    rec.step(&mut live, &idle, &lines(&["time resume"]));
    rec.step(&mut live, &idle, &[]);
    assert_ne!(live.time_of_day(), held);
    let rec = rec.finish(&live);
    let text = Recording::parse(&rec.to_text()).unwrap();
    for r in [&rec, &text] {
        let w = replayed(r);
        assert_eq!(w.time_of_day(), live.time_of_day());
        assert_eq!(w.world_hash(), live.world_hash());
    }
}
