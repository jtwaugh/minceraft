//! Headless raft stability and drift check: the measurable form of the
//! Phase 3 raft test in docs/design/basic-gameplay.md (sections 4.7 and 14).
//!
//! The raft is the layout crate's real `raft_start` mask, floating in a sea
//! built in a `MemoryWorld`. Scripted `RaftFrame`s move it each tick.

use game::collision::{CollisionWorld, MemoryWorld};
use game::input::TickInput;
use game::physics::PlayerPhysics;
use game::player::Player;
use game::raft::{Raft, RaftFrame, RaftWorld};
use layout::{Layout, StructurePlacement};
use terrain::Block;

const ORIGIN: [i32; 3] = [0, 64, 0];
/// Water fills `y = 1..SURFACE` over the whole sea.
const SURFACE: i32 = 64;
const SEA: i32 = 24;
const TICKS: u32 = 1000;
const MAX_TICKS: u32 = 300;

fn physics() -> PlayerPhysics {
    PlayerPhysics::default_from_data().unwrap()
}

fn cells() -> Vec<[i32; 3]> {
    let l = Layout {
        structures: vec![StructurePlacement {
            structure: "raft_start".into(),
            origin: ORIGIN,
            rotation: 0,
        }],
        ..Layout::default()
    };
    let mut c = l.generate_layout_region(-SEA, -SEA, 2 * SEA as u32, 2 * SEA as u32, false);
    c.raft_cells.sort();
    c.raft_cells
}

/// Sea floor and water, with the raft mask cells absent from the base.
fn sea() -> (MemoryWorld, Raft) {
    let cells = cells();
    assert!(!cells.is_empty());
    let mut w = MemoryWorld::new();
    for x in -SEA..SEA {
        for z in -SEA..SEA {
            w.set(x, 0, z, Block::Stone);
            for y in 1..SURFACE {
                w.set(x, y, z, Block::Water);
            }
        }
    }
    (w, Raft::new(&cells))
}

