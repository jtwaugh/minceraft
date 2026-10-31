//! Thin wasm-bindgen wrapper around the `terrain` crate.
//!
//! Seeds are passed as `BigInt` (u64). Results are returned as typed arrays
//! (copied out of wasm memory by the bindgen glue).

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use layout::{Habitats, Layout};
use terrain::Biases;
mod mesh;
use wasm_bindgen::prelude::*;

/// Terrain-only block bytes for a `w x d` region at world `(x0, z0)`.
/// Layout: `((z * w) + x) * 128 + y`; values are `terrain::Block` ids (see
/// `legend()`; 0 Air .. 6 Water, 7 Ice, ...).
#[wasm_bindgen]
pub fn generate_region(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_region(seed, x0, z0, w, d).blocks
}

/// Like `generate_region` but with trees, plants, snow layers and ice.
/// Identical for any region origin / size (decorated with a margin, cropped).
#[wasm_bindgen]
pub fn generate_region_decorated(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_decorated(seed, x0, z0, w, d).blocks
}

/// Terrain-only per-column summary, 4 bytes per column in order `z * w + x`:
/// `[top_block, top_y, floor_y, floor_block]` (floor = first non-water/ice
/// block below a water or ice surface; equals top for dry columns).
#[wasm_bindgen]
pub fn heightmap(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_heightmap(seed, x0, z0, w, d)
}

/// Same format as `heightmap`, after decoration: `top_block` may be a log,
/// leaves, a plant or a snow layer.
#[wasm_bindgen]
pub fn heightmap_decorated(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_heightmap_with_biases(seed, x0, z0, w, d, &Biases::default(), true)
}

/// Coarse land survey for finding islands and features over a wide area:
/// 4 bytes per sample `[top_y, biome, gaps, gap_h]` (order `k * nx + i`;
/// gaps = air gaps above sea level under solid, gap_h = the tallest) at world columns
/// `(x0 & !3) + i * step`, `(z0 & !3) + k * step`; `step` is rounded up to a
/// multiple of 4. Land iff `top_y >= 64`. Heights equal the terrain
/// heightmap's at those columns, but cost one lattice point per sample.
#[wasm_bindgen]
pub fn coarse_heightmap(seed: u64, x0: i32, z0: i32, step: u32, nx: u32, nz: u32) -> Vec<u8> {
    terrain::generate_coarse_heightmap(seed, x0, z0, step, nx, nz)
}

/// Biome id per column (`z * w + x`); names/colours via `legend()`.
#[wasm_bindgen]
pub fn biome_map(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    terrain::generate_climate(seed, x0, z0, w, d, &[], &[]).biome
}

/// Temperature T in [0, 1] per column (`z * w + x`).
#[wasm_bindgen]
pub fn temperature_map(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<f32> {
    terrain::generate_climate(seed, x0, z0, w, d, &[], &[]).temperature
}

/// Humidity H in [0, 1] per column (`z * w + x`).
#[wasm_bindgen]
pub fn humidity_map(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<f32> {
    terrain::generate_climate(seed, x0, z0, w, d, &[], &[]).humidity
}

/// Legend JSON (generated at build time from `data/*.toml`):
/// `{ blocks: [{id, name, color:[r,g,b], opacity, collision, fire:[enc,flam]}],
///    biomes: [{id, name, map:[r,g,b], grass:[[r,g,b]x3], leaf:[..x3], sky:[..x3], water:[..x3], dry:[..x3]}],
///    ramps: {grass:"humidity", leaf:"humidity", water:"humidity", dry:"humidity", sky:"temperature"}, blend: {radius, shade_mix, shade_radius} }`.
/// Ramps have three stops at 0, 0.5 and 1 of the named climate value.
#[wasm_bindgen]
pub fn legend() -> String {
    String::from(terrain::LEGEND_JSON)
}

/// Habitat index per column (`z * w + x`) for the plain seed (no layout
/// bias terms, so it lines up with the other viewer modes). The index is a
/// rule's position in `data/habitats.toml` (see `habitat_names()`); land
/// columns hold 255.
#[wasm_bindgen]
pub fn habitat_map(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Vec<u8> {
    Layout::new(seed).habitat_map(x0, z0, w, d).habitat
}

/// Habitat ids in rule order, as a JSON array of strings; index `i` is
/// the value `habitat_map` uses for that habitat.
#[wasm_bindgen]
pub fn habitat_names() -> String {
    let mut json = String::from("[");
    for (i, r) in Habitats::shipped().rules.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('"');
        json.push_str(&r.id);
        json.push('"');
    }
    json.push(']');
    json
}
