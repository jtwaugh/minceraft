//! Headless "walk around the island" check: the measurable form of the
//! Phase 2 done-when in docs/design/basic-gameplay.md (reconciliation note 10).
//!
//! Builds the v0 level with `crates/layout`, drops a `Player` on dry land at
//! the island's west shore and drives it with scripted `TickInput` over the
//! island to the east shore, then fuzzes random input on decorated terrain.

use game::collision::{block_box, Aabb, CollisionWorld, RegionWorld};
use game::input::TickInput;
use game::physics::PlayerPhysics;
use game::player::Player;
use layout::{build_layout, Layout, LevelFile};
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

/// Region covering the island and its surroundings (world coordinates).
const X0: i32 = -20;
const Z0: i32 = -30;
const W: u32 = 100;
const D: u32 = 90;

/// How close to the target the scripted path must end, in blocks.
const TOLERANCE: f64 = 0.5;
/// Overlap deeper than this with a blocking box counts as being inside it.
const OVERLAP_EPS: f64 = 1e-6;

fn load() -> (Layout, [i32; 2]) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/levels/v0.toml");
    let file = LevelFile::load(path).expect("v0 level file");
    let layout = build_layout(&file.world, &file.levels);
    (layout, file.levels[0].island_center)
}

fn physics() -> PlayerPhysics {
    PlayerPhysics::default_from_data().unwrap()
}

/// Y of the first free block above the highest non-air block in the column.
fn stand_y(world: &impl CollisionWorld, x: i32, z: i32) -> i32 {
    (1..HEIGHT as i32)
        .rev()
        .find(|&y| world.block_at(x, y, z) != Block::Air)
        .map_or(0, |y| y + 1)
}

/// Whether a player can stand in column `(x, z)`: dry and above the sea.
fn is_dry(world: &impl CollisionWorld, x: i32, z: i32) -> bool {
    let y = stand_y(world, x, z);
    y > SEA_LEVEL + 1 && world.block_at(x, y - 1, z) != Block::Water
}

