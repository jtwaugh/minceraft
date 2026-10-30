//! Building blocks of the block mesher, ported from `viewer/mesh.js`: the
//! 16-byte vertex writer and the coordinate hashes that vary tiles and plants. Nothing here reads block data.
//!
//! Numbers are `f64` and rounded like JavaScript's `Math.round` so the bytes
//! match the JS mesher exactly.
#![no_std]

extern crate alloc;

mod mesh;
pub mod quads;

pub use mesh::{mesh_blocks, Mesh};
pub use quads::{Quads, Tint};

/// JavaScript `Math.round`: halves round towards +infinity.
pub fn js_round(v: f64) -> f64 {
    let s = v + 0.5;
    let t = s as i64 as f64;
    if t > s {
        t - 1.0
    } else {
        t
    }
}

/// Integer hash of a world coordinate and a salt (signed, as the JS returns it).
pub fn hash_at(x: i32, y: i32, z: i32, salt: i32) -> i32 {
    let mut h = x.wrapping_mul(0x27d4eb2d)
        ^ y.wrapping_mul(0x165667b1)
        ^ z.wrapping_mul(0x9e3779b1u32 as i32)
        ^ salt;
    h = (h ^ ((h as u32) >> 15) as i32).wrapping_mul(0x85ebca6bu32 as i32);
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(0xc2b2ae35u32 as i32);
    h ^ ((h as u32) >> 16) as i32
}

/// Tile variant 0-7 from the world coordinate alone.
pub fn variant_at(x: i32, y: i32, z: i32) -> u32 {
    (hash_at(x, y, z, 0) & 7) as u32
}

/// Cross-plant shift in x and z, in whole 1/16s from -2 to 2.
pub fn plant_offset(x: i32, y: i32, z: i32) -> [f64; 2] {
    let h = hash_at(x, y, z, 0x2c1b3c6d) as u32;
    [((h >> 4) % 5) as f64 - 2.0, ((h >> 12) % 5) as f64 - 2.0].map(|v| v / 16.0)
}

/// Cross-plant shape: mirrored texture, height (12/16..16/16) and brightness.
#[derive(Debug, PartialEq)]
pub struct PlantShape {
    pub mirror: bool,
    pub height: f64,
    pub bright: f64,
}

pub fn plant_shape(x: i32, y: i32, z: i32) -> PlantShape {
    let h = hash_at(x, y, z, 0x7f4a7c15) as u32;
    PlantShape {
        mirror: h & 1 == 1,
        height: (12 + ((h >> 1) & 0xff) % 5) as f64 / 16.0,
        bright: 1.0 + (((h >> 9) & 0xff) as f64 / 255.0 - 0.5) * 0.12,
    }
}
