//! Replays cover the raft: its pose and the `driving` flag are in the world
//! hash. See docs/design/raft-scene-feedback.md Phase D and
//! docs/design/debug-console.md section 7.

use game::input::TickInput;
use game::replay::{replay, Recorder};
use game::scene::{raft_scene, RaftScene};
use game::walk::Walker;
use terrain::Block;

/// Blocks ahead of the helm (towards -z) where the wall stands.
const WALL_GAP: i32 = 10;

fn fresh(scene: &RaftScene) -> Walker {
    let mut w = Walker::in_raft_scene(scene.clone()).unwrap();
    let [hx, hy, hz] = scene.helm;
    for x in hx - 8..=hx + 8 {
        for y in hy - 1..=hy {
            assert!(w.set_block(x, y, hz - WALL_GAP, Block::Stone));
        }
    }
    w
}

fn input(forward: f32, strafe: f32, yaw: f32) -> TickInput {
    TickInput {
        move_forward: forward,
        move_strafe: strafe,
        yaw,
        ..TickInput::default()
    }
}

fn interact() -> TickInput {
    TickInput {
        interact: true,
        ..TickInput::default()
    }
}

/// Swim to the raft, climb on, walk to the helm, board it, drive into the
/// wall, turn, drive on, and interact to leave. Built by running a probe
/// walker; the result is a fixed list that needs no feedback to replay.
fn stream(scene: &RaftScene) -> Vec<TickInput> {
    let mut w = fresh(scene);
    let mut out = Vec::new();
    let mut run = |w: &mut Walker, i: TickInput| {
        w.tick(&i);
        out.push(i);
    };
    let swim = TickInput {
        jump: true,
        ..input(1.0, 0.0, 0.0)
    };
    for _ in 0..300 {
        run(&mut w, swim);
        if w.player().on_ground {
            break;
        }
    }
    assert!(w.player().on_ground);
    let helm = |w: &Walker| {
        let o = w.raft_offset(1.0);
        [
            f64::from(scene.helm[0]) + 0.5 + o[0],
            f64::from(scene.helm[2]) + 0.5 + o[2],
        ]
    };
    for _ in 0..200 {
        let h = helm(&w);
        let p = w.player().pos;
        let (dx, dz) = (h[0] - p[0], h[1] - p[2]);
        if dx.hypot(dz) < 0.2 {
            break;
        }
        let yaw = dx.atan2(-dz) as f32;
        // The helm is a block: jump up onto it.
        let hop = TickInput {
            jump: true,
            ..input(1.0, 0.0, yaw)
        };
        run(&mut w, hop);
    }
    for _ in 0..10 {
        run(&mut w, TickInput::default());
    }
    run(&mut w, interact());
    assert!(
        w.driving(),
        "not at the helm: {:?} helm {:?} now {:?}",
        w.player(),
        scene.helm,
        helm(&w)
    );
    for _ in 0..200 {
        run(&mut w, input(1.0, 0.0, 0.0));
    }
    for _ in 0..30 {
        run(&mut w, input(0.0, 1.0, 0.0));
    }
    for _ in 0..30 {
        run(&mut w, input(1.0, 0.0, 0.0));
    }
    run(&mut w, interact());
    assert!(!w.driving());
    out
}

#[test]
fn the_same_stream_gives_the_same_hashes_and_replays() {
    let scene = raft_scene();
    let inputs = stream(&scene);
    let (mut a, mut b) = (fresh(&scene), fresh(&scene));
    assert_eq!(a.world_hash(), b.world_hash());
    let mut rec = Recorder::new();
    let mut drove = false;
    let start = a.raft_offset(1.0);
    for i in &inputs {
        rec.step(&mut a, i, &[]);
        b.tick(i);
        assert_eq!(a.world_hash(), b.world_hash(), "tick {}", a.sim_tick());
        drove |= a.driving();
    }
    assert!(drove);
    assert_ne!(a.raft_offset(1.0), start);
    let mut c = fresh(&scene);
    replay(&rec.finish(&a), &mut c).unwrap();
    assert_eq!(c.world_hash(), a.world_hash());
}

#[test]
fn a_different_stream_gives_a_different_hash() {
    let scene = raft_scene();
    let inputs = stream(&scene);
    let (mut a, mut b) = (fresh(&scene), fresh(&scene));
    for i in &inputs {
        a.tick(i);
    }
    // Same stream with the first tick of driving turning instead.
    let at = inputs.iter().position(|i| i.interact).unwrap() + 1;
    let mut other = inputs.clone();
    other[at] = input(0.0, 1.0, 0.0);
    for i in &other {
        b.tick(i);
    }
    assert_ne!(a.world_hash(), b.world_hash());
}
