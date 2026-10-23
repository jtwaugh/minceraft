//! Headless farming session: the measurable form of the Phase 7 done-when in
//! docs/design/basic-gameplay.md (section 13) and the farming tests of
//! section 14 ("Hoe tills grass and dirt only with air above", "harvest
//! works only on mature crops and resets to stage 0", and the replay).
//!
//! A `Walker` on a flat grass floor is driven by scripted `TickInput`s: the
//! player tills, plants, waits for the crop to grow, harvests, and forages.

use game::input::TickInput;
use game::walk::Walker;
use terrain::{Block, Region, HEIGHT};

/// Top layer of the floor; the player stands at `y = TOP + 1`.
const TOP: i32 = 3;
const SIZE: u32 = 32;
const HALF: i32 = 16;
const EMPTY_SLOT: u8 = 0;
const HOE_SLOT: u8 = 2;
/// Ticks in one 4-minute day at 20 Hz.
const DAY: u64 = 4800;
/// The field tile, water beside it, and the tall grass to forage.
const FIELD: [i32; 2] = [0, 0];
const WATER: [i32; 2] = [2, 0];
const TALL_GRASS: [i32; 2] = [6, 6];

fn set(r: &mut Region, x: i32, y: i32, z: i32, block: Block) {
    let i = r.index((x + HALF) as u32, y as u32, (z + HALF) as u32);
    r.blocks[i] = block as u8;
}

/// Stone below a grass top, with water and tall grass placed.
fn floor() -> Region {
    let mut r = Region {
        blocks: vec![0u8; SIZE as usize * SIZE as usize * HEIGHT],
        width: SIZE,
        depth: SIZE,
        height: HEIGHT as u32,
        x0: -HALF,
        z0: -HALF,
    };
    for z in -HALF..HALF {
        for x in -HALF..HALF {
            for y in 0..TOP {
                set(&mut r, x, y, z, Block::Stone);
            }
            set(&mut r, x, TOP, z, Block::Grass);
        }
    }
    set(&mut r, WATER[0], TOP, WATER[1], Block::Water);
    set(
        &mut r,
        TALL_GRASS[0],
        TOP + 1,
        TALL_GRASS[1],
        Block::TallGrass,
    );
    r
}

fn idle() -> TickInput {
    TickInput::default()
}

/// Looking straight down.
fn down() -> TickInput {
    TickInput {
        pitch: -std::f32::consts::FRAC_PI_2,
        ..idle()
    }
}

fn press() -> TickInput {
    TickInput {
        secondary: true,
        ..down()
    }
}

fn start() -> Walker {
    let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
    w.add_region(floor());
    for _ in 0..40 {
        w.tick(&idle());
    }
    assert!(w.player().on_ground);
    w
}

fn select(w: &mut Walker, slot: u8) {
    w.tick(&TickInput {
        tool_slot: Some(slot),
        ..idle()
    });
}

/// Stand on the column `(x, z)` and let the aim settle.
fn stand(w: &mut Walker, [x, z]: [i32; 2]) {
    w.teleport([f64::from(x) + 0.5, f64::from(TOP) + 1.0, f64::from(z) + 0.5]);
    w.tick(&down());
}

fn at(w: &Walker, [x, z]: [i32; 2], dy: i32) -> Block {
    w.block_at(x, TOP + dy, z)
}

fn count(w: &Walker, item: &str) -> usize {
    w.items().iter().filter(|e| e.item.item == item).count()
}

/// Put seeds on the ground in front of the player and pick them up.
fn pick_up_seeds(w: &mut Walker) {
    select(w, EMPTY_SLOT);
    stand(w, [FIELD[0], FIELD[1] - 3]);
    w.clear_items();
    w.spawn_item(
        game::items::ItemInstance {
            item: "seeds".into(),
        },
        [0.5, 4.5, -1.5],
    );
    for _ in 0..40 {
        w.tick(&idle());
    }
    let pos = w.items().iter().next().expect("seeds on the ground").pos;
    let eye = w.eye(1.0);
    let d = [0, 1, 2].map(|i| pos[i] + [0.0, 0.1, 0.0][i] - eye[i]);
    let flat = d[0].hypot(d[2]);
    let aim = TickInput {
        yaw: d[0].atan2(-d[2]) as f32,
        pitch: d[1].atan2(flat) as f32,
        ..idle()
    };
    w.tick(&aim);
    w.tick(&TickInput {
        interact: true,
        ..aim
    });
    assert_eq!(w.hands().map(|h| h.item.as_str()), Some("seeds"));
}

