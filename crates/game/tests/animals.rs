//! Headless sword and animals session: the measurable form of the Phase 6
//! done-when in docs/design/basic-gameplay.md (section 14: "One click
//! produces one attack" and "Wooden sword kills").
//!
//! A `Walker` on a flat stone floor is driven by scripted `TickInput`s: a
//! sword is selected and the player hits a cow, a pig and a chicken.

use game::animal_entities::AnimalEntity;
use game::input::{InputAccumulator, TickInput};
use game::item_entities::ItemEntity;
use game::items::ItemInstance;
use game::player::Player;
use game::targeting::EntityId;
use game::walk::Walker;
use terrain::{Block, Region, HEIGHT};

/// Top stone layer of the floor; the player stands at `y = TOP + 1`.
const TOP: u32 = 3;
const SIZE: u32 = 32;
const SWORD_SLOT: u8 = 1;
/// Ticks that outlast every `hurt_ticks` in data/animals.toml.
const WINDOW: usize = 12;

fn floor() -> Region {
    let (x0, z0) = (-16, -16);
    let mut r = Region {
        blocks: Vec::new(),
        width: SIZE,
        depth: SIZE,
        height: HEIGHT as u32,
        x0,
        z0,
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

fn idle() -> TickInput {
    TickInput::default()
}

/// A walker on the floor with the sword selected, standing still.
fn start() -> Walker {
    let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
    w.add_region(floor());
    for _ in 0..40 {
        w.tick(&idle());
    }
    assert!(w.player().on_ground);
    w.tick(&TickInput {
        tool_slot: Some(SWORD_SLOT),
        ..idle()
    });
    w
}

fn animal(w: &Walker, id: EntityId) -> Option<&AnimalEntity> {
    w.animals().get(id)
}

/// Aim at the middle of the animal's box, and walk toward it when it has
/// fled out of reach.
fn chase(w: &Walker, id: EntityId) -> TickInput {
    let a = animal(w, id).expect("animal alive");
    let at = [a.pos[0], a.pos[1] + a.box_size[1] / 2.0, a.pos[2]];
    let eye = w.eye(1.0);
    let d = [0, 1, 2].map(|i| at[i] - eye[i]);
    let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
    TickInput {
        yaw: d[0].atan2(-d[2]) as f32,
        pitch: d[1].atan2(flat) as f32,
        move_forward: if flat > 1.5 { 1.0 } else { 0.0 },
        ..idle()
    }
}

/// Tick aimed at the animal until it has settled into reach, with its
/// invulnerability window closed.
fn settle(w: &mut Walker, id: EntityId) {
    for _ in 0..400 {
        let input = chase(w, id);
        w.tick(&input);
        let a = animal(w, id).expect("animal alive");
        let eye = w.eye(1.0);
        let near = (a.pos[0] - eye[0]).hypot(a.pos[2] - eye[2]) < 1.5;
        if a.hurt_ticks_left == 0 && near && a.velocity[1] == 0.0 {
            return;
        }
    }
    panic!("never got a clean swing");
}

/// One click on the animal; returns its health afterwards (0 if gone).
fn click(w: &mut Walker, id: EntityId) -> u32 {
    let input = TickInput {
        primary: true,
        ..chase(w, id)
    };
    w.tick(&input);
    animal(w, id).map_or(0, |a| a.health)
}

/// Hit with the sword, one clean swing at a time, until the animal is dead;
/// returns how many hits it took.
fn hits_to_kill(species: &str) -> u32 {
    let mut w = start();
    let id = w.spawn_animal(species, [0.5, 4.0, -1.5], 0.0).unwrap();
    let mut hits = 0;
    loop {
        settle(&mut w, id);
        let before = animal(&w, id).unwrap().health;
        let after = click(&mut w, id);
        assert!(after < before, "a clean swing lands");
        hits += 1;
        if animal(&w, id).unwrap().death_ticks_left.is_some() {
            return hits;
        }
        assert!(hits < 10);
    }
}

#[test]
fn wooden_sword_kills_cow_and_pig_in_three_and_chicken_in_one() {
    assert_eq!(hits_to_kill("cow"), 3);
    assert_eq!(hits_to_kill("pig"), 3);
    assert_eq!(hits_to_kill("chicken"), 1);
}

#[test]
fn hits_inside_the_invulnerability_window_are_ignored() {
    let mut w = start();
    let id = w.spawn_animal("cow", [0.5, 4.0, -1.5], 0.0).unwrap();
    settle(&mut w, id);
    let full = animal(&w, id).unwrap().health;
    let first = click(&mut w, id);
    assert!(first < full);
    // Hit again on the next tick, and the one after: nothing lands.
    for _ in 0..2 {
        assert_eq!(click(&mut w, id), first);
    }
    assert!(animal(&w, id).unwrap().hurt_ticks_left > 0);
    // Once the window has closed a hit lands again.
    settle(&mut w, id);
    assert!(click(&mut w, id) < first);
}

#[test]
fn one_click_produces_one_attack_even_between_ticks() {
    let mut w = start();
    let id = w.spawn_animal("cow", [0.5, 4.0, -1.5], 0.0).unwrap();
    settle(&mut w, id);
    let full = animal(&w, id).unwrap().health;

    // The click lands between ticks, with the input accumulator in front.
    let mut acc = InputAccumulator::new();
    let aim = chase(&w, id);
    acc.set_look(aim.yaw, aim.pitch);
    acc.press_primary();
    let mut input = acc.drain();
    input.move_forward = aim.move_forward;
    w.tick(&input);
    let after_one = animal(&w, id).unwrap().health;
    assert!(after_one < full, "the click attacks");

    // The press was consumed: draining again gives no second attack, and
    // after the window closes the animal is still at the same health.
    for _ in 0..WINDOW {
        let aim = chase(&w, id);
        acc.set_look(aim.yaw, aim.pitch);
        let input = acc.drain();
        assert!(!input.primary);
        w.tick(&TickInput {
            move_forward: aim.move_forward,
            ..input
        });
    }
    assert_eq!(animal(&w, id).unwrap().health, after_one);
}

#[test]
fn killed_animal_drops_can_be_picked_up_and_carried() {
    let mut w = start();
    let id = w.spawn_animal("chicken", [0.5, 4.0, -1.5], 0.0).unwrap();
    settle(&mut w, id);
    click(&mut w, id);
    assert!(animal(&w, id).unwrap().death_ticks_left.is_some());
    for _ in 0..200 {
        w.tick(&idle());
    }
    assert!(animal(&w, id).is_none(), "the body is gone");
    let drops: Vec<ItemEntity> = w.items().iter().cloned().collect();
    assert!(!drops.is_empty(), "a chicken always drops raw chicken");
    assert!(drops.iter().all(|d| d.velocity.iter().all(|v| *v == 0.0)));

    // Walk up to it, then aim at it and pick it up.
    let target_pos = drops
        .iter()
        .find(|d| d.item.item == "raw_chicken")
        .expect("raw chicken")
        .pos;
    let aim_at = |w: &Walker| {
        let eye = w.eye(1.0);
        let at = [target_pos[0], target_pos[1] + 0.1, target_pos[2]];
        let d = [0, 1, 2].map(|i| at[i] - eye[i]);
        let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
        TickInput {
            yaw: d[0].atan2(-d[2]) as f32,
            pitch: d[1].atan2(flat) as f32,
            move_forward: if flat > 1.0 { 1.0 } else { 0.0 },
            ..idle()
        }
    };
    for _ in 0..100 {
        let aim = aim_at(&w);
        w.tick(&aim);
        if aim.move_forward == 0.0 {
            break;
        }
    }
    let aim = TickInput {
        move_forward: 0.0,
        ..aim_at(&w)
    };
    w.tick(&aim);
    w.tick(&TickInput {
        secondary: true,
        ..aim
    });
    assert_eq!(
        w.hands(),
        Some(&ItemInstance {
            item: "raw_chicken".into()
        })
    );
    assert_eq!(w.items().len(), drops.len() - 1);

    // Carried: it stays in the hands while the player walks.
    for _ in 0..20 {
        w.tick(&TickInput {
            move_forward: 1.0,
            ..aim
        });
    }
    assert!(w.hands().is_some());
}

/// Everything the script can change.
#[derive(Debug, PartialEq)]
struct Final {
    player: Player,
    animals: Vec<AnimalEntity>,
    items: Vec<ItemEntity>,
}

fn run_session() -> Final {
    let mut w = start();
    let ids = [
        w.spawn_animal("cow", [0.5, 4.0, -1.5], 0.0).unwrap(),
        w.spawn_animal("pig", [3.5, 4.0, -2.5], 0.0).unwrap(),
        w.spawn_animal("chicken", [-3.5, 4.0, -2.5], 0.0).unwrap(),
    ];
    // Let them wander, then hit the cow once and let it flee, then kill all.
    for _ in 0..60 {
        w.tick(&idle());
    }
    for id in ids {
        for _ in 0..4 {
            if animal(&w, id).is_none_or(|a| a.death_ticks_left.is_some()) {
                break;
            }
            settle(&mut w, id);
            click(&mut w, id);
        }
        // Wait out the death animation so the drops spawn.
        for _ in 0..30 {
            w.tick(&idle());
        }
    }
    for _ in 0..80 {
        w.tick(&idle());
    }
    Final {
        player: w.player().clone(),
        animals: w.animals().iter().cloned().collect(),
        items: w.items().iter().cloned().collect(),
    }
}

#[test]
fn identical_seeded_sessions_end_in_identical_state() {
    let a = run_session();
    assert!(a.animals.is_empty(), "all three were killed");
    assert!(!a.items.is_empty());
    assert_eq!(a, run_session());
}
