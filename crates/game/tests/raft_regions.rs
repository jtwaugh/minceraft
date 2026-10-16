//! The raft and its rider across region borders (docs/design/world-layout.md
//! Phase D, "Sim support for entities and the raft crossing region borders";
//! raft mask from 4.5). The mask is per region, so the raft is built from the
//! union of the masks and collides against the streamed region set.

use game::input::TickInput;
use game::physics::PlayerPhysics;
use game::player::Player;
use game::raft::{merge_masks, Raft, RaftFrame, RaftWorld};
use game::walk::RegionSet;
use layout::{Layout, RegionCache, RegionCoord, StructurePlacement, REGION_SIZE};
use terrain::{Block, Region, HEIGHT};

const S: i32 = REGION_SIZE as i32;
const SURFACE: u32 = 64;

/// A layout whose raft_start deck straddles the border between regions
/// (0, 0) and (1, 0).
fn layout() -> Layout {
    Layout {
        seed: 9,
        structures: vec![StructurePlacement {
            structure: "raft_start".into(),
            origin: [S - 2, 64, 20],
            rotation: 0,
        }],
        ..Layout::default()
    }
}

fn masks(l: &Layout) -> (Vec<[i32; 3]>, Vec<[i32; 3]>, Vec<[i32; 3]>) {
    let left = l.generate_layout_region_at(RegionCoord::new(0, 0), false);
    let right = l.generate_layout_region_at(RegionCoord::new(1, 0), false);
    let both = l.generate_layout_region(0, 0, 2 * REGION_SIZE, REGION_SIZE, false);
    (left.raft_cells, right.raft_cells, both.raft_cells)
}

/// Stone floor at y = 0 and water up to the surface; the raft floats in it.
fn sea(x0: i32, z0: i32, width: u32, depth: u32) -> Region {
    let mut r = Region {
        blocks: Vec::new(),
        width,
        depth,
        height: HEIGHT as u32,
        x0,
        z0,
    };
    let mut blocks = vec![0u8; width as usize * depth as usize * HEIGHT];
    for z in 0..depth {
        for x in 0..width {
            blocks[r.index(x, 0, z)] = Block::Stone as u8;
            for y in 1..SURFACE {
                blocks[r.index(x, y, z)] = Block::Water as u8;
            }
        }
    }
    r.blocks = blocks;
    r
}

fn boxes(raft: &Raft) -> Vec<([f64; 3], [f64; 3])> {
    raft.boxes().map(|b| (b.min, b.max)).collect()
}

#[test]
fn straddling_raft_is_built_with_all_its_cells() {
    let (left, right, both) = masks(&layout());
    assert!(!left.is_empty() && !right.is_empty());
    let merged = Raft::from_region_masks([left.as_slice(), right.as_slice()]);
    let single = Raft::from_region_masks([both.as_slice()]);
    assert_eq!(boxes(&merged), boxes(&single));
    assert_eq!(merged.centre(), single.centre());
    // Region order and a cell listed by two regions make no difference.
    let swapped = Raft::from_region_masks([right.as_slice(), left.as_slice(), left.as_slice()]);
    assert_eq!(boxes(&merged), boxes(&swapped));
    assert_eq!(
        merge_masks([left.as_slice(), right.as_slice()]).len(),
        both.len()
    );
}

/// Settle a player on the deck, then slide the raft in +x with the rider.
/// Returns the rider's position after every tick.
fn ride(base: &RegionSet, raft: &mut Raft, p: &PlayerPhysics) -> Vec<[f64; 3]> {
    let top = 65.0;
    let c = raft.centre();
    let mut pl = Player::new([c[0], top, c[1]], p);
    for _ in 0..40 {
        pl.tick(&TickInput::default(), p, &RaftWorld::new(base, raft));
    }
    assert!(pl.on_ground);
    let frame = RaftFrame {
        displacement: [0.05, 0.0, 0.0],
        ..RaftFrame::default()
    };
    let mut out = Vec::new();
    for n in 0..200 {
        raft.advance(&frame, &mut [&mut pl], p);
        pl.tick(&TickInput::default(), p, &RaftWorld::new(base, raft));
        assert!(pl.on_ground, "tick {n}: lost support at {:?}", pl.pos);
        out.push(pl.pos);
    }
    out
}

#[test]
fn rider_crosses_the_border_like_on_one_large_region() {
    let (left, right, both) = masks(&layout());
    let p = PlayerPhysics::default_from_data().unwrap();

    let mut two = RegionSet::default();
    two.push(sea(0, 0, REGION_SIZE, REGION_SIZE));
    two.push(sea(S, 0, REGION_SIZE, REGION_SIZE));
    let mut one = RegionSet::default();
    one.push(sea(0, 0, 2 * REGION_SIZE, REGION_SIZE));

    let mut raft_two = Raft::from_region_masks([left.as_slice(), right.as_slice()]);
    let mut raft_one = Raft::from_region_masks([both.as_slice()]);
    let start = raft_two.centre()[0];
    let a = ride(&two, &mut raft_two, &p);
    let b = ride(&one, &mut raft_one, &p);
    assert_eq!(a, b);
    // The rider really moved over the border while standing on the raft.
    assert!(start < f64::from(S) + 5.0);
    assert!((raft_two.offset()[0] - 10.0).abs() < 1e-9);
    assert!(a.iter().any(|q| q[0] < f64::from(S)) && a.iter().any(|q| q[0] >= f64::from(S)));
}

#[test]
fn raft_cells_survive_unloading_and_reloading_the_region() {
    let mut cache = RegionCache::new(layout(), false);
    let merged = |cache: &RegionCache| {
        let at = |rx| {
            cache
                .get(RegionCoord::new(rx, 0))
                .unwrap()
                .raft_cells
                .as_slice()
        };
        merge_masks([at(0), at(1)])
    };
    cache.update(RegionCoord::new(0, 0), 1, 1);
    let before = merged(&cache);
    assert!(!before.is_empty());

    let far = cache.update(RegionCoord::new(20, 0), 0, 0);
    assert!(far.unloaded.contains(&RegionCoord::new(0, 0)));
    assert!(!cache.contains(RegionCoord::new(1, 0)));

    cache.update(RegionCoord::new(1, 0), 1, 1);
    let after = merged(&cache);
    assert_eq!(before, after);
    assert_eq!(boxes(&Raft::new(&before)), boxes(&Raft::new(&after)));
}
