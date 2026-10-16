//! Targeting types, the ray-vs-box test and reach tunables
//! (basic-gameplay.md section 5). Tunables come from `data/targeting.toml`.

use crate::collision::{Aabb, CollisionWorld};
use std::fmt;

const DEFAULT_TARGETING: &str = include_str!("../../../data/targeting.toml");

/// Identifies an entity. Placeholder until the entity crate defines one.
pub type EntityId = u32;

/// A face of a block or box, named by the direction its normal points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    /// -y
    Bottom,
    /// +y
    Top,
    /// -z
    North,
    /// +z
    South,
    /// -x
    West,
    /// +x
    East,
}

/// What the player's eye ray currently points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Block { pos: [i32; 3], face: Face },
    Entity { id: EntityId },
    None,
}

/// Entry point of a ray into a box: distance along the ray (in units of the
/// direction's length) and the face crossed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub distance: f64,
    pub face: Face,
}

/// Slab test of the ray `origin + t * dir` (`t >= 0`) against `aabb`.
///
/// Returns the entry hit. Pass a unit `dir` to get `distance` in blocks.
/// A ray that only grazes an edge or corner counts as a hit. A ray whose
/// origin is inside the box has no entry point and returns `None`: a target
/// never sits on the contents of the eye's own cell. Boxes behind the origin
/// and misses also return `None`.
pub fn ray_vs_box(origin: [f64; 3], dir: [f64; 3], aabb: &Aabb) -> Option<RayHit> {
    let mut t_enter = f64::NEG_INFINITY;
    let mut t_exit = f64::INFINITY;
    let mut face = Face::Bottom;
    for a in 0..3 {
        if dir[a] == 0.0 {
            if origin[a] < aabb.min[a] || origin[a] > aabb.max[a] {
                return None;
            }
            continue;
        }
        let t_min = (aabb.min[a] - origin[a]) / dir[a];
        let t_max = (aabb.max[a] - origin[a]) / dir[a];
        // Entering through the min side when moving in +, else the max side.
        let (near, far, f) = if dir[a] > 0.0 {
            (t_min, t_max, [Face::West, Face::Bottom, Face::North][a])
        } else {
            (t_max, t_min, [Face::East, Face::Top, Face::South][a])
        };
        if near > t_enter {
            t_enter = near;
            face = f;
        }
        t_exit = t_exit.min(far);
    }
    if t_enter > t_exit || t_enter < 0.0 || !t_enter.is_finite() {
        return None;
    }
    Some(RayHit {
        distance: t_enter,
        face,
    })
}

/// Cast the ray `origin + t * dir` through the block selection boxes of
/// `world` and return the nearest hit within `reach` blocks as
/// `Target::Block`, with its distance. `dir` need not be unit length.
/// Returns `None` for a zero direction or when nothing is in reach. A box
/// containing the origin is never hit (see `ray_vs_box`).
pub fn cast_block(
    world: &impl CollisionWorld,
    origin: [f64; 3],
    dir: [f64; 3],
    reach: f64,
) -> Option<(Target, f64)> {
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if !len.is_finite() || len == 0.0 || !reach.is_finite() || reach < 0.0 {
        return None;
    }
    let d = [dir[0] / len, dir[1] / len, dir[2] / len];
    let end = [0, 1, 2].map(|i| origin[i] + d[i] * reach);
    let query = Aabb::new(
        [0, 1, 2].map(|i| origin[i].min(end[i])),
        [0, 1, 2].map(|i| origin[i].max(end[i])),
    );
    let mut boxes = Vec::new();
    world.selection_boxes_in(&query, &mut boxes);
    let mut best: Option<(Target, f64)> = None;
    for b in &boxes {
        if let Some(hit) = ray_vs_box(origin, d, &b.aabb) {
            if hit.distance <= reach && best.is_none_or(|(_, bd)| hit.distance < bd) {
                best = Some((
                    Target::Block {
                        pos: b.pos,
                        face: hit.face,
                    },
                    hit.distance,
                ));
            }
        }
    }
    best
}

/// Reach and box tunables from `data/targeting.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetingConfig {
    pub block_reach: f32,
    pub entity_reach: f32,
    pub entity_box_expansion: f32,
}

#[derive(Debug)]
pub enum TargetingError {
    Parse(String),
    Missing(String),
    Invalid(String),
}

impl fmt::Display for TargetingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetingError::Parse(e) => write!(f, "targeting.toml: {e}"),
            TargetingError::Missing(k) => write!(f, "targeting.toml: missing `{k}`"),
            TargetingError::Invalid(e) => write!(f, "targeting.toml: {e}"),
        }
    }
}

