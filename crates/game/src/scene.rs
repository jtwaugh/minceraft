//! The raft test scene: a flat sea with the layout crate's `raft_start`
//! structure floating in it, and a spawn in the water in front of it.
//! See docs/design/basic-gameplay.md section 13, phase 3 (Water and raft).

use crate::collision::Aabb;
use crate::targeting::TargetableEntity;
use layout::Structure;
use terrain::{Block, Region, HEIGHT};

/// The sea covers `SEA_MIN..SEA_MIN + SEA_SIZE` in x and z.
pub const SEA_MIN: i32 = -32;
pub const SEA_SIZE: u32 = 64;
/// Sea floor: stone up to and including this y.
const FLOOR: i32 = 48;
/// Water fills `FLOOR + 1 .. SURFACE`; the raft sits on top of it.
const SURFACE: i32 = 64;
/// Side of the square grass field to till, and the y of its grass.
pub const FIELD_SIZE: i32 = 3;
pub const FIELD_Y: i32 = SURFACE - 1;
/// Blocks between the raft's +z edge and the spawn.
const SPAWN_GAP: f64 = 3.0;

#[derive(Clone)]
pub struct RaftScene {
    /// Sea floor and water, without the raft.
    pub sea: Region,
    /// The raft's mask cells.
    pub cells: Vec<[i32; 3]>,
    /// The raft's blocks alone, in a region the same size as `sea`.
    pub raft: Region,
    /// Feet position in the water, facing the raft at yaw 0 (towards -z).
    pub spawn: [f64; 3],
    /// First column `[x, z]` of the hoe-able field, `FIELD_SIZE` square; its
    /// grass is at `FIELD_Y`.
    pub field: [i32; 2],
    /// Stub targets for checking the selection outline: a bare entity box.
    pub entities: Vec<TargetableEntity>,
}

fn empty() -> Region {
    Region {
        blocks: vec![0u8; SEA_SIZE as usize * SEA_SIZE as usize * HEIGHT],
        width: SEA_SIZE,
        depth: SEA_SIZE,
        height: HEIGHT as u32,
        x0: SEA_MIN,
        z0: SEA_MIN,
    }
}

/// Build the scene. The raft's bottom layer sits at the water surface.
pub fn raft_scene() -> RaftScene {
    let template = Structure::raft_start();
    let cells = template.solid_cells([0, SURFACE, 0], 0);
    let mut sea = empty();
    for z in 0..SEA_SIZE {
        for x in 0..SEA_SIZE {
            for y in 0..SURFACE {
                let b = if y <= FLOOR {
                    Block::Stone
                } else {
                    Block::Water
                };
                let i = sea.index(x, y as u32, z);
                sea.blocks[i] = b as u8;
            }
        }
    }
    let mut raft = empty();
    let (mut sum_x, mut max_z, mut min_y) = ((i32::MAX, i32::MIN), i32::MIN, i32::MAX);
    for &(x, y, z, b) in &cells {
        let (lx, lz) = ((x - SEA_MIN) as u32, (z - SEA_MIN) as u32);
        let i = raft.index(lx, y as u32, lz);
        raft.blocks[i] = b;
        sum_x = (sum_x.0.min(x), sum_x.1.max(x));
        max_z = max_z.max(z);
        min_y = min_y.min(y);
    }
    let spawn = [
        (sum_x.0 + sum_x.1 + 1) as f64 / 2.0,
        (min_y - 1) as f64,
        max_z as f64 + 1.0 + SPAWN_GAP,
    ];
    let entities = add_islet(&mut sea, spawn);
    let field = add_field(&mut sea, spawn);
    RaftScene {
        sea,
        cells: cells.iter().map(|&(x, y, z, _)| [x, y, z]).collect(),
        raft,
        spawn,
        field,
        entities,
    }
}

/// A grassy 3x3 islet beside the spawn with a few placed plants on it, and a
/// stub animal-like box standing on it. Test scene only: it exists so the
/// selection outline and the dispatch overlay have plants and an entity to
/// point at. Returns the stub entities.
fn add_islet(sea: &mut Region, spawn: [f64; 3]) -> Vec<TargetableEntity> {
    let (x0, z0) = (spawn[0].floor() as i32 + 1, spawn[2].floor() as i32 - 1);
    // The grass layer replaces the top water layer, so the islet is level
    // with the surface and a swimmer's eye sees over it with no water between.
    let top = SURFACE as u32 - 1;
    for dz in 0..3 {
        for dx in 0..3 {
            let (lx, lz) = ((x0 + dx - SEA_MIN) as u32, (z0 + dz - SEA_MIN) as u32);
            for y in (FLOOR as u32 + 1)..top {
                let i = sea.index(lx, y, lz);
                sea.blocks[i] = Block::Dirt as u8;
            }
            let i = sea.index(lx, top, lz);
            sea.blocks[i] = Block::Grass as u8;
        }
    }
    for (dx, dz, plant) in [
        (0, 0, Block::TallGrass),
        (1, 0, Block::Dandelion),
        (2, 0, Block::Rose),
    ] {
        let i = sea.index(
            (x0 + dx - SEA_MIN) as u32,
            top + 1,
            (z0 + dz - SEA_MIN) as u32,
        );
        sea.blocks[i] = plant as u8;
    }
    let min = [
        f64::from(x0) + 0.7,
        f64::from(top) + 1.0,
        f64::from(z0) + 1.3,
    ];
    vec![TargetableEntity {
        id: 1,
        kind: "animal".into(),
        aabb: Aabb::from_size(min, [0.9, 0.9, 0.9]),
    }]
}

/// A bare grass field level with the water, two blocks east of the islet
/// and surrounded by water, so tilled tiles are hydrated. Nothing on it.
fn add_field(sea: &mut Region, spawn: [f64; 3]) -> [i32; 2] {
    let field = [spawn[0].floor() as i32 + 5, spawn[2].floor() as i32 - 1];
    for dz in 0..FIELD_SIZE {
        for dx in 0..FIELD_SIZE {
            let (lx, lz) = (
                (field[0] + dx - SEA_MIN) as u32,
                (field[1] + dz - SEA_MIN) as u32,
            );
            for y in (FLOOR as u32 + 1)..FIELD_Y as u32 {
                let i = sea.index(lx, y, lz);
                sea.blocks[i] = Block::Dirt as u8;
            }
            let i = sea.index(lx, FIELD_Y as u32, lz);
            sea.blocks[i] = Block::Grass as u8;
        }
    }
    field
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raft_is_inside_the_sea_and_spawn_is_in_water() {
        let s = raft_scene();
        assert!(!s.cells.is_empty());
        for c in &s.cells {
            assert!(c[0] >= SEA_MIN && c[0] < SEA_MIN + SEA_SIZE as i32);
            assert!(c[2] >= SEA_MIN && c[2] < SEA_MIN + SEA_SIZE as i32);
        }
        let at = |p: [f64; 3]| {
            s.sea.get(
                (p[0].floor() as i32 - SEA_MIN) as u32,
                p[1].floor() as u32,
                (p[2].floor() as i32 - SEA_MIN) as u32,
            )
        };
        assert_eq!(at(s.spawn), Block::Water);
        let solid = s.raft.blocks.iter().filter(|&&b| b != 0).count();
        assert_eq!(solid, s.cells.len());
    }
}
