//! Per-tick fish swimming: wander, loose schooling, depth clearance and
//! scatter from a swimming player. See docs/design/fish-and-fishing.md
//! section 3.2. A fish only moves where its whole box is in water and clear
//! of blocks and the raft. All randomness comes from the sim `Rng`.

use std::collections::BTreeMap;

use crate::clock::TICK_SECONDS;
use crate::collision::{Aabb, CollisionWorld};
use crate::fish_entities::{FishStore, SchoolId};
use crate::fish_spawn_config::FishSpawnConfig;
use crate::rng::Rng;
use crate::targeting::EntityId;

/// Horizontal distance (blocks) from the school centre at which the pull
/// towards it equals the wander; nearer, the wander wins.
pub const SCHOOL_RADIUS: f64 = 4.0;

/// How far (blocks) up and down a water column is scanned for its surface
/// and bottom.
const SCAN: i32 = 128;

/// The swim state of every fish, beside the `FishStore`.
#[derive(Clone, Debug)]
pub struct FishSteer {
    config: FishSpawnConfig,
    /// Per fish: the wander heading it turns towards, and the ticks before
    /// it picks a new heading and depth.
    wander: BTreeMap<EntityId, (f64, u32)>,
}

/// Unit horizontal direction of a heading in degrees (0 looks along -z).
fn dir(heading: f64) -> [f64; 2] {
    let (sin, cos) = heading.to_radians().sin_cos();
    [sin, -cos]
}

fn heading_of(dx: f64, dz: f64) -> f64 {
    dx.atan2(-dz).to_degrees().rem_euclid(360.0)
}

fn is_water<W: CollisionWorld + ?Sized>(world: &W, x: i32, y: i32, z: i32) -> bool {
    world.block_at(x, y, z).info().is_water()
}

/// The water of column `x, z` through cell `y`, as `[bottom, top)` heights,
/// or `None` if that cell is not water.
fn water_span<W: CollisionWorld + ?Sized>(world: &W, x: i32, y: i32, z: i32) -> Option<[f64; 2]> {
    if !is_water(world, x, y, z) {
        return None;
    }
    let (mut lo, mut hi) = (y, y);
    while lo > y - SCAN && is_water(world, x, lo - 1, z) {
        lo -= 1;
    }
    while hi < y + SCAN && is_water(world, x, hi + 1, z) {
        hi += 1;
    }
    Some([lo as f64, hi as f64 + 1.0])
}

/// The range of box-bottom heights for a box of `size` at `pos` (x and z;
/// y picks the water layer): `clearance` below the surface and above the
/// bottom of every column it spans, less where the water is too shallow.
/// `None` if any column is not water there or the box does not fit.
fn allowed_y<W: CollisionWorld + ?Sized>(
    world: &W,
    pos: [f64; 3],
    size: [f64; 2],
    clearance: f64,
) -> Option<[f64; 2]> {
    let h = size[0] / 2.0;
    let cells = |c: f64| (c - h).floor() as i32..=(c + h).ceil() as i32 - 1;
    let y = (pos[1] + size[1] / 2.0).floor() as i32;
    let [mut bottom, mut top] = [f64::MIN, f64::MAX];
    for x in cells(pos[0]) {
        for z in cells(pos[2]) {
            let [b, t] = water_span(world, x, y, z)?;
            bottom = bottom.max(b);
            top = top.min(t);
        }
    }
    let room = top - bottom - size[1];
    if room <= 0.0 {
        return None;
    }
    let c = clearance.min(room / 2.0);
    Some([bottom + c, top - c - size[1]])
}

/// `pos` with its height clamped into the allowed range, if the box fits
/// there clear of blocks and the raft.
fn place<W: CollisionWorld + ?Sized>(
    world: &W,
    pos: [f64; 3],
    size: [f64; 2],
    clearance: f64,
) -> Option<[f64; 3]> {
    let [lo, hi] = allowed_y(world, pos, size, clearance)?;
    let p = [pos[0], pos[1].clamp(lo, hi), pos[2]];
    let h = size[0] / 2.0;
    let bb = Aabb::from_size([p[0] - h, p[1], p[2] - h], [size[0], size[1], size[0]]);
    let mut boxes = Vec::new();
    world.boxes_in(&bb, &mut boxes);
    boxes.is_empty().then_some(p)
}

impl FishSteer {
    pub fn new(config: FishSpawnConfig) -> FishSteer {
        FishSteer {
            config,
            wander: BTreeMap::new(),
        }
    }

