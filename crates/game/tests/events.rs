//! Headless "every action produces its event" check: the measurable form of
//! the Phase 8 done-when in docs/design/basic-gameplay.md (sections 11 and
//! 14).
//!
//! Scripted sessions drive a `Walker` and collect every drained event: on
//! the raft scene (water, raft, a fall, swings) and on a flat grass floor
//! (animal hit and kill, drop and pickup, forage, till, plant, harvest).
//! There is one test per event kind that exists (`item_put_down` is not
//! emitted until stations exist, decided in #471), and each session replays
//! to identical events and world hash.

use game::events::GameEvent;
use game::input::TickInput;
use game::items::ItemInstance;
use game::scene::raft_scene;
use game::walk::Walker;
use terrain::{Block, Region, HEIGHT};

/// Top layer of the floor; the player stands at `y = TOP + 1`.
const TOP: i32 = 3;
const SIZE: u32 = 32;
const HALF: i32 = 16;
const EMPTY_SLOT: u8 = 0;
const SWORD_SLOT: u8 = 1;
const HOE_SLOT: u8 = 2;
/// Ticks in one 4-minute day at 20 Hz.
const DAY: u64 = 4800;
const FIELD: [i32; 2] = [0, 0];
const WATER: [i32; 2] = [2, 0];
const TALL_GRASS: [i32; 2] = [6, 6];

fn p() -> String {
    "player".to_string()
}

fn set(r: &mut Region, x: i32, y: i32, z: i32, block: Block) {
    let i = r.index((x + HALF) as u32, y as u32, (z + HALF) as u32);
    r.blocks[i] = block as u8;
}

/// Stone below a grass top, with water beside the field and tall grass to forage.
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

fn down() -> TickInput {
    TickInput {
        pitch: -std::f32::consts::FRAC_PI_2,
        ..idle()
    }
}

/// A walker plus every event it has emitted, in order.
struct Session {
    w: Walker,
    events: Vec<GameEvent>,
    /// Every event, including those already taken.
    log: Vec<GameEvent>,
}

impl Session {
    fn on_floor() -> Session {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.add_region(floor());
        let mut s = Session {
            w,
            events: vec![],
            log: vec![],
        };
        s.ticks(&idle(), 40);
        assert!(s.w.player().on_ground);
        s
    }

    fn on_raft() -> Session {
        let w = Walker::in_raft_scene(raft_scene()).unwrap();
        Session {
            w,
            events: vec![],
            log: vec![],
        }
    }

    fn tick(&mut self, input: &TickInput) {
        self.w.tick(input);
        let new = self.w.drain_events();
        self.log.extend(new.iter().cloned());
        self.events.extend(new);
    }

    fn ticks(&mut self, input: &TickInput, n: usize) {
        for _ in 0..n {
            self.tick(input);
        }
    }

