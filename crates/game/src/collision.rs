//! Block collision boxes and the collision world (basic-gameplay.md 4.4).
//!
//! Shapes come from the terrain crate's block data (`data/blocks.toml`
//! `collision`): `none` has no box, `full` a unit cube, `inset` the cactus
//! shape (1/16 inset on the sides and top). Partial-height shapes are a
//! game-side table and not part of this module yet.

use std::collections::HashMap;
use terrain::{Block, Collision, Region, HEIGHT};

/// Cactus inset on the four sides and the top.
pub const INSET: f64 = 1.0 / 16.0;

/// Axis-aligned box in world coordinates (`min <= max` on every axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Aabb {
        Aabb { min, max }
    }

    /// The box with its minimum corner at `min` and the given size.
    pub fn from_size(min: [f64; 3], size: [f64; 3]) -> Aabb {
        Aabb::new(min, [min[0] + size[0], min[1] + size[1], min[2] + size[2]])
    }

    /// Strict overlap: boxes that only touch do not intersect.
    pub fn intersects(&self, o: &Aabb) -> bool {
        (0..3).all(|i| self.min[i] < o.max[i] && self.max[i] > o.min[i])
    }

    pub fn offset(&self, d: [f64; 3]) -> Aabb {
        Aabb::new(
            [self.min[0] + d[0], self.min[1] + d[1], self.min[2] + d[2]],
            [self.max[0] + d[0], self.max[1] + d[1], self.max[2] + d[2]],
        )
    }
}

/// The collision box of `block` placed at block coordinates `(x, y, z)`.
pub fn block_box(block: Block, x: i32, y: i32, z: i32) -> Option<Aabb> {
    let (x, y, z) = (x as f64, y as f64, z as f64);
    match block.info().collision {
        Collision::None => None,
        Collision::Full => Some(Aabb::new([x, y, z], [x + 1.0, y + 1.0, z + 1.0])),
        Collision::Inset => Some(Aabb::new(
            [x + INSET, y, z + INSET],
            [x + 1.0 - INSET, y + 1.0 - INSET, z + 1.0 - INSET],
        )),
    }
}

/// A world that can report the block boxes overlapping a query box.
pub trait CollisionWorld {
    /// The block at integer coordinates; air where nothing is known.
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block;

    /// Append every block box that strictly overlaps `query` to `out`.
    fn boxes_in(&self, query: &Aabb, out: &mut Vec<Aabb>) {
        let lo = |i: usize| query.min[i].floor() as i32;
        let hi = |i: usize| query.max[i].floor() as i32;
        for x in lo(0)..=hi(0) {
            for y in lo(1)..=hi(1) {
                for z in lo(2)..=hi(2) {
                    if let Some(b) = block_box(self.block_at(x, y, z), x, y, z) {
                        if b.intersects(query) {
                            out.push(b);
                        }
                    }
                }
            }
        }
    }
}

/// A collision world backed by a generated terrain region. Everything
/// outside the region (or above or below it) is air.
pub struct RegionWorld<'a> {
    pub region: &'a Region,
}

impl<'a> RegionWorld<'a> {
    pub fn new(region: &'a Region) -> RegionWorld<'a> {
        RegionWorld { region }
    }
}

impl CollisionWorld for RegionWorld<'_> {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        let r = self.region;
        let (lx, lz) = (x as i64 - r.x0 as i64, z as i64 - r.z0 as i64);
        if lx < 0 || lz < 0 || lx >= r.width as i64 || lz >= r.depth as i64 {
            return Block::Air;
        }
        if y < 0 || y >= HEIGHT as i32 {
            return Block::Air;
        }
        r.get(lx as u32, y as u32, lz as u32)
    }
}

/// A simple in-memory world for tests: unset positions are air.
#[derive(Default)]
pub struct MemoryWorld {
    blocks: HashMap<(i32, i32, i32), Block>,
}

impl MemoryWorld {
    pub fn new() -> MemoryWorld {
        MemoryWorld::default()
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, block: Block) {
        self.blocks.insert((x, y, z), block);
    }
}

