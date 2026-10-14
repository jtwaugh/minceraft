//! Thin wasm-bindgen wrapper around the `terrain` crate.
//!
//! Seeds are passed as `BigInt` (u64). Results are returned as
//! `Uint8Array`s (copied out of wasm memory by the bindgen glue).

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use wasm_bindgen::prelude::*;

/// Block bytes for a `w x d` region at world `(x0, z0)`.
/// Layout: `((z * w) + x) * 128 + y`; values are `terrain::Block` ids
/// (0 Air, 1 Stone, 2 Dirt, 3 Grass, 4 Sand, 5 Gravel, 6 Water).
#[wasm_bindgen]
pub fn generate_region(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_region(seed, x0, z0, w, d).blocks
}

/// Per-column summary, 4 bytes per column in order `z * w + x`:
/// `[top_block, top_y, floor_y, floor_block]` (floor = first non-water block
/// below a water surface; equals top for dry columns).
#[wasm_bindgen]
pub fn heightmap(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_heightmap(seed, x0, z0, w, d)
}
