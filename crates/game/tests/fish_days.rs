//! Fish over several in-game days with the raft at anchor: the Phase B "done
//! when" of docs/design/fish-and-fishing.md (sections 3.1 and 3.2), on real
//! streamed terrain. The stock reset each morning is a separate ticket.

use game::collision::{Aabb, CollisionWorld};
use game::fish_entities::FishEntity;
use game::fish_spawn_config::FishSpawnConfig;
use game::fish_world::FishWorld;
use game::raft::{Raft, RaftWorld};
use game::stream::{StreamConfig, Streamer};
use game::targeting::EntityId;
use game::walk::RegionSet;
use layout::{Fish, Layout};
use terrain::SEA_LEVEL;

const SEED: u64 = 25;
/// Three gathering-plus-service days.
const TICKS: u64 = 3 * 4800;

struct Sea {
    streamer: Streamer,
    regions: RegionSet,
    raft: Raft,
    /// The raft centre, x and z.
    anchor: [f64; 2],
}

/// Streams the world around the deepest water column near the middle of
/// region (0, 0) and anchors a small raft on it, in the top water layer.
fn sea() -> Sea {
    let layout = Layout {
        seed: SEED,
        ..Layout::default()
    };
    let mut streamer = Streamer::new(layout, StreamConfig::default_from_data().unwrap());
    let mut regions = RegionSet::default();
    streamer.update([64.5, 80.0, 64.5], &mut regions);
    let mut best = None;
    for z in (32..96).step_by(2) {
        for x in (32..96).step_by(2) {
            if let Some(c) = streamer.fish_column(x, z) {
                if best.is_none_or(|(d, _, _)| c.depth > d) {
                    best = Some((c.depth, x, z));
                }
            }
        }
    }
    let (depth, x, z) = best.expect("no water near the middle of region (0, 0)");
    assert!(depth >= 3, "deepest water is {depth}");
    let y = SEA_LEVEL - 1;
    let raft = Raft::new(&[[x, y, z], [x + 1, y, z], [x, y, z + 1], [x + 1, y, z + 1]]);
    let anchor = raft.centre();
    Sea {
        streamer,
        regions,
        raft,
        anchor,
    }
}

/// Every cell the fish's box spans is water.
fn in_water(world: &impl CollisionWorld, f: &FishEntity) -> bool {
    let bb = f.aabb();
    let lo = |i: usize| bb.min[i].floor() as i32;
    let hi = |i: usize| bb.max[i].ceil() as i32 - 1;
    (lo(0)..=hi(0)).all(|x| {
        (lo(1)..=hi(1)).all(|y| (lo(2)..=hi(2)).all(|z| world.block_at(x, y, z).info().is_water()))
    })
}

/// Runs the days, checking every tick, and returns the fish positions after
/// each tick.
fn run() -> Vec<Vec<(EntityId, [f64; 3])>> {
    let Sea {
        mut streamer,
        regions,
        raft,
        anchor,
    } = sea();
    let world = RaftWorld::new(&regions, &raft);
    // The player stands on the deck, out of the water.
    let deck = f64::from(SEA_LEVEL);
    let player = Aabb::from_size([anchor[0] - 0.3, deck, anchor[1] - 0.3], [0.6, 1.8, 0.6]);
    let config = FishSpawnConfig::default_from_data().unwrap();
    let fish = Fish::shipped();
    let mut fw = FishWorld::default_from_data(SEED, 1000).unwrap();
    let mut seen: Vec<EntityId> = Vec::new();
    let mut spawn_ticks = Vec::new();
    let mut trace = Vec::new();
    for tick in 0..TICKS {
        let stock = fw.stock();
        fw.tick(tick, anchor, &player, &world, |x, z| {
            streamer.fish_column(x, z)
        });
        let new: Vec<&FishEntity> = fw
            .store()
            .iter()
            .filter(|f| !seen.contains(&f.id))
            .collect();
        if !new.is_empty() {
            assert!(fw.is_phase_change(tick), "fish spawned at tick {tick}");
            assert!(new.len() as u32 <= stock, "tick {tick}: over the stock");
            spawn_ticks.push(tick);
        }
        for f in &new {
            let (x, z) = (f.pos[0].floor() as i32, f.pos[2].floor() as i32);
            let col = streamer.fish_column(x, z).expect("spawned on land");
            let allowed = fish.species_for(&col);
            assert!(
                allowed.iter().any(|(s, _)| *s == f.species),
                "{f:?} in {col:?}"
            );
            seen.push(f.id);
        }
        let near = fw
            .store()
            .iter()
            .filter(|f| (f.pos[0] - anchor[0]).hypot(f.pos[2] - anchor[1]) <= config.target_radius)
            .count() as u32;
        assert!(near <= config.target_count, "tick {tick}: {near} fish");
        assert!(fw.store().len() as u32 + fw.stock() <= config.daily_stock);
        for f in fw.store().iter() {
            assert!(in_water(&world, f), "tick {tick}: {f:?} out of water");
        }
        trace.push(fw.store().iter().map(|f| (f.id, f.pos)).collect());
    }
    // Fish that wander out of the target radius are topped up on later days.
    assert_eq!(spawn_ticks.first(), Some(&0), "{spawn_ticks:?}");
    assert!(spawn_ticks.len() >= 2, "{spawn_ticks:?}");
    assert!(seen.len() >= 6, "only {} fish spawned", seen.len());
    trace
}

#[test]
fn fish_at_anchor_over_several_days() {
    let a = run();
    let b = run();
    assert!(a == b, "two runs gave different fish positions");
}