    /// The events since the last call.
    fn take(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    fn select(&mut self, slot: u8) {
        self.tick(&TickInput {
            tool_slot: Some(slot),
            ..idle()
        });
    }

    /// Stand on the column `(x, z)` and let the aim settle.
    fn stand(&mut self, [x, z]: [i32; 2]) {
        self.w
            .teleport([f64::from(x) + 0.5, f64::from(TOP) + 1.0, f64::from(z) + 0.5]);
        self.tick(&down());
    }

    /// Aim at `at` from the eye.
    fn aim(&self, at: [f64; 3]) -> TickInput {
        let eye = self.w.eye(1.0);
        let d = [0, 1, 2].map(|i| at[i] - eye[i]);
        TickInput {
            yaw: d[0].atan2(-d[2]) as f32,
            pitch: d[1].atan2(d[0].hypot(d[2])) as f32,
            ..idle()
        }
    }

    /// Put `item` on the ground at `pos`, wait for it to settle, and pick
    /// it up.
    fn pick_up(&mut self, item: &str, pos: [f64; 3]) {
        self.w.clear_items();
        self.w.spawn_item(ItemInstance { item: item.into() }, pos);
        self.ticks(&idle(), 40);
        let at = self.w.items().iter().next().expect("the item").pos;
        let aim = self.aim([at[0], at[1] + 0.1, at[2]]);
        self.tick(&aim);
        self.tick(&TickInput {
            interact: true,
            ..aim
        });
        assert_eq!(self.w.hands().map(|h| h.item.as_str()), Some(item));
    }

    /// Aim at the middle of the animal's box and walk to it if far.
    fn chase(&self, id: u32) -> TickInput {
        let a = self.w.animals().get(id).expect("animal alive");
        let eye = self.w.eye(1.0);
        let flat = (a.pos[0] - eye[0]).hypot(a.pos[2] - eye[2]);
        TickInput {
            move_forward: if flat > 1.5 { 1.0 } else { 0.0 },
            ..self.aim([a.pos[0], a.pos[1] + a.box_size[1] / 2.0, a.pos[2]])
        }
    }

    /// One clean sword click on the animal.
    fn hit(&mut self, id: u32) {
        for _ in 0..400 {
            let input = self.chase(id);
            self.tick(&input);
            let a = self.w.animals().get(id).expect("animal alive");
            let eye = self.w.eye(1.0);
            let near = (a.pos[0] - eye[0]).hypot(a.pos[2] - eye[2]) < 1.5;
            if a.hurt_ticks_left == 0 && near && a.velocity[1] == 0.0 {
                break;
            }
        }
        let input = TickInput {
            primary: true,
            ..self.chase(id)
        };
        self.tick(&input);
    }
}

fn swing(tool: Option<&str>) -> GameEvent {
    GameEvent::Swing {
        player: p(),
        tool: tool.map(String::from),
    }
}

fn is_kind(e: &GameEvent, kind: &str) -> bool {
    let name = format!("{e:?}");
    name.split([' ', '(']).next() == Some(kind)
}

fn only(events: &[GameEvent], kind: &str) -> Vec<GameEvent> {
    events
        .iter()
        .filter(|e| is_kind(e, kind))
        .cloned()
        .collect()
}

fn press() -> TickInput {
    TickInput {
        secondary: true,
        ..down()
    }
}

#[test]
fn swing_fires_on_every_primary_press_at_nothing_or_a_block_and_not_otherwise() {
    let mut s = Session::on_floor();
    s.take();
    let sky = TickInput {
        primary: true,
        pitch: 1.2,
        ..idle()
    };
    s.tick(&sky);
    s.tick(&TickInput {
        primary: true,
        ..down()
    });
    assert_eq!(s.take(), vec![swing(None), swing(None)]);
    // No press, and a secondary press, emit no swing.
    s.tick(&idle());
    s.tick(&press());
    assert!(only(&s.take(), "Swing").is_empty());
    // The sword names itself.
    s.select(SWORD_SLOT);
    s.take();
    s.tick(&sky);
    assert_eq!(s.take(), vec![swing(Some("sword"))]);
}

#[test]
fn hitting_and_killing_a_chicken_emits_swing_hurt_killed_then_the_drop() {
    let mut s = Session::on_floor();
    s.select(SWORD_SLOT);
    let id = s.w.spawn_animal("chicken", [0.5, 4.0, -1.5], 0.0).unwrap();
    s.take();
    s.hit(id);
    // Walking and aiming at it emitted nothing else; the click is ordered
    // swing, then hurt, then killed (a chicken has one hit's health).
    let hit = s.take();
    let [GameEvent::Swing { .. }, GameEvent::EntityHurt {
        entity,
        attacker,
        damage,
    }, GameEvent::EntityKilled {
        entity: killed,
        kind,
        attacker: killer,
    }] = hit.as_slice()
    else {
        panic!("{hit:?}");
    };
    assert_eq!((*entity, *killed), (id, id));
    assert_eq!((attacker.as_str(), killer.as_str()), ("player", "player"));
    assert_eq!(kind, "chicken");
    assert!(*damage > 0);
    assert_eq!(hit[0], swing(Some("sword")));

    // The body goes, and the drops follow, from the animal.
    s.ticks(&idle(), 200);
    let drops = only(&s.take(), "ItemDropped");
    assert!(!drops.is_empty());
    for d in &drops {
        let GameEvent::ItemDropped { source, .. } = d else {
            unreachable!()
        };
        assert_eq!(source, "animal");
    }
    assert!(drops
        .iter()
        .any(|d| matches!(d, GameEvent::ItemDropped { item, .. } if item == "raw_chicken")));
}

#[test]
fn hurting_without_killing_emits_hurt_only() {
    let mut s = Session::on_floor();
    s.select(SWORD_SLOT);
    let id = s.w.spawn_animal("cow", [0.5, 4.0, -1.5], 0.0).unwrap();
    s.take();
    s.hit(id);
    let hit = s.take();
    assert_eq!(hit.len(), 2, "{hit:?}");
    assert_eq!(hit[0], swing(Some("sword")));
    assert!(matches!(hit[1], GameEvent::EntityHurt { entity, .. } if entity == id));
}

#[test]
fn pickup_then_drop_emit_item_picked_up_then_item_dropped_from_the_player() {
    let mut s = Session::on_floor();
    s.stand([0, -3]);
    s.take();
    s.pick_up("seeds", [0.5, 4.5, -1.5]);
    assert_eq!(
        s.take(),
        vec![GameEvent::ItemPickedUp {
            item: "seeds".into(),
            player: p()
        }]
    );
    s.tick(&TickInput {
        drop: true,
        ..idle()
    });
    let dropped = s.take();
    let [GameEvent::ItemDropped {
        item,
        position,
        source,
    }] = dropped.as_slice()
    else {
        panic!("{dropped:?}");
    };
    assert_eq!((item.as_str(), source.as_str()), ("seeds", "player"));
    assert!(position[1] > 4.0, "thrown from the eye: {position:?}");
    // Dropping with empty hands emits nothing.
    s.tick(&TickInput {
        drop: true,
        ..idle()
    });
    assert!(s.take().is_empty());
}

#[test]
fn foraging_emits_foraged_with_the_yields_and_the_drops() {
    let mut s = Session::on_floor();
    s.select(EMPTY_SLOT);
    s.stand(TALL_GRASS);
    s.take();
    for _ in 0..60 {
        s.tick(&press());
        let got = s.take();
        if let Some(GameEvent::Foraged { position, yields }) = got.last() {
            assert_eq!(*position, [TALL_GRASS[0], TOP + 1, TALL_GRASS[1]]);
            assert!(!yields.is_empty());
            // Each yield is dropped first, from the forage.
            let drops = only(&got, "ItemDropped");
            assert_eq!(drops.len(), yields.len());
            assert!(drops
                .iter()
                .all(|d| matches!(d, GameEvent::ItemDropped { source, .. } if source == "forage")));
            return;
        }
        assert!(
            matches!(got.as_slice(), [GameEvent::ForageMissed { .. }]),
            "a miss emits only forage_missed: {got:?}"
        );
        // Wait out the one-minute cooldown.
        s.ticks(&idle(), 1200);
        s.take();
    }
    panic!("1 in 3 over 60 tries");
}

/// Till, plant, grow to maturity, and harvest; returns the events of each
/// step.
fn farm_session() -> (Session, [Vec<GameEvent>; 3]) {
    let mut s = Session::on_floor();
    s.select(HOE_SLOT);
    s.stand(FIELD);
    s.take();
    s.tick(&press());
    let tilled = s.take();

    s.select(EMPTY_SLOT);
    s.stand([0, -3]);
    s.pick_up("seeds", [0.5, 4.5, -1.5]);
    s.stand(FIELD);
    s.take();
    s.tick(&press());
    let planted = s.take();

    let crop = |s: &Session| s.w.block_at(FIELD[0], TOP + 1, FIELD[1]);
    let mut waited = 0;
    while crop(&s) != Block::Wheat7 {
        s.tick(&idle());
        waited += 1;
        assert!(waited <= 8 * DAY, "ready within eight days");
    }
    s.take();
    s.tick(&press());
    let harvested = s.take();
    (s, [tilled, planted, harvested])
}

#[test]
fn till_plant_and_harvest_each_emit_their_event() {
    let (_, [tilled, planted, harvested]) = farm_session();
    let pos = [FIELD[0], TOP, FIELD[1]];
    let [GameEvent::BlockTilled { position, from, to }] = tilled.as_slice() else {
        panic!("{tilled:?}");
    };
    assert_eq!((*position, from.as_str()), (pos, "grass"));
    assert!(to.starts_with("farmland"), "{to}");

    assert_eq!(
        planted,
        vec![GameEvent::CropPlanted {
            position: pos,
            crop: "wheat".into()
        }]
    );

    // The harvest event comes first, then one drop per yield from the
    // harvest.
    let GameEvent::CropHarvested {
        position,
        crop,
        yields,
    } = &harvested[0]
    else {
        panic!("{harvested:?}");
    };
    assert_eq!(
        (*position, crop.as_str()),
        ([pos[0], pos[1] + 1, pos[2]], "wheat")
    );
    assert!(yields.iter().any(|y| y == "wheat"));
    assert_eq!(harvested.len(), 1 + yields.len(), "{harvested:?}");
    assert!(harvested[1..]
        .iter()
        .all(|e| matches!(e, GameEvent::ItemDropped { source, .. } if source == "harvest")));
}

#[test]
fn a_fall_onto_the_raft_emits_one_landed_with_the_fall_distance() {
    let mut s = Session::on_raft();
    board(&mut s);
    s.take();
    let [x, y, z] = s.w.player().pos;
    s.w.teleport([x, y + 5.0, z]);
    s.ticks(&TickInput::default(), 60);
    let landed = only(&s.take(), "Landed");
    let [GameEvent::Landed { fall_distance }] = landed.as_slice() else {
        panic!("{landed:?}");
    };
    assert!((fall_distance - 5.0).abs() < 0.5, "{fall_distance}");
}

/// Swim at the raft holding jump until standing on it.
fn board(s: &mut Session) {
    let swim = TickInput {
        move_forward: 1.0,
        jump: true,
        ..idle()
    };
    for _ in 0..300 {
        s.tick(&swim);
        if s.w.player().on_ground {
            return;
        }
    }
    panic!("never boarded: {:?}", s.w.player());
}

/// Board the raft, then walk back off into the sea.
fn raft_session() -> Session {
    let mut s = Session::on_raft();
    board(&mut s);
    s.tick(&TickInput {
        sneak: true,
        ..idle()
    });
    let back = TickInput {
        move_forward: -1.0,
        ..idle()
    };
    s.ticks(&back, 200);
    s
}

#[test]
fn boarding_and_leaving_the_raft_emit_the_water_and_raft_edges_in_order() {
    let s = raft_session();
    let edges: Vec<GameEvent> = s
        .log
        .iter()
        .filter(|e| !is_kind(e, "Landed"))
        .cloned()
        .collect();
    // In the water from the start, so the first edge is climbing out; each
    // edge alternates, none doubled.
    assert_eq!(edges.first(), Some(&GameEvent::ExitedWater { player: p() }));
    assert!(edges.contains(&GameEvent::SteppedOntoRaft { player: p() }));
    assert!(edges.contains(&GameEvent::SteppedOffRaft { player: p() }));
    assert_eq!(edges.last(), Some(&GameEvent::EnteredWater { player: p() }));
    let water: Vec<bool> = edges
        .iter()
        .filter_map(|e| match e {
            GameEvent::EnteredWater { .. } => Some(true),
            GameEvent::ExitedWater { .. } => Some(false),
            _ => None,
        })
        .collect();
    assert!(water.windows(2).all(|w| w[0] != w[1]), "{edges:?}");
    let raft: Vec<bool> = edges
        .iter()
        .filter_map(|e| match e {
            GameEvent::SteppedOntoRaft { .. } => Some(true),
            GameEvent::SteppedOffRaft { .. } => Some(false),
            _ => None,
        })
        .collect();
    assert!(raft.windows(2).all(|w| w[0] != w[1]), "{edges:?}");
    assert_eq!(raft.first(), Some(&true));
}

/// The whole floor script: swings, an animal kill, drop and pickup, forage,
/// till, plant, harvest.
fn floor_session() -> (Vec<GameEvent>, u64) {
    let (mut s, _) = farm_session();
    s.select(SWORD_SLOT);
    let id = s.w.spawn_animal("chicken", [0.5, 4.0, -1.5], 0.0).unwrap();
    s.hit(id);
    s.ticks(&idle(), 200);
    s.select(EMPTY_SLOT);
    s.stand(TALL_GRASS);
    for _ in 0..5 {
        s.tick(&press());
        s.ticks(&idle(), 1200);
    }
    (s.log, s.w.world_hash())
}

#[test]
fn recorded_sessions_replay_to_identical_events_and_world_hash() {
    let (a, hash) = floor_session();
    for kind in [
        "Swing",
        "EntityHurt",
        "EntityKilled",
        "ItemDropped",
        "ItemPickedUp",
        "BlockTilled",
        "CropPlanted",
        "CropHarvested",
    ] {
        assert!(a.iter().any(|e| is_kind(e, kind)), "no {kind} in {a:?}");
    }
    assert_eq!(floor_session(), (a, hash));

    let (r1, r2) = (raft_session(), raft_session());
    assert!(!r1.log.is_empty());
    assert_eq!(r1.log, r2.log);
    assert_eq!(r1.w.world_hash(), r2.w.world_hash());
}