    /// Advance every fish one tick. `swimmer` is the position of a player
    /// who is in the water, if any; fish within the scatter distance swim
    /// away from it.
    pub fn tick<W: CollisionWorld + ?Sized>(
        &mut self,
        store: &mut FishStore,
        world: &W,
        swimmer: Option<[f64; 3]>,
        rng: &mut Rng,
    ) {
        self.wander.retain(|id, _| store.get(*id).is_some());
        let mut centres: BTreeMap<SchoolId, [f64; 3]> = BTreeMap::new();
        for f in store.iter() {
            let c = centres.entry(f.school).or_default();
            *c = [c[0] + f.pos[0], c[1] + f.pos[2], c[2] + 1.0];
        }
        let bodies = store.bodies().clone();
        let c = &self.config;
        for f in store.iter_mut() {
            let body = bodies.get(&f.species).expect("species checked at spawn");
            let (heading, ticks_left) = self.wander.entry(f.id).or_insert((f.heading, 0));
            if *ticks_left == 0 {
                *heading = f64::from(rng.angle()).to_degrees();
                let [lo, hi] = c.retarget_ticks;
                *ticks_left = rng.range(lo as i32, hi as i32) as u32;
                if let Some([lo, hi]) = allowed_y(world, f.pos, f.box_size, c.clearance) {
                    let unit = f64::from(rng.next_u32()) / 4_294_967_296.0;
                    f.depth_target = lo + (hi - lo) * unit;
                }
            }
            *ticks_left -= 1;

            let mut speed = body.swim_speed;
            let away = swimmer.filter(|p| {
                let d = [0, 1, 2].map(|i| f.pos[i] - p[i]);
                d[0].hypot(d[1]).hypot(d[2]) < c.scatter_distance
            });
            let goal = if let Some(p) = away {
                speed *= c.scatter_speed;
                let (dx, dz) = (f.pos[0] - p[0], f.pos[2] - p[2]);
                if dx.hypot(dz) > 1e-9 {
                    *heading = heading_of(dx, dz);
                }
                *heading
            } else {
                let [sx, sz, n] = centres[&f.school];
                let [gx, gz] = dir(*heading);
                let pull = |s: f64, p: f64| (s / n - p) / SCHOOL_RADIUS;
                heading_of(gx + pull(sx, f.pos[0]), gz + pull(sz, f.pos[2]))
            };
            let turn = (goal - f.heading + 180.0).rem_euclid(360.0) - 180.0;
            let max_turn = body.turn_rate * TICK_SECONDS;
            f.heading = (f.heading + turn.clamp(-max_turn, max_turn)).rem_euclid(360.0);

            let step = speed * TICK_SECONDS;
            let [mx, mz] = dir(f.heading);
            let y = f.pos[1] + (f.depth_target - f.pos[1]).clamp(-step, step);
            let ahead = [f.pos[0] + mx * step, y, f.pos[2] + mz * step];
            let size = f.box_size;
            if let Some(p) = place(world, ahead, size, c.clearance) {
                f.pos = p;
            } else {
                // Blocked ahead: rise or sink in place and turn back.
                *heading = (f.heading + 180.0).rem_euclid(360.0);
                if let Some(p) = place(world, [f.pos[0], y, f.pos[2]], size, c.clearance) {
                    f.pos = p;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::fish_bodies::FishBodies;
    use crate::raft::{Raft, RaftWorld};
    use terrain::Block;

    /// Water from y 51 to 58 over a stone floor at 50, inside stone walls,
    /// for x and z in `-half..half`.
    fn pool(half: i32) -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -half - 1..=half {
            for z in -half - 1..=half {
                let wall = x < -half || x >= half || z < -half || z >= half;
                for y in 50..=58 {
                    let b = if y == 50 || wall {
                        Block::Stone
                    } else {
                        Block::Water
                    };
                    w.set(x, y, z, b);
                }
            }
        }
        w
    }

    fn setup(spawns: &[(&str, [f64; 3], SchoolId)]) -> (FishStore, FishSteer) {
        let mut store = FishStore::new(FishBodies::default_from_data().unwrap(), 0);
        for &(sp, pos, school) in spawns {
            store.spawn(sp, pos, 0.0, school).unwrap();
        }
        let steer = FishSteer::new(FishSpawnConfig::default_from_data().unwrap());
        (store, steer)
    }

    fn two_schools() -> Vec<(&'static str, [f64; 3], SchoolId)> {
        let mut v = Vec::new();
        for i in 0..5 {
            let i = i as f64;
            v.push(("mackerel", [-4.0 + i, 54.0, -3.0], 0));
            v.push(("tuna", [3.0, 53.0 + i / 2.0, 2.0 + i], 1));
        }
        v.push(("salmon", [-5.5, 57.5, 5.5], 2));
        v
    }

    fn positions(store: &FishStore) -> Vec<[f64; 3]> {
        store.iter().map(|f| f.pos).collect()
    }

    #[test]
    fn fish_stay_in_water_and_keep_clearance() {
        let mut world = pool(8);
        // A pillar and a shelf to swim around.
        for y in 51..=58 {
            world.set(0, y, 0, Block::Stone);
        }
        for x in 4..8 {
            for z in -8..8 {
                world.set(x, 51, z, Block::Stone);
            }
        }
        let raft = Raft::new(&[[-2, 58, -2], [-1, 58, -2], [-2, 58, -1]]);
        let rw = RaftWorld::new(&world, &raft);
        let (mut store, mut steer) = setup(&two_schools());
        let mut rng = Rng::new(3);
        for t in 0..4000 {
            steer.tick(&mut store, &rw, None, &mut rng);
            for f in store.iter() {
                let bb = f.aabb();
                let lo = |i: usize| bb.min[i].floor() as i32;
                let hi = |i: usize| bb.max[i].ceil() as i32 - 1;
                for x in lo(0)..=hi(0) {
                    for z in lo(2)..=hi(2) {
                        // Every cell from 1 below to 1 above the box is water.
                        for y in lo(1) - 1..=hi(1) + 1 {
                            let ok = is_water(&rw, x, y, z);
                            assert!(ok, "tick {t}: fish {} at {:?}", f.id, f.pos);
                        }
                    }
                }
                let mut boxes = Vec::new();
                rw.boxes_in(&bb, &mut boxes);
                assert!(boxes.is_empty(), "tick {t}: fish {} in a box", f.id);
                assert!(bb.min[1] >= 52.0 - 1e-9 && bb.max[1] <= 58.0 + 1e-9);
            }
        }
    }

    #[test]
    fn shallow_water_keeps_fish_off_the_bottom() {
        let mut world = MemoryWorld::new();
        for x in -6..6 {
            for z in -6..6 {
                world.set(x, 60, z, Block::Stone);
                world.set(x, 61, z, Block::Water);
            }
        }
        let (mut store, mut steer) = setup(&[("mackerel", [0.5, 61.0, 0.5], 0)]);
        let mut rng = Rng::new(9);
        for _ in 0..1000 {
            steer.tick(&mut store, &world, None, &mut rng);
            let f = store.iter().next().unwrap();
            // Centred in the single water layer, never on the floor.
            assert!((f.pos[1] - 61.35).abs() < 1e-9, "{:?}", f.pos);
        }
    }

    #[test]
    fn a_nearby_swimmer_scatters_fish() {
        let world = pool(16);
        let start = [0.5, 54.0, 0.5];
        let player = [2.5, 54.0, 0.5];
        for seed in 0..10 {
            let (mut store, mut steer) = setup(&[("mackerel", start, 0)]);
            let mut rng = Rng::new(seed);
            for _ in 0..40 {
                steer.tick(&mut store, &world, Some(player), &mut rng);
            }
            let p = store.iter().next().unwrap().pos;
            let d = (p[0] - player[0]).hypot(p[2] - player[2]);
            assert!(d > 3.0, "seed {seed}: {p:?} is {d} from the player");
        }
        // A swimmer out of range changes nothing.
        let run = |swimmer| {
            let (mut store, mut steer) = setup(&[("mackerel", start, 0)]);
            let mut rng = Rng::new(1);
            for _ in 0..40 {
                steer.tick(&mut store, &world, swimmer, &mut rng);
            }
            positions(&store)
        };
        assert_eq!(run(None), run(Some([12.5, 54.0, 0.5])));
    }

    #[test]
    fn schools_stay_together() {
        let world = pool(24);
        let mut spawns = Vec::new();
        for i in 0..6 {
            let x = -15.0 + 6.0 * i as f64;
            spawns.push(("mackerel", [x, 54.0, (i % 2) as f64 * 8.0 - 4.0], 0));
        }
        let (mut store, mut steer) = setup(&spawns);
        let mut rng = Rng::new(5);
        let spread = |s: &FishStore| {
            let p = positions(s);
            let n = p.len() as f64;
            let cx = p.iter().map(|q| q[0]).sum::<f64>() / n;
            let cz = p.iter().map(|q| q[2]).sum::<f64>() / n;
            let far = p.iter().map(|q| (q[0] - cx).hypot(q[2] - cz));
            far.fold(0.0, f64::max)
        };
        assert!(spread(&store) > 12.0);
        for t in 0..3000 {
            steer.tick(&mut store, &world, None, &mut rng);
            if t >= 400 {
                assert!(spread(&store) < 8.0,"tick {t}: {}", spread(&store));
            }
        }
    }

    #[test]
    fn same_seed_and_input_give_same_positions() {
        let world = pool(10);
        let run = |seed| {
            let (mut store, mut steer) = setup(&two_schools());
            let mut rng = Rng::new(seed);
            let mut out = Vec::new();
            for t in 0..600 {
                let swimmer = (t > 300).then_some([0.5, 55.0, 0.5]);
                steer.tick(&mut store, &world, swimmer, &mut rng);
                if t == 150 {
                    store.remove(3);
                }
                out.push(positions(&store));
            }
            out
        };
        assert_eq!(run(11), run(11));
        assert_ne!(run(11), run(12));
    }
}
