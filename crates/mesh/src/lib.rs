//! Building blocks of the block mesher, ported from `viewer/mesh.js`: the
//! 16-byte vertex writer and the coordinate hashes that vary tiles and plants. Nothing here reads block data.
//!
//! Numbers are `f64` and rounded like JavaScript's `Math.round` so the bytes
//! match the JS mesher exactly.
#![no_std]

extern crate alloc;

pub mod ao;
pub mod greedy;
mod mesh;
pub mod plants;
pub mod quads;
pub mod shapes;
pub mod tables;
pub mod tint;

pub use mesh::{mesh_blocks, Mesh};
pub use quads::{Quads, Tint};
pub use tables::{b, face_uv, Face, CROSS, FACES, LEAVES, OPAQUE, OPAQUE_FAST, UPPER_HALF, VARIES};
pub use tint::Tints;

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

/// Tile variant counts and cumulative weights per base layer, as `variantTable`
/// in `viewer/mesh.js` builds them: 9 numbers a layer, the count (0 for a layer
/// without variants) then the cumulative weights.
#[derive(Clone, Copy)]
pub struct Variants<'a>(pub &'a [f64]);

impl Variants<'_> {
    pub const NONE: Variants<'static> = Variants(&[]);
    const STRIDE: usize = 9;
}

/// `layer` moved to its tile variant, picked by the world coordinate alone
/// (a salt apart from `variant_at`). Layers without variants stay.
pub fn pick_layer(v: &Variants, layer: u8, x: i32, y: i32, z: i32) -> u8 {
    let Some(e) =
        v.0.get(layer as usize * Variants::STRIDE..(layer as usize + 1) * Variants::STRIDE)
    else {
        return layer;
    };
    let count = e[0] as usize;
    if count == 0 {
        return layer;
    }
    let r = (hash_at(x, y, z, 0x5bd1e995) as u32 >> 8) as f64 / 16777216.0;
    let mut i = 0;
    while i < count - 1 && r >= e[1 + i] {
        i += 1;
    }
    layer + i as u8
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
