//! Headless swim and climb-out check: the measurable form of the Phase 3
//! done-when in docs/design/basic-gameplay.md (sections 4.3 and 14).
//!
//! A pool in a `MemoryWorld`, ringed by a deck-height ledge standing in for
//! the raft edge at `y = SEA_LEVEL`. The player swims up from the bottom,
//! climbs out onto the ledge from each side, and drops in from height.

use game::collision::MemoryWorld;
use game::input::TickInput;
use game::physics::PlayerPhysics;
use game::player::Player;
use terrain::Block;

/// Pool floor top is at `y = 1`; water fills `y = 1..SURFACE`, 5 blocks deep.
const SURFACE: i32 = 6;
/// The pool spans `-HALF..HALF` on x and z; the ledge ring lies outside it.
const HALF: i32 = 3;
const RING: i32 = 3;
/// Bound on the ticks any scripted phase may take.
const MAX_TICKS: u32 = 200;

fn physics() -> PlayerPhysics {
    PlayerPhysics::default_from_data().unwrap()
}

/// Stone floor under everything, a 5-deep pool, and a ledge ring around it
/// whose top surface is at the water surface (`y = SURFACE`).
fn pool() -> MemoryWorld {
    let mut w = MemoryWorld::new();
    let r = HALF + RING;
    for x in -r..r {
        for z in -r..r {
            w.set(x, 0, z, Block::Stone);
            let in_pool = (-HALF..HALF).contains(&x) && (-HALF..HALF).contains(&z);
            for y in 1..SURFACE {
                w.set(x, y, z, if in_pool { Block::Water } else { Block::Stone });
            }
        }
    }
    w
}

fn jump() -> TickInput {
    TickInput {
        jump: true,
        ..TickInput::default()
    }
}

#[test]
fn swim_up_from_five_deep_reaches_the_surface() {
    let (phys, w) = (physics(), pool());
    let mut p = Player::new([0.5, 1.0, 0.5], &phys);
    let mut reached = None;
    for t in 0..MAX_TICKS {
        p.tick(&jump(), &phys, &w);
        if p.pos[1] >= SURFACE as f64 - 1.0 {
            reached = Some(t);
            break;
        }
    }
    let t = reached.unwrap_or_else(|| panic!("never surfaced: {p:?}"));
    assert!(t < MAX_TICKS, "{t}");
    assert_eq!(p.health, phys.max_health);
}

/// Swim up in the middle of the pool, then hold jump and push toward the
/// ledge on one side until standing on it.
fn climb_out(dir: [i32; 2]) -> Player {
    let (phys, w) = (physics(), pool());
    let mut p = Player::new([-0.5, 1.0, -0.5], &phys);
    for _ in 0..MAX_TICKS {
        p.tick(&jump(), &phys, &w);
        if p.pos[1] >= SURFACE as f64 - 1.5 {
            break;
        }
    }
    // Forward is -z at yaw 0.
    let yaw = (dir[0] as f64).atan2(-(dir[1] as f64)) as f32;
    let push = TickInput {
        move_forward: 1.0,
        yaw,
        jump: true,
        ..TickInput::default()
    };
    for _ in 0..MAX_TICKS {
        p.tick(&push, &phys, &w);
        // On the ledge once standing at its top, past the pool's edge.
        let beyond = p.pos[0] * dir[0] as f64 + p.pos[2] * dir[1] as f64 > HALF as f64 + 0.3;
        if p.on_ground && beyond {
            break;
        }
    }
    p
}

#[test]
fn climbs_out_onto_the_ledge_from_all_four_sides() {
    for dir in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let p = climb_out(dir);
        assert!(p.on_ground, "{dir:?}: {p:?}");
        assert_eq!(p.pos[1], SURFACE as f64, "{dir:?}: {p:?}");
        let along = p.pos[0] * dir[0] as f64 + p.pos[2] * dir[1] as f64;
        assert!(along > HALF as f64, "{dir:?}: {p:?}");
    }
}

#[test]
fn climb_out_is_repeatable() {
    for dir in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        assert_eq!(climb_out(dir), climb_out(dir), "{dir:?}");
    }
}

#[test]
fn entering_water_from_height_does_no_fall_damage() {
    let (phys, w) = (physics(), pool());
    let mut p = Player::new([-0.5, 40.0, -0.5], &phys);
    let mut entered = false;
    for _ in 0..MAX_TICKS {
        p.tick(&TickInput::default(), &phys, &w);
        entered |= p.pos[1] < SURFACE as f64;
        if p.on_ground {
            break;
        }
    }
    assert!(entered, "{p:?}");
    assert!(p.on_ground, "settles on the pool floor: {p:?}");
    assert_eq!(p.health, phys.max_health, "{p:?}");
}
