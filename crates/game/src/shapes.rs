//! Game-side collision shapes (basic-gameplay.md 4.4, decided in #88).
//!
//! Blocks listed here use this table's box in place of their
//! `data/blocks.toml` `collision`. The sizes are this project's own
//! (villages-and-villagers.md 2.4).

use crate::collision::Aabb;
use terrain::Block;

/// A box in block-local coordinates (`0..1` on every axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Shape {
    /// The shape placed in the cell at block coordinates `(x, y, z)`.
    pub fn at(&self, x: i32, y: i32, z: i32) -> Aabb {
        let o = [x as f64, y as f64, z as f64];
        Aabb::new(
            [o[0] + self.min[0], o[1] + self.min[1], o[2] + self.min[2]],
            [o[0] + self.max[0], o[1] + self.max[1], o[2] + self.max[2]],
        )
    }
}

/// `plank_step`: the bottom half of the cell.
pub const PLANK_STEP: Shape = Shape {
    min: [0.0, 0.0, 0.0],
    max: [1.0, 0.5, 1.0],
};

/// `drying_rack`: a frame inset 2/16 on the sides, 12/16 tall.
pub const DRYING_RACK: Shape = Shape {
    min: [2.0 / 16.0, 0.0, 2.0 / 16.0],
    max: [14.0 / 16.0, 12.0 / 16.0, 14.0 / 16.0],
};

/// `net`: a heap of netting over the whole floor, 3/16 tall.
pub const NET: Shape = Shape {
    min: [0.0, 0.0, 0.0],
    max: [1.0, 3.0 / 16.0, 1.0],
};

/// The table's shape for `block`, or `None` to use `data/blocks.toml`.
pub fn shape(block: Block) -> Option<Shape> {
    match block {
        Block::PlankStep => Some(PLANK_STEP),
        Block::DryingRack => Some(DRYING_RACK),
        Block::Net => Some(NET),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{block_box, selection_box, CollisionWorld, MemoryWorld};
    use crate::movement::{move_box, move_with_step};

    #[test]
    fn table_blocks_get_their_boxes() {
        assert_eq!(
            block_box(Block::PlankStep, 3, 7, -2),
            Some(Aabb::new([3.0, 7.0, -2.0], [4.0, 7.5, -1.0]))
        );
        assert_eq!(
            block_box(Block::DryingRack, 3, 7, -2),
            Some(Aabb::new([3.125, 7.0, -1.875], [3.875, 7.75, -1.125]))
        );
        assert_eq!(
            block_box(Block::Net, 3, 7, -2),
            Some(Aabb::new([3.0, 7.0, -2.0], [4.0, 7.1875, -1.0]))
        );
        // Selection uses the same boxes, being smaller than the cell.
        for b in [Block::PlankStep, Block::DryingRack, Block::Net] {
            assert_eq!(selection_box(b, 3, 7, -2), block_box(b, 3, 7, -2));
        }
        assert_eq!(shape(Block::Stone), None);
        assert_eq!(shape(Block::Cactus), None);
    }

    #[test]
    fn world_queries_see_table_boxes() {
        let mut w = MemoryWorld::new();
        w.set(0, 0, 0, Block::Net);
        let mut out = Vec::new();
        // Above the net's top is clear; inside it is not.
        w.boxes_in(&Aabb::new([0.2, 0.2, 0.2], [0.8, 1.0, 0.8]), &mut out);
        assert!(out.is_empty());
        w.boxes_in(&Aabb::new([0.2, 0.1, 0.2], [0.8, 1.0, 0.8]), &mut out);
        assert_eq!(out, vec![NET.at(0, 0, 0)]);
        let mut sel = Vec::new();
        w.selection_boxes_in(&Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]), &mut sel);
        assert_eq!(sel[0].aabb, NET.at(0, 0, 0));
    }

    #[test]
    fn player_steps_up_onto_plank_step() {
        let mut w = MemoryWorld::new();
        for x in -3..4 {
            for z in -3..4 {
                w.set(x, 0, z, Block::Stone);
            }
        }
        w.set(1, 1, 0, Block::PlankStep);
        let bb = Aabb::from_size([0.2, 1.0, 0.2], [0.6, 1.8, 0.6]);
        let m = move_with_step(&w, &bb, [0.4, 0.0, 0.0], 0.5, true);
        assert!((m.applied[0] - 0.4).abs() < 1e-12);
        assert!((m.aabb.min[1] - 1.5).abs() < 1e-12);
        assert!(m.on_ground);
        // Without stepping the plank step blocks.
        assert!(move_box(&w, &bb, [0.4, 0.0, 0.0]).applied[0] < 0.4);
    }
}