/// FNV-1a over the bits of every field of the player's state.
fn hash_state(h: &mut u64, p: &Player) {
    let mut put = |bits: u64| {
        for b in bits.to_le_bytes() {
            *h = (*h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    };
    for v in p.pos.iter().chain(&p.vel).chain(&[p.fall_distance]) {
        put(v.to_bits());
    }
    put(p.on_ground as u64);
    put(p.health as u64);
}

const HASH_SEED: u64 = 0xcbf2_9ce4_8422_2325;

/// The scripted driver: walk straight at `target`, jumping whenever the block
/// ahead of the feet is solid. The yaw faces the target (forward is `-z` at
/// yaw 0), so the whole path is a function of position alone.
fn steer(p: &Player, target: [f64; 2], world: &impl CollisionWorld) -> TickInput {
    let (dx, dz) = (target[0] - p.pos[0], target[1] - p.pos[2]);
    let dist = dx.hypot(dz);
    if dist < 0.2 {
        return TickInput::default();
    }
    let (ux, uz) = (dx / dist, dz / dist);
    let (ax, az) = (p.pos[0] + ux * 0.6, p.pos[2] + uz * 0.6);
    let feet = p.pos[1].floor() as i32;
    let (bx, bz) = (ax.floor() as i32, az.floor() as i32);
    let blocked = block_box(world.block_at(bx, feet, bz), bx, feet, bz).is_some();
    TickInput {
        move_forward: 1.0,
        yaw: dx.atan2(-dz) as f32,
        jump: blocked,
        ..TickInput::default()
    }
}

struct Walk {
    end: Player,
    hash: u64,
    /// Highest and lowest feet height seen while walking.
    max_y: f64,
    min_y: f64,
    /// Times the standing level rose (steps up) or fell (ledges) by a block.
    steps_up: u32,
    ledges_down: u32,
}

/// Walk from the west shore across the island's centre to the east shore.
fn scripted_walk(region: &Region, center: [i32; 2]) -> (Walk, [f64; 2]) {
    let world = RegionWorld::new(region);
    let phys = physics();
    // The nearest row to the island's centre whose dry span has no rise
    // above one block (a jump clears one block, not two). Shores are the
    // ends of that span.
    let span = |z: i32| {
        let shore = |dir: i32| {
            let mut x = center[0];
            while is_dry(&world, x + dir, z) {
                x += dir;
            }
            x
        };
        (shore(-1), shore(1))
    };
    let climbable = |z: i32| {
        let (west, east) = span(z);
        is_dry(&world, center[0], z)
            && (west..east).all(|x| stand_y(&world, x + 1, z) - stand_y(&world, x, z) <= 1)
    };
    let z = (0..=20)
        .flat_map(|d| [center[1] + d, center[1] - d])
        .find(|&z| climbable(z))
        .expect("a climbable row across the island");
    let (west, east) = span(z);
    let start = [west as f64 + 0.5, z as f64 + 0.5];
    let target = [east as f64 + 0.5, z as f64 + 0.5];
    let y = stand_y(&world, west, z) as f64;
    let mut p = Player::new([start[0], y, start[1]], &phys);
    let mut hash = HASH_SEED;
    let mut level = y;
    let mut w = Walk {
        end: p.clone(),
        hash,
        max_y: y,
        min_y: y,
        steps_up: 0,
        ledges_down: 0,
    };
    for _ in 0..2000 {
        let input = steer(&p, target, &world);
        p.tick(&input, &phys, &world);
        hash_state(&mut hash, &p);
        w.max_y = w.max_y.max(p.pos[1]);
        w.min_y = w.min_y.min(p.pos[1]);
        if p.on_ground && (p.pos[1] - level).abs() >= 0.5 {
            if p.pos[1] > level {
                w.steps_up += 1;
            } else {
                w.ledges_down += 1;
            }
            level = p.pos[1];
        }
    }
    w.end = p;
    w.hash = hash;
    (w, target)
}

#[test]
fn scripted_path_crosses_the_island() {
    let (layout, center) = load();
    let region = layout.generate_region(X0, Z0, W, D, false);
    let (w, target) = scripted_walk(&region, center);
    let err = (w.end.pos[0] - target[0]).hypot(w.end.pos[2] - target[1]);
    assert!(
        err <= TOLERANCE,
        "ended {err:.2} blocks from target: {:?}",
        w.end
    );
    assert!(w.end.on_ground);
    assert_eq!(w.end.health, physics().max_health, "{:?}", w.end);
    // The route climbs over the island (steps) and comes down the far side
    // (ledges), not along a flat shore.
    assert!(w.max_y - w.min_y >= 4.0, "{} to {}", w.min_y, w.max_y);
    assert!(
        w.steps_up >= 3 && w.ledges_down >= 1,
        "{} steps, {} ledges",
        w.steps_up,
        w.ledges_down
    );
}

#[test]
fn scripted_path_is_repeatable() {
    let (layout, center) = load();
    let region = layout.generate_region(X0, Z0, W, D, false);
    let (a, _) = scripted_walk(&region, center);
    let (b, _) = scripted_walk(&region, center);
    assert_eq!(a.hash, b.hash);
    assert_eq!(a.end, b.end);
}

/// xorshift64*: a fixed-seed generator, so the fuzz run is repeatable.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// Uniform in `[-1, 1]`.
    fn axis(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    fn chance(&mut self, one_in: u64) -> bool {
        self.next().is_multiple_of(one_in)
    }
}

/// Random input for `ticks` ticks on decorated terrain, checking after every
/// tick that the body overlaps no blocking box. Returns the state hash.
fn fuzz(region: &Region, center: [i32; 2], seed: u64, ticks: u32) -> u64 {
    let world = RegionWorld::new(region);
    let phys = physics();
    let mut rng = Rng(seed);
    // Start on dry land a few blocks off the island's centre.
    let (sx, sz) = (center[0] - 6, center[1] + 3);
    let sy = stand_y(&world, sx, sz) as f64;
    let mut p = Player::new([sx as f64 + 0.5, sy, sz as f64 + 0.5], &phys);
    let mut input = TickInput::default();
    let mut hash = HASH_SEED;
    let mut boxes: Vec<Aabb> = Vec::new();
    for t in 0..ticks {
        // Hold each random input for a while, like a key press.
        if t % 10 == 0 {
            input = TickInput {
                move_forward: rng.axis().round(),
                move_strafe: rng.axis().round(),
                yaw: rng.axis() * std::f32::consts::PI,
                jump: !rng.chance(4),
                sneak: rng.chance(4),
                ..TickInput::default()
            };
        }
        p.tick(&input, &phys, &world);
        hash_state(&mut hash, &p);
        // Stay inside the generated region; outside it is empty air.
        let inside =
            |v: f64, lo: i32, n: u32| v > lo as f64 + 2.0 && v < (lo + n as i32) as f64 - 2.0;
        if !inside(p.pos[0], X0, W) || !inside(p.pos[2], Z0, D) {
            break;
        }
        let bb = p.aabb(&phys);
        boxes.clear();
        world.boxes_in(&bb, &mut boxes);
        for b in &boxes {
            let depth = (0..3)
                .map(|i| bb.max[i].min(b.max[i]) - bb.min[i].max(b.min[i]))
                .fold(f64::INFINITY, f64::min);
            assert!(depth <= OVERLAP_EPS, "tick {t}: inside {b:?}: {p:?}");
        }
    }
    hash
}

#[test]
fn fuzzed_input_never_enters_a_blocking_box() {
    let (layout, center) = load();
    let region = layout.generate_region(X0, Z0, W, D, true);
    for seed in 1..=20u64 {
        let a = fuzz(&region, center, seed, 3000);
        let b = fuzz(&region, center, seed, 3000);
        assert_eq!(a, b, "seed {seed} differs between runs");
    }
}
