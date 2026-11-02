//! Items on raft stations keep their raft-local cell through swell and raft
//! travel, and are part of the world hash: docs/design/cooking-stations.md
//! Phase B and the determinism invariant in CLAUDE.md.

use game::input::TickInput;
use game::items::ItemInstance;
use game::scene::{raft_scene, RaftScene};
use game::walk::Walker;
use terrain::Block;

const TICKS: usize = 1000;

fn idle() -> TickInput {
    TickInput::default()
}

fn beef() -> ItemInstance {
    ItemInstance {
        item: "raw_beef".into(),
    }
}

fn drive(forward: f32, strafe: f32) -> TickInput {
    TickInput {
        move_forward: forward,
        move_strafe: strafe,
        ..idle()
    }
}

fn aim(w: &Walker, at: [f64; 3]) -> TickInput {
    let eye = w.eye(1.0);
    let d = [0, 1, 2].map(|i| at[i] - eye[i]);
    TickInput {
        yaw: d[0].atan2(-d[2]) as f32,
        pitch: d[1].atan2(d[0].hypot(d[2])) as f32,
        ..idle()
    }
}

/// The rest cell of the first counter on the raft.
fn counter(scene: &RaftScene) -> [i32; 3] {
    scene
        .cells
        .iter()
        .copied()
        .find(|&[x, y, z]| {
            let i = scene.raft.index(
                (x - scene.raft.x0) as u32,
                y as u32,
                (z - scene.raft.z0) as u32,
            );
            scene.raft.blocks[i] == Block::Counter as u8
        })
        .expect("a counter on the raft")
}

/// Interact aimed at the top of the raft cell `cell`, at the raft's pose now.
fn press(w: &mut Walker, cell: [i32; 3]) {
    let o = w.raft_offset(1.0);
    let at = [0, 1, 2].map(|i| f64::from(cell[i]) + o[i] + [0.5, 0.99, 0.5][i]);
    let a = aim(w, at);
    w.tick(&a);
    w.tick(&TickInput {
        interact: true,
        ..a
    });
}

/// Stand on the deck beside `cell` at the raft's pose now, carrying beef if
/// `carrying`.
fn stand_beside(w: &mut Walker, scene: &RaftScene, cell: [i32; 3], carrying: bool) {
    let dz = if scene.cells.contains(&[cell[0], cell[1] - 1, cell[2] - 1]) {
        -1
    } else {
        1
    };
    let o = w.raft_offset(1.0);
    let feet = [
        f64::from(cell[0]) + 0.5 + o[0],
        f64::from(cell[1]) + o[1],
        f64::from(cell[2] + dz) + 0.5 + o[2],
    ];
    w.teleport(feet);
    for _ in 0..20 {
        w.tick(&idle());
    }
    if carrying {
        let feet = w.player().pos;
        w.spawn_item(beef(), feet);
        for _ in 0..40 {
            w.tick(&idle());
        }
        let at = w.items().iter().next().expect("the item").pos;
        let a = aim(w, [at[0], at[1] + 0.1, at[2]]);
        w.tick(&a);
        w.tick(&TickInput {
            secondary: true,
            ..a
        });
        assert_eq!(w.hands(), Some(&beef()));
    }
}

/// A raft-scene walker carrying beef beside the first counter, and then
/// either putting it on the counter or pressing at empty air instead.
fn session(put_down: bool) -> (Walker, RaftScene, [i32; 3]) {
    let scene = raft_scene();
    let cell = counter(&scene);
    let mut w = Walker::in_raft_scene(scene.clone()).unwrap();
    stand_beside(&mut w, &scene, cell, true);
    if put_down {
        press(&mut w, cell);
        assert_eq!(w.hands(), None);
    } else {
        let a = aim(&w, [0.0, 200.0, 0.0]);
        w.tick(&a);
        w.tick(&TickInput {
            interact: true,
            ..a
        });
        assert_eq!(w.hands(), Some(&beef()));
    }
    (w, scene, cell)
}

fn with_item() -> (Walker, RaftScene, [i32; 3]) {
    session(true)
}

/// Board the helm, drive and turn for `TICKS` ticks, and step off.
fn travel(w: &mut Walker, scene: &RaftScene) {
    let [hx, hy, hz] = scene.helm;
    w.teleport([
        f64::from(hx) + 0.5,
        f64::from(hy) + 1.0,
        f64::from(hz) + 0.5,
    ]);
    for _ in 0..10 {
        w.tick(&idle());
    }
    let board = TickInput {
        interact: true,
        ..idle()
    };
    w.tick(&board);
    assert!(w.driving());
    let start = w.raft_offset(1.0);
    for t in 0..TICKS {
        // Forward throughout, with turns to one side and back again.
        let strafe = match t / 100 {
            1 | 5 => 1.0,
            3 | 7 => -1.0,
            _ => 0.0,
        };
        w.tick(&drive(1.0, strafe));
    }
    assert_ne!(w.raft_offset(1.0), start);
    w.tick(&board);
    assert!(!w.driving());
}

#[test]
fn an_item_keeps_its_raft_cell_through_swell_and_travel() {
    let (mut w, scene, cell) = with_item();
    let expected = vec![(cell, "raw_beef".to_string())];
    assert_eq!(w.station_items(), expected);
    travel(&mut w, &scene);
    assert_eq!(w.station_items(), expected);
    stand_beside(&mut w, &scene, cell, false);
    press(&mut w, cell);
    assert_eq!(w.hands(), Some(&beef()));
    assert!(w.station_items().is_empty());
}

#[test]
fn the_world_hash_covers_station_contents() {
    let (a, ..) = session(true);
    let (b, ..) = session(true);
    assert_eq!(a.station_items(), b.station_items());
    assert_eq!(a.world_hash(), b.world_hash());

    // The same inputs with the item left in the hands instead.
    let (c, ..) = session(false);
    assert!(c.station_items().is_empty());
    assert_eq!(a.sim_tick(), c.sim_tick());
    assert_ne!(a.world_hash(), c.world_hash());
}
