//! A walker owns a `FishWorld` and ticks it: Phase C of
//! docs/design/fish-and-fishing.md (sections 3.1 and 3.2), through
//! `Walker::tick`.

use game::input::TickInput;
use game::stream::{StreamConfig, Streamer};
use game::walk::Walker;
use layout::Layout;

const SEED: u64 = 25;
/// Ticks per phase (120 s at 20 ticks a second).
const PHASE_TICKS: u64 = 2400;

fn layout() -> Layout {
    Layout {
        seed: SEED,
        ..Layout::default()
    }
}

/// The deepest water column near the middle of region (0, 0).
fn water_column() -> (i32, i32) {
    let mut streamer = Streamer::new(layout(), StreamConfig::default_from_data().unwrap());
    let mut regions = game::walk::RegionSet::default();
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
    let (_, x, z) = best.expect("no water near the middle of region (0, 0)");
    (x, z)
}

fn walker() -> Walker {
    let (x, z) = water_column();
    Walker::with_layout(layout(), [f64::from(x) + 0.5, 80.0, f64::from(z) + 0.5]).unwrap()
}

fn positions(w: &Walker) -> Vec<(u32, [f64; 3])> {
    w.fish().iter().map(|f| (f.id, f.pos)).collect()
}

#[test]
fn fish_appear_on_a_phase_change_and_not_mid_phase() {
    let mut w = walker();
    let input = TickInput::default();
    let mut seen: Vec<u32> = Vec::new();
    let mut spawn_ticks = Vec::new();
    for _ in 0..3 * 4800 {
        w.tick(&input);
        let new: Vec<_> = w.fish().iter().filter(|f| !seen.contains(&f.id)).collect();
        if !new.is_empty() {
            spawn_ticks.push(w.sim_tick());
            seen.extend(new.iter().map(|f| f.id));
        }
    }
    assert!(!spawn_ticks.is_empty(), "no fish in three days");
    for t in &spawn_ticks {
        assert_eq!(t % PHASE_TICKS, 0, "fish spawned mid-phase at tick {t}");
    }
}

#[test]
fn spawn_fish_checks_the_species_and_ids_are_distinct() {
    let mut w = walker();
    let pos = [0.5, 40.0, 0.5];
    assert!(w.spawn_fish("nope", pos, 0.0).is_none());
    let fish = w
        .spawn_fish("mackerel", pos, 0.0)
        .expect("a shipped species spawns");
    let animal = w.spawn_animal("cow", [0.5, 4.0, -2.0], 0.0).unwrap();
    assert_ne!(fish, animal);
    assert!(fish > animal, "fish ids sit above animal ids");
    assert!(w.fish().get(fish).is_some());
}

#[test]
fn same_seed_and_input_agree_and_fish_change_the_hash() {
    let (mut a, mut b) = (walker(), walker());
    let input = TickInput::default();
    for _ in 0..3 * 4800 {
        a.tick(&input);
        b.tick(&input);
    }
    assert!(!a.fish().is_empty());
    assert_eq!(positions(&a), positions(&b));
    assert_eq!(a.world_hash(), b.world_hash());
    let before = a.world_hash();
    a.spawn_fish("mackerel", [0.5, 40.0, 0.5], 0.0).unwrap();
    assert_ne!(a.world_hash(), before);
}

#[test]
fn a_walker_without_a_streamer_spawns_no_fish() {
    let mut w = Walker::new([0.5, 4.0, 0.5]).unwrap();
    let input = TickInput::default();
    for _ in 0..PHASE_TICKS + 10 {
        w.tick(&input);
    }
    assert!(w.fish().is_empty());
}
