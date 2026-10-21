use super::*;
use crate::collision::block_boxes_in;
use crate::input::TickInput;
use crate::physics::PlayerPhysics;
use crate::player::Player;
use crate::raft::{Raft, RaftWorld};
use layout::{build_layout, default_v0_level, erode, find_path, NavMap, Thresholds, WorldConfig};
use terrain::SEA_LEVEL;

fn run(travel: &mut RaftTravel) -> (u64, [f64; 2]) {
    let mut sum = [0.0; 2];
    let mut ticks = 0;
    while let Some(f) = travel.step() {
        assert_eq!(f.yaw_delta, 0.0);
        assert_eq!(f.tilt, [0.0; 2]);
        assert_eq!(f.displacement[1], 0.0);
        sum[0] += f.displacement[0];
        sum[1] += f.displacement[2];
        ticks += 1;
    }
    (ticks, sum)
}

#[test]
fn speed_comes_from_data() {
    assert_eq!(default_speed().unwrap(), 0.15);
    let t = RaftTravel::from_data(vec![[0.0, 0.0], [3.0, 0.0]]).unwrap();
    assert_eq!(t.speed, 0.15);
    assert!(parse_speed("[travel]\nspeed = 0\n").is_err());
    assert!(parse_speed("").is_err());
}

#[test]
fn ends_on_the_last_point_in_ticks_that_follow_from_length_and_speed() {
    // 3 + 4 blocks along two legs, with a corner.
    let mut t = RaftTravel::new(vec![[1.0, 1.0], [4.0, 1.0], [4.0, 5.0]], 0.5);
    let (ticks, sum) = run(&mut t);
    assert_eq!(ticks, 14);
    assert_eq!(t.position(), [4.0, 5.0]);
    assert!((sum[0] - 3.0).abs() < 1e-9 && (sum[1] - 4.0).abs() < 1e-9);
    assert!(t.is_done() && t.step().is_none());
    // A length that is not a multiple of the speed takes one more tick.
    let mut t = RaftTravel::new(vec![[0.0, 0.0], [1.0, 0.0]], 0.3);
    assert_eq!(run(&mut t).0, 4);
    assert_eq!(t.position(), [1.0, 0.0]);
}

#[test]
fn speed_is_constant_through_corners() {
    let mut t = RaftTravel::new(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], 0.6);
    t.step().unwrap();
    // Second tick: 0.4 along +x to the corner, then 0.2 along +z.
    let d = t.step().unwrap().displacement;
    assert!(
        (d[0] - 0.4).abs() < 1e-9 && (d[2] - 0.2).abs() < 1e-9,
        "{d:?}"
    );
}

#[test]
fn trivial_paths_are_done_at_once() {
    assert!(RaftTravel::new(vec![[2.0, 2.0]], 1.0).step().is_none());
    assert!(RaftTravel::new(Vec::new(), 1.0).step().is_none());
}

#[test]
fn columns_become_centres() {
    assert_eq!(
        column_centres(&[[0, 0], [1, 2]], [10, -4]),
        [[10.5, -3.5], [11.5, -1.5]]
    );
}

/// The travel sim on a generated layout: the raft follows the route found on
/// the eroded navigable set, its boxes never overlap a block, and a rider
/// arrives with it.
#[test]
fn raft_travels_a_pathfound_route_without_collision_and_carries_a_rider() {
    let thresholds = Thresholds::shipped();
    let phys = PlayerPhysics::default_from_data().unwrap();
    let mut travelled = 0;
    for seed in [12345u64, 7, 99] {
        let level = default_v0_level(seed);
        let layout = build_layout(&WorldConfig { seed }, std::slice::from_ref(&level));
        // Navigability is measured on terrain without the raft stamped in.
        let bare = layout::Layout {
            structures: Vec::new(),
            ..layout.clone()
        };
        let (x0, z0, size) = (-48, -48, 96u32);
        let nav = NavMap::from_region(&bare.generate_region(x0, z0, size, size, true), &thresholds);
        let eroded = erode(&nav, thresholds.raft_half_width);
        let start = [-x0, -z0];
        let mut found = None;
        for r in (10..44).rev() {
            for goal in [
                [start[0] + r, start[1]],
                [start[0], start[1] + r],
                [start[0] - r, start[1]],
            ] {
                if let Some(p) = find_path(&eroded, start, goal) {
                    found = Some(p);
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        let Some(path) = found else { continue };
        assert!(path.len() > 5, "seed {seed}");

        let out = layout.generate_layout_region(x0, z0, size, size, true);
        let mut raft = Raft::new(&out.raft_cells);
        let mut points = column_centres(&path, [x0, z0]);
        // Start from the raft's own centre, keeping the route's shape.
        let shift = [
            raft.centre()[0] - points[0][0],
            raft.centre()[1] - points[0][1],
        ];
        for p in &mut points {
            p[0] += shift[0];
            p[1] += shift[1];
        }
        let end = *points.last().unwrap();
        let mut travel = RaftTravel::from_data(points).unwrap();

        let base = crate::collision::RegionWorld::new(&out.region);
        let mut rider = Player::new(
            [raft.centre()[0], SEA_LEVEL as f64 + 1.0, raft.centre()[1]],
            &phys,
        );
        let settled = RaftWorld::new(&base, &raft);
        for _ in 0..20 {
            rider.tick(&TickInput::default(), &phys, &settled);
        }
        assert!(rider.on_ground, "seed {seed}");
        let rest = rider.pos;
        let mut ticks = 0;
        while let Some(frame) = travel.step() {
            raft.advance(&frame, &mut [&mut rider], &phys);
            let world = RaftWorld::new(&base, &raft);
            for b in raft.boxes() {
                let mut hit = Vec::new();
                block_boxes_in(&world, &b, &mut hit);
                assert!(hit.is_empty(), "seed {seed} tick {ticks}: {b:?} in {hit:?}");
            }
            rider.tick(&TickInput::default(), &phys, &world);
            assert!(rider.on_ground, "seed {seed} tick {ticks}");
            ticks += 1;
        }
        assert_eq!(raft.quarter_turns(), 0);
        assert!(
            (raft.centre()[0] - end[0]).abs() < 1e-6 && (raft.centre()[1] - end[1]).abs() < 1e-6
        );
        let off = raft.offset();
        assert!(
            (rider.pos[0] - (rest[0] + off[0])).abs() < 1e-6,
            "seed {seed}"
        );
        assert!(
            (rider.pos[2] - (rest[2] + off[2])).abs() < 1e-6,
            "seed {seed}"
        );
        assert!(ticks > 30);
        travelled += 1;
    }
    assert!(travelled > 0, "no seed had a route");
}