/// Hash of every block in the farm area, so two runs can be compared.
fn overlay_hash(w: &Walker) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for y in 0..8 {
        for z in -HALF..HALF {
            for x in -HALF..HALF {
                h = (h ^ w.block_at(x, y, z) as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    h
}

#[test]
fn till_plant_grow_pick_and_the_crop_is_gone() {
    let mut w = start();
    select(&mut w, HOE_SLOT);
    stand(&mut w, FIELD);
    w.tick(&press());
    assert!(matches!(
        at(&w, FIELD, 0),
        Block::FarmlandDry | Block::FarmlandWet
    ));

    pick_up_seeds(&mut w);
    stand(&mut w, FIELD);
    w.tick(&press());
    assert_eq!(at(&w, FIELD, 1), Block::Wheat0);
    assert!(w.hands().is_none(), "the seed was consumed");

    // Grows a stage at a time, never skipping one. A lone crop is slower
    // than a row: about 5 days at the data's odds.
    let mut last = 0;
    let mut ticks = 0;
    while at(&w, FIELD, 1) != Block::Wheat7 {
        w.tick(&idle());
        ticks += 1;
        assert!(
            ticks <= 8 * DAY,
            "ready within eight days: {:?} {last}",
            at(&w, FIELD, 1)
        );
        let stage = at(&w, FIELD, 1) as u8 - Block::Wheat0 as u8;
        assert!(stage == last || stage == last + 1);
        last = stage;
    }
    assert!(ticks > DAY, "not instant: took {ticks}");

    // Pick with empty hands: produce drops and the crop is gone.
    select(&mut w, EMPTY_SLOT);
    w.clear_items();
    w.tick(&press());
    assert_eq!(at(&w, FIELD, 1), Block::Air);
    assert!(matches!(
        at(&w, FIELD, 0),
        Block::FarmlandDry | Block::FarmlandWet
    ));
    assert_eq!(count(&w, "wheat"), 1);
    assert!(count(&w, "seeds") <= 2);
    assert_eq!(w.items().len(), 1 + count(&w, "seeds"));

    // Nothing left to pick.
    w.clear_items();
    w.tick(&press());
    assert_eq!(at(&w, FIELD, 1), Block::Air);
    assert_eq!(w.items().len(), 0);
}

#[test]
fn hoe_changes_only_grass_and_dirt_with_air_above() {
    // A row of other terrain next to untouched grass, plus a covered grass.
    let mut r = floor();
    let others = [Block::Stone, Block::Sand, Block::Gravel, Block::Water];
    for (i, b) in others.iter().enumerate() {
        set(&mut r, i as i32 - 2, TOP, -4, *b);
    }
    set(&mut r, 2, TOP, -4, Block::Dirt);
    set(&mut r, -2, TOP, -3, Block::Grass);
    set(&mut r, -2, TOP + 1, -3, Block::Stone);
    let mut w = {
        let mut f = Walker::new([0.5, 5.0, 0.5]).unwrap();
        f.add_region(r);
        for _ in 0..40 {
            f.tick(&idle());
        }
        f
    };
    let before: Vec<Block> = (-6..=6)
        .flat_map(|x| (-8..=2).map(move |z| (x, z)))
        .flat_map(|(x, z)| (0..8).map(move |y| (x, y, z)))
        .map(|(x, y, z)| w.block_at(x, y, z))
        .collect();
    select(&mut w, HOE_SLOT);
    // Aim at the middle of each cell's top from the spawn point, pressing
    // once per cell.
    for z in -4..=-1 {
        for x in -2..=2 {
            let eye = w.eye(1.0);
            let d = [
                f64::from(x) + 0.5 - eye[0],
                f64::from(TOP) + 1.0 - eye[1],
                f64::from(z) + 0.5 - eye[2],
            ];
            let aim = TickInput {
                yaw: d[0].atan2(-d[2]) as f32,
                pitch: d[1].atan2(d[0].hypot(d[2])) as f32,
                secondary: true,
                ..idle()
            };
            w.tick(&aim);
            for _ in 0..4 {
                w.tick(&idle());
            }
        }
    }
    let mut i = 0;
    for x in -6..=6 {
        for z in -8..=2 {
            for y in 0..8 {
                let (b0, b1) = (before[i], w.block_at(x, y, z));
                i += 1;
                if b0 == b1 {
                    continue;
                }
                assert!(
                    matches!(b0, Block::Grass | Block::Dirt),
                    "{b0:?} became {b1:?} at {x},{y},{z}"
                );
                assert!(
                    matches!(b1, Block::FarmlandDry | Block::FarmlandWet),
                    "{b0:?} became {b1:?}"
                );
                assert_eq!(w.block_at(x, y + 1, z), Block::Air, "air above");
            }
        }
    }
    assert!(
        matches!(
            w.block_at(-1, TOP, -3),
            Block::FarmlandDry | Block::FarmlandWet
        ),
        "plain grass is tilled"
    );
    assert_eq!(w.block_at(-2, TOP, -3), Block::Grass, "covered grass stays");
    assert_eq!(w.block_at(0, TOP, -4), Block::Gravel);
}

#[test]
fn foraging_drops_seeds_keeps_the_grass_and_respects_the_cooldown() {
    let mut w = start();
    select(&mut w, EMPTY_SLOT);
    stand(&mut w, TALL_GRASS);
    let mut found = false;
    for _ in 0..60 {
        w.clear_items();
        w.tick(&press());
        assert_eq!(at(&w, TALL_GRASS, 1), Block::TallGrass, "the grass stays");
        if count(&w, "seeds") > 0 {
            found = true;
            // Pressing again inside the cooldown yields nothing.
            for _ in 0..20 {
                w.clear_items();
                w.tick(&press());
                assert_eq!(w.items().len(), 0);
            }
            break;
        }
        // Wait out the cooldown (one minute) before the next try.
        for _ in 0..1200 {
            w.tick(&idle());
        }
    }
    assert!(found, "1 in 3 over 60 tries");
}

/// The same script every time: till, plant, grow, harvest, forage.
fn run_session() -> (u64, usize) {
    let mut w = start();
    select(&mut w, HOE_SLOT);
    stand(&mut w, FIELD);
    w.tick(&press());
    pick_up_seeds(&mut w);
    stand(&mut w, FIELD);
    w.tick(&press());
    for _ in 0..8 * DAY {
        w.tick(&idle());
        if at(&w, FIELD, 1) == Block::Wheat7 {
            break;
        }
    }
    select(&mut w, EMPTY_SLOT);
    w.tick(&press());
    stand(&mut w, TALL_GRASS);
    for _ in 0..5 {
        w.tick(&press());
        for _ in 0..1200 {
            w.tick(&idle());
        }
    }
    (overlay_hash(&w), w.items().len())
}

#[test]
fn identical_scripted_sessions_have_identical_overlay_hashes() {
    let (a, items) = run_session();
    assert!(items > 0, "the session produced drops");
    assert_eq!((a, items), run_session());
}
