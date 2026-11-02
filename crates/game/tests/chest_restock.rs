//! Chests are topped up to the level's supplies at each gathering start:
//! docs/design/cooking-stations.md Phase C, sections 7.1 and 8.

use game::chests::LevelSupply;
use game::events::GameEvent;
use game::input::TickInput;
use game::phase::PhaseClock;
use game::scene::raft_scene;
use game::walk::Walker;

fn walker() -> Walker {
    Walker::in_raft_scene(raft_scene()).unwrap()
}

fn supply(item: &str, count: usize, chest: usize) -> LevelSupply {
    LevelSupply {
        item: item.into(),
        count,
        chest,
    }
}

fn restocks(w: &mut Walker) -> Vec<([i32; 3], Vec<String>)> {
    w.drain_events()
        .into_iter()
        .filter_map(|e| match e {
            GameEvent::ChestRestocked { chest, items } => Some((chest, items)),
            _ => None,
        })
        .collect()
}

fn count(w: &Walker, item: &str) -> usize {
    w.chest_items().iter().filter(|(_, i)| i == item).count()
}

/// Ticks per phase, to land on the first tick of a later phase.
fn phase_ticks() -> (u64, u64) {
    let c = PhaseClock::default_phases().unwrap();
    let g = (0..).find(|&t| c.at(t).0 != c.at(0).0).unwrap();
    let s = (g..).find(|&t| c.at(t).0 == c.at(0).0).unwrap();
    (g, s)
}

fn run(w: &mut Walker, ticks: u64) {
    for _ in 0..ticks {
        w.tick(&TickInput::default());
    }
}

#[test]
fn tick_zero_stocks_and_later_gathering_starts_top_up() {
    let (service, day) = phase_ticks();
    let mut w = walker();
    w.set_supplies(vec![supply("raw_beef", 3, 0), supply("leather", 2, 1)]);
    run(&mut w, 1);
    let first = restocks(&mut w);
    assert_eq!(first.len(), 2);
    assert_eq!(count(&w, "raw_beef"), 3);
    assert_eq!(count(&w, "leather"), 2);
    assert!(first.iter().all(|(_, items)| !items.is_empty()));

    // Nothing at the service start, and nothing while nothing is missing.
    run(&mut w, service);
    assert!(restocks(&mut w).is_empty());

    // A bigger list next day adds only the shortfall and keeps what is stored.
    let (cell, _) = w
        .chest_items()
        .into_iter()
        .find(|(_, i)| i == "raw_beef")
        .unwrap();
    w.set_supplies(vec![supply("raw_beef", 4, 0), supply("leather", 2, 1)]);
    run(&mut w, day - service);
    assert_eq!(restocks(&mut w), vec![(cell, vec!["raw_beef".to_string()])]);
    assert_eq!(count(&w, "raw_beef"), 4);
    assert_eq!(count(&w, "leather"), 2);
}

#[test]
fn two_walkers_agree_on_the_world_hash() {
    let (_, day) = phase_ticks();
    let (mut a, mut b) = (walker(), walker());
    for w in [&mut a, &mut b] {
        w.set_supplies(vec![supply("raw_beef", 3, 0)]);
    }
    run(&mut a, day + 5);
    run(&mut b, day + 5);
    assert_eq!(a.world_hash(), b.world_hash());
    assert!(count(&a, "raw_beef") > 0, "the supplies are stocked");
}