impl CollisionWorld for MemoryWorld {
    fn block_at(&self, x: i32, y: i32, z: i32) -> Block {
        self.blocks.get(&(x, y, z)).copied().unwrap_or(Block::Air)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain::generate_decorated;

    fn query(w: &impl CollisionWorld, q: &Aabb) -> Vec<Aabb> {
        let mut v = Vec::new();
        w.boxes_in(q, &mut v);
        v
    }

    #[test]
    fn shapes_match_blocks_toml() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/blocks.toml"
        ))
        .unwrap();
        let doc: toml::Value = text.parse().unwrap();
        let blocks = doc["block"].as_array().unwrap();
        assert_eq!(blocks.len(), Block::COUNT);
        for b in blocks {
            let id = b["id"].as_integer().unwrap() as usize;
            let block = Block::ALL[id];
            let got = block_box(block, 3, 7, -2);
            let expect = match b["collision"].as_str().unwrap() {
                "none" => None,
                "full" => Some(Aabb::new([3.0, 7.0, -2.0], [4.0, 8.0, -1.0])),
                "inset" => Some(Aabb::new(
                    [3.0 + INSET, 7.0, -2.0 + INSET],
                    [4.0 - INSET, 8.0 - INSET, -1.0 - INSET],
                )),
                c => panic!("unknown collision {c}"),
            };
            assert_eq!(got, expect, "{}", b["name"]);
        }
    }

    #[test]
    fn query_returns_overlapping_boxes_only() {
        let mut w = MemoryWorld::new();
        w.set(0, 0, 0, Block::Stone);
        w.set(1, 0, 0, Block::Cactus);
        w.set(2, 0, 0, Block::Stone);
        let q = Aabb::new([0.5, 0.5, 0.5], [1.5, 1.5, 0.9]);
        let got = query(&w, &q);
        assert_eq!(got.len(), 2);
        assert!(got.contains(&block_box(Block::Stone, 0, 0, 0).unwrap()));
        assert!(got.contains(&block_box(Block::Cactus, 1, 0, 0).unwrap()));
        // Touching only: block (0,0,0) tops out at y = 1.
        let touching = Aabb::new([0.2, 1.0, 0.2], [0.8, 2.0, 0.8]);
        assert!(query(&w, &touching).is_empty());
        // A query in the cactus's side inset does not hit it.
        let gap = Aabb::new([1.0, 0.0, 0.2], [1.05, 0.5, 0.8]);
        assert!(query(&w, &gap).is_empty());
        // Far away.
        let far = Aabb::new([10.0, 0.0, 10.0], [11.0, 1.0, 11.0]);
        assert!(query(&w, &far).is_empty());
    }

    #[test]
    fn region_world_agrees_with_memory_world() {
        let r = generate_decorated(12345, -8, -8, 32, 32);
        let mut mem = MemoryWorld::new();
        for z in 0..32u32 {
            for x in 0..32u32 {
                for y in 0..HEIGHT as u32 {
                    mem.set(x as i32 - 8, y as i32, z as i32 - 8, r.get(x, y, z));
                }
            }
        }
        let rw = RegionWorld::new(&r);
        for (x, z) in [(-8.0, -8.0), (0.3, 4.6), (10.0, 12.0), (20.5, 20.5)] {
            for y in (0..HEIGHT).step_by(8) {
                let q = Aabb::from_size([x, y as f64, z], [3.4, 9.0, 3.4]);
                assert_eq!(query(&rw, &q), query(&mem, &q));
            }
        }
        // Outside the region is air; inside, a full-height column has ground.
        let out = Aabb::new([100.0, 0.0, 100.0], [102.0, 127.0, 102.0]);
        assert!(query(&rw, &out).is_empty());
        assert!(!query(&rw, &Aabb::new([0.0, 0.0, 0.0], [1.0, 128.0, 1.0])).is_empty());
    }

    #[test]
    fn overlapping_regions_give_same_boxes() {
        let a = generate_decorated(777, 0, 0, 48, 48);
        let b = generate_decorated(777, 16, 16, 48, 48);
        let (wa, wb) = (RegionWorld::new(&a), RegionWorld::new(&b));
        for (x, z) in [(16.0, 16.0), (25.5, 30.2), (40.0, 41.0), (20.0, 44.0)] {
            for y in (0..HEIGHT).step_by(4) {
                let q = Aabb::from_size([x, y as f64, z], [4.0, 6.0, 4.0]);
                assert_eq!(query(&wa, &q), query(&wb, &q), "at {x},{y},{z}");
            }
        }
    }
}
