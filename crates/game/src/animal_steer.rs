//! Steering for a swimming animal (docs/design/raft-scene-feedback.md Phase C,
//! decided in #714): head for the nearest solid, non-water support cell within
//! a radius. The raft counts as support through `RaftWorld`'s boxes.

use crate::collision::{Aabb, CollisionWorld};

/// How far (in blocks, horizontally) a swimming animal looks for support.
pub const SEARCH_RADIUS: f64 = 8.0;

/// Support cells are searched from this many blocks below the animal's feet
/// to this many above them, so a shore step or a raft deck counts.
const BELOW: i32 = 1;
const ABOVE: i32 = 1;

/// The facing in degrees (0 looks along -z, as in the animal tick) towards the
/// nearest support box centre within `radius` of `pos`, or `None`. Ties break
/// by lowest box centre x, then z, then y, so the result is deterministic.
pub fn steer_to_support<W: CollisionWorld + ?Sized>(
    world: &W,
    pos: [f64; 3],
    radius: f64,
) -> Option<f64> {
    let feet = pos[1].floor();
    let query = Aabb::new(
        [pos[0] - radius, feet - BELOW as f64, pos[2] - radius],
        [pos[0] + radius, feet + 1.0 + ABOVE as f64, pos[2] + radius],
    );
    let mut boxes = Vec::new();
    world.boxes_in(&query, &mut boxes);
    let mut best: Option<([f64; 3], f64)> = None;
    for b in &boxes {
        let c = [0, 1, 2].map(|i| (b.min[i] + b.max[i]) / 2.0);
        let d = ((c[0] - pos[0]).powi(2) + (c[2] - pos[2]).powi(2)).sqrt();
        if d > radius {
            continue;
        }
        let better = match best {
            None => true,
            Some((bc, bd)) => d
                .total_cmp(&bd)
                .then(c[0].total_cmp(&bc[0]))
                .then(c[2].total_cmp(&bc[2]))
                .then(c[1].total_cmp(&bc[1]))
                .is_lt(),
        };
        if better {
            best = Some((c, d));
        }
    }
    let (c, _) = best?;
    let (dx, dz) = (c[0] - pos[0], c[2] - pos[2]);
    Some(dx.atan2(-dz).to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;
    use crate::raft::{Raft, RaftWorld};
    use terrain::Block;

    fn sea() -> MemoryWorld {
        let mut w = MemoryWorld::new();
        for x in -12..12 {
            for z in -12..12 {
                w.set(x, 63, z, Block::Water);
                w.set(x, 64, z, Block::Water);
            }
        }
        w
    }

    const P: [f64; 3] = [0.5, 64.0, 0.5];

    #[test]
    fn heads_for_the_only_solid_cell() {
        let mut w = sea();
        w.set(5, 64, 0, Block::Stone); // due +x
        let f = steer_to_support(&w, P, SEARCH_RADIUS).unwrap();
        assert!((f - 90.0).abs() < 1e-9, "{f}");
        w.set(5, 64, 0, Block::Air);
        w.set(0, 64, -4, Block::Stone); // due -z
        let f = steer_to_support(&w, P, SEARCH_RADIUS).unwrap();
        assert!(f.abs() < 1e-9, "{f}");
    }

    #[test]
    fn nearer_wins_and_ties_are_fixed() {
        let mut w = sea();
        w.set(6, 64, 0, Block::Stone);
        w.set(-3, 64, 0, Block::Stone); // centre 3.0 away, nearer
        let f = steer_to_support(&w, P, SEARCH_RADIUS).unwrap();
        assert!((f + 90.0).abs() < 1e-9, "{f}");
        // Equidistant: the lower x wins.
        let mut w = sea();
        w.set(4, 64, 0, Block::Stone);
        w.set(-4, 64, 0, Block::Stone);
        let f = steer_to_support(&w, [0.5 + 0.0, 64.0, 0.5], SEARCH_RADIUS).unwrap();
        let again = steer_to_support(&w, P, SEARCH_RADIUS).unwrap();
        assert_eq!(f, again);
        assert!((f + 90.0).abs() < 1e-9, "{f}");
    }

    #[test]
    fn none_in_range_gives_none() {
        let mut w = sea();
        assert_eq!(steer_to_support(&w, P, SEARCH_RADIUS), None);
        w.set(10, 64, 0, Block::Stone);
        assert_eq!(steer_to_support(&w, P, SEARCH_RADIUS), None);
        assert!(steer_to_support(&w, P, 11.0).is_some());
    }

    #[test]
    fn water_and_air_are_never_targets() {
        let w = sea();
        assert_eq!(steer_to_support(&w, P, SEARCH_RADIUS), None);
        assert_eq!(
            steer_to_support(&MemoryWorld::new(), P, SEARCH_RADIUS),
            None
        );
    }

    #[test]
    fn a_raft_cell_counts_as_support() {
        let w = sea();
        let raft = Raft::new(&[[4, 64, 0], [5, 64, 0]]);
        let world = RaftWorld::new(&w, &raft);
        let f = steer_to_support(&world, P, SEARCH_RADIUS).unwrap();
        assert!((f - 90.0).abs() < 1e-9, "{f}");
    }
}