impl std::error::Error for TargetingError {}

impl TargetingConfig {
    /// The shipped defaults from `data/targeting.toml`.
    pub fn default_from_data() -> Result<TargetingConfig, TargetingError> {
        TargetingConfig::parse(DEFAULT_TARGETING)
    }

    /// Parse targeting text; every value is required and must be a finite,
    /// non-negative number.
    pub fn parse(text: &str) -> Result<TargetingConfig, TargetingError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| TargetingError::Parse(e.to_string()))?;
        let get = |table: &str, key: &str| -> Result<f32, TargetingError> {
            let path = format!("{table}.{key}");
            let value = root
                .get(table)
                .and_then(|t| t.get(key))
                .ok_or_else(|| TargetingError::Missing(path.clone()))?;
            let n = match value {
                toml::Value::Float(f) => *f as f32,
                toml::Value::Integer(i) => *i as f32,
                _ => return Err(TargetingError::Invalid(format!("{path} must be a number"))),
            };
            if !n.is_finite() || n < 0.0 {
                return Err(TargetingError::Invalid(format!(
                    "{path} must be a non-negative number"
                )));
            }
            Ok(n)
        };
        Ok(TargetingConfig {
            block_reach: get("reach", "block")?,
            entity_reach: get("reach", "entity")?,
            entity_box_expansion: get("entity", "box_expansion")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::MemoryWorld;

    fn unit() -> Aabb {
        Aabb::new([0.0; 3], [1.0; 3])
    }

    #[test]
    fn each_of_six_faces() {
        let cases = [
            ([-2.0, 0.5, 0.5], [1.0, 0.0, 0.0], Face::West),
            ([3.0, 0.5, 0.5], [-1.0, 0.0, 0.0], Face::East),
            ([0.5, -2.0, 0.5], [0.0, 1.0, 0.0], Face::Bottom),
            ([0.5, 3.0, 0.5], [0.0, -1.0, 0.0], Face::Top),
            ([0.5, 0.5, -2.0], [0.0, 0.0, 1.0], Face::North),
            ([0.5, 0.5, 3.0], [0.0, 0.0, -1.0], Face::South),
        ];
        for (o, d, face) in cases {
            let h = ray_vs_box(o, d, &unit()).expect("hit");
            assert_eq!(h.face, face);
            assert!((h.distance - 2.0).abs() < 1e-12, "{face:?}");
        }
    }

    #[test]
    fn diagonal_hit_distance() {
        // From (-1, -0.25, 0.5) toward +x and +y at 45 degrees: enters the west
        // face at x = 0 after travelling sqrt(2).
        let s = 1.0 / 2f64.sqrt();
        let h = ray_vs_box([-1.0, -0.25, 0.5], [s, s, 0.0], &unit()).expect("hit");
        assert_eq!(h.face, Face::West);
        assert!((h.distance - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn miss_behind_and_inside() {
        // Pointing away, offset past the box, box behind, origin inside.
        assert!(ray_vs_box([-2.0, 0.5, 0.5], [0.0, 1.0, 0.0], &unit()).is_none());
        assert!(ray_vs_box([-2.0, 2.0, 0.5], [1.0, 0.0, 0.0], &unit()).is_none());
        assert!(ray_vs_box([3.0, 0.5, 0.5], [1.0, 0.0, 0.0], &unit()).is_none());
        assert!(ray_vs_box([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], &unit()).is_none());
        assert!(ray_vs_box([0.5, 0.5, 0.5], [0.3, 0.4, 0.5], &unit()).is_none());
    }

    #[test]
    fn grazing_edge_hits() {
        // Runs exactly along the top face plane (y = 1).
        let h = ray_vs_box([-1.0, 1.0, 0.5], [1.0, 0.0, 0.0], &unit());
        assert!(h.is_some());
        // Just above it misses.
        assert!(ray_vs_box([-1.0, 1.0001, 0.5], [1.0, 0.0, 0.0], &unit()).is_none());
    }

    fn block(pos: [i32; 3], face: Face) -> Option<Target> {
        Some(Target::Block { pos, face })
    }

    fn cast(w: &impl CollisionWorld, o: [f64; 3], d: [f64; 3]) -> Option<Target> {
        let reach = TargetingConfig::default_from_data().unwrap().block_reach as f64;
        cast_block(w, o, d, reach).map(|(t, _)| t)
    }

    #[test]
    fn casts_hit_solid_plant_and_water() {
        use terrain::Block;
        let mut w = MemoryWorld::new();
        w.set(3, 0, 0, Block::Stone);
        w.set(0, 0, 3, Block::TallGrass);
        w.set(0, 3, 0, Block::Water);
        let o = [0.5, 0.5, 0.5];
        assert_eq!(cast(&w, o, [1.0, 0.0, 0.0]), block([3, 0, 0], Face::West));
        assert_eq!(cast(&w, o, [0.0, 0.0, 1.0]), block([0, 0, 3], Face::North));
        assert_eq!(cast(&w, o, [0.0, 1.0, 0.0]), block([0, 3, 0], Face::Bottom));
        // Air only.
        assert_eq!(cast(&w, o, [-1.0, 0.0, 0.0]), None);
        assert_eq!(cast(&w, o, [0.0, 0.0, 0.0]), None);
    }

    #[test]
    fn cactus_uses_its_inset_box() {
        use terrain::Block;
        let mut w = MemoryWorld::new();
        w.set(3, 0, 0, Block::Cactus);
        let (_, d) = cast_block(&w, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 9.0).unwrap();
        assert!((d - (3.0 + 1.0 / 16.0 - 0.5)).abs() < 1e-9);
    }

    #[test]
    fn nearest_wins_and_reach_limits() {
        use terrain::Block;
        let mut w = MemoryWorld::new();
        w.set(2, 0, 0, Block::TallGrass);
        w.set(3, 0, 0, Block::Stone);
        w.set(9, 4, 4, Block::Stone);
        let o = [0.5, 0.5, 0.5];
        assert_eq!(cast(&w, o, [1.0, 0.0, 0.0]), block([2, 0, 0], Face::West));
        // Stone at x = 5 is 4.5 away from x = 0.5: exactly at the reach.
        let mut w = MemoryWorld::new();
        w.set(5, 0, 0, Block::Stone);
        assert_eq!(cast(&w, o, [1.0, 0.0, 0.0]), block([5, 0, 0], Face::West));
        let mut w = MemoryWorld::new();
        w.set(6, 0, 0, Block::Stone);
        assert_eq!(cast(&w, o, [1.0, 0.0, 0.0]), None);
        assert_eq!(cast(&w, [0.5, 0.5, 0.5], [2.0, 0.0, 0.0]), None);
    }

    #[test]
    fn raft_cells_are_targeted_at_their_current_pose() {
        use crate::raft::{Raft, RaftFrame, RaftWorld};
        use terrain::Block;
        let mut base = MemoryWorld::new();
        let mut cells = Vec::new();
        for x in 0..2 {
            base.set(x, 0, 0, Block::Stone);
            cells.push([x, 0, 0]);
        }
        let mut raft = Raft::new(&cells);
        let o = [-2.5, 0.5, 0.5];
        let east = [1.0, 0.0, 0.0];
        assert_eq!(
            cast(&RaftWorld::new(&base, &raft), o, east),
            block([0, 0, 0], Face::West)
        );
        let frame = RaftFrame {
            displacement: [0.0, 0.0, 3.0],
            ..RaftFrame::default()
        };
        let p = crate::physics::PlayerPhysics::default_from_data().unwrap();
        raft.advance(&frame, &mut [], &p);
        let world = RaftWorld::new(&base, &raft);
        // The rest pose is empty now.
        assert_eq!(cast(&world, o, east), None);
        let o = [-2.5, 0.5, 3.5];
        assert_eq!(cast(&world, o, east), block([0, 0, 3], Face::West));
    }

    #[test]
    fn loads_shipped_data() {
        let c = TargetingConfig::default_from_data().unwrap();
        assert_eq!(c.block_reach, 4.5);
        assert_eq!(c.entity_reach, 3.0);
        assert!((c.entity_box_expansion - 0.1).abs() < 1e-6);
    }

    #[test]
    fn loader_rejects_bad_data() {
        assert!(matches!(
            TargetingConfig::parse("[reach]\nblock = 4.5\n"),
            Err(TargetingError::Missing(_))
        ));
        let bad = "[reach]\nblock = -1\nentity = 3\n[entity]\nbox_expansion = 0.1\n";
        assert!(matches!(
            TargetingConfig::parse(bad),
            Err(TargetingError::Invalid(_))
        ));
        assert!(matches!(
            TargetingConfig::parse("not toml ="),
            Err(TargetingError::Parse(_))
        ));
    }
}