/// The deck surface (the station cells above it are ignored) and the deck's
/// footprint bounds `[min_x, max_x, min_z, max_z]` (cells inclusive).
fn deck_top(raft: &Raft) -> (i32, [i32; 4]) {
    let top = ORIGIN[1] as f64 + 1.0;
    let (mut x0, mut x1, mut z0, mut z1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for b in raft.boxes().filter(|b| b.max[1] == top) {
        x0 = x0.min(b.min[0].floor() as i32);
        x1 = x1.max(b.max[0].ceil() as i32 - 1);
        z0 = z0.min(b.min[2].floor() as i32);
        z1 = z1.max(b.max[2].ceil() as i32 - 1);
    }
    (top as i32, [x0, x1, z0, z1])
}

fn centre_player(raft: &Raft, p: &PlayerPhysics) -> Player {
    let (top, _) = deck_top(raft);
    let c = raft.centre();
    Player::new([c[0], top as f64, c[1]], p)
}

/// Ticks the player is given to land on the still raft before it moves.
const SETTLE: u32 = 40;

/// Settle the player on the still raft, then run `ticks` with the raft
/// swaying along a fixed path. Returns the final player, its xz position
/// relative to where it settled (in the raft's frame), and whether the raft
/// held it up at the end.
fn ride(ticks: u32, tilt: [f64; 2], sneak: bool) -> (Player, [f64; 2], bool) {
    let (base, mut raft) = sea();
    let phys = physics();
    let mut pl = centre_player(&raft, &phys);
    let input = TickInput {
        sneak,
        ..TickInput::default()
    };
    for _ in 0..SETTLE {
        pl.tick(&input, &phys, &RaftWorld::new(&base, &raft));
    }
    assert!(pl.on_ground && raft.supports(&pl.aabb(&phys)), "{pl:?}");
    let c0 = raft.centre();
    let start = [pl.pos[0] - c0[0], pl.pos[2] - c0[1]];
    for t in 0..ticks {
        let phase = t as f64 * 0.05;
        let frame = RaftFrame {
            displacement: [0.02 * phase.sin(), 0.0, 0.015 * phase.cos()],
            yaw_delta: 0.0,
            tilt,
        };
        raft.advance(&frame, &mut [&mut pl], &phys);
        let world = RaftWorld::new(&base, &raft);
        pl.tick(&input, &phys, &world);
    }
    let c = raft.centre();
    let rel = [pl.pos[0] - c[0] - start[0], pl.pos[2] - c[1] - start[1]];
    let held = raft.supports(&pl.aabb(&phys));
    (pl, rel, held)
}

#[test]
fn stays_on_a_moving_raft_for_1000_ticks_without_swell() {
    let (pl, rel, held) = ride(TICKS, [0.0; 2], false);
    assert!(held && pl.on_ground, "{pl:?}");
    assert!(rel[0].abs() < 1e-6 && rel[1].abs() < 1e-6, "{rel:?}");
}

#[test]
fn swell_does_not_push_the_rider_sneaking_or_not() {
    for sneak in [false, true] {
        let (pl, rel, held) = ride(TICKS, [3.0, 3.0], sneak);
        assert!(held && pl.on_ground, "{pl:?}");
        assert!(rel[0].abs() < 1e-6 && rel[1].abs() < 1e-6, "{rel:?}");
    }
}

#[test]
fn ride_is_deterministic() {
    for (tilt, sneak) in [([0.0; 2], false), ([3.0, -2.0], false), ([3.0, 3.0], true)] {
        assert_eq!(ride(TICKS, tilt, sneak), ride(TICKS, tilt, sneak));
    }
}

/// Start in the water 3 blocks off the raft side `dir`, then swim at the
/// still raft and climb onto it. Returns the player and raft.
fn climb(dir: [i32; 2]) -> (Player, Raft) {
    let (base, raft) = sea();
    let phys = physics();
    let (top, [x0, x1, z0, z1]) = deck_top(&raft);
    let c = raft.centre();
    let edge = |d: i32, lo: i32, hi: i32| if d > 0 { hi as f64 + 1.0 } else { lo as f64 };
    let start = [
        if dir[0] != 0 {
            edge(dir[0], x0, x1) + 3.0 * dir[0] as f64
        } else {
            c[0]
        },
        SURFACE as f64 - 1.0,
        if dir[1] != 0 {
            edge(dir[1], z0, z1) + 3.0 * dir[1] as f64
        } else {
            c[1]
        },
    ];
    let mut pl = Player::new(start, &phys);
    // Forward is -z at yaw 0; walk against `dir`.
    let yaw = ((-dir[0]) as f64).atan2(dir[1] as f64) as f32;
    let input = TickInput {
        move_forward: 1.0,
        yaw,
        jump: true,
        ..TickInput::default()
    };
    for _ in 0..MAX_TICKS {
        let world = RaftWorld::new(&base, &raft);
        pl.tick(&input, &phys, &world);
        if pl.on_ground && raft.supports(&pl.aabb(&phys)) && pl.pos[1] >= top as f64 - 1e-6 {
            break;
        }
    }
    (pl, raft)
}

#[test]
fn swims_to_the_raft_and_climbs_on_from_all_four_sides() {
    let phys = physics();
    for dir in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        let (pl, raft) = climb(dir);
        let (top, _) = deck_top(&raft);
        assert!(pl.on_ground, "{dir:?}: {pl:?}");
        assert!(raft.supports(&pl.aabb(&phys)), "{dir:?}: {pl:?}");
        assert!(pl.pos[1] >= top as f64 - 1e-6, "{dir:?}: {pl:?}");
        assert_eq!(climb(dir).0, pl, "{dir:?} deterministic");
    }
}

#[test]
fn raft_world_leaves_rest_cells_open() {
    let (base, raft) = sea();
    let world = RaftWorld::new(&base, &raft);
    for c in cells() {
        assert_eq!(world.block_at(c[0], c[1], c[2]), Block::Air);
    }
}
