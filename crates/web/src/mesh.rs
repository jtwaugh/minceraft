//! Wasm export of the Rust block mesher (`crates/mesh`), the counterpart of
//! `meshBlocks` in `viewer/mesh.js`. `viewer/meshwasm.js` turns the result
//! into the JS mesher's `{opaque, cutout, water, ice, meanY, minY}` shape.
use alloc::vec::Vec;
use mesh::{Tints, Variants};
use wasm_bindgen::prelude::*;

/// Bit in `mesh_blocks`' `flags`: fast leaves.
const FAST: u32 = 1;
/// Bit in `mesh_blocks`' `flags`: merge untinted cube faces.
const GREEDY: u32 = 2;
/// Bit in `mesh_blocks`' `flags`: `tints` carries the corner grids.
const CORNERS: u32 = 4;

/// The four vertex buffers (16 bytes a vertex, 4 vertices a face) and the
/// surface statistics of a meshed area.
#[wasm_bindgen]
pub struct MeshOut {
    opaque: Vec<u8>,
    cutout: Vec<u8>,
    water: Vec<u8>,
    ice: Vec<u8>,
    mean_y: f64,
    min_y: u32,
}

#[wasm_bindgen]
impl MeshOut {
    #[wasm_bindgen(getter)]
    pub fn opaque(&self) -> Vec<u8> {
        self.opaque.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn cutout(&self) -> Vec<u8> {
        self.cutout.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn water(&self) -> Vec<u8> {
        self.water.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn ice(&self) -> Vec<u8> {
        self.ice.clone()
    }
    #[wasm_bindgen(getter, js_name = meanY)]
    pub fn mean_y(&self) -> f64 {
        self.mean_y
    }
    #[wasm_bindgen(getter, js_name = minY)]
    pub fn min_y(&self) -> u32 {
        self.min_y
    }
}

/// Mesh `blocks` (`((z * w) + x) * 128 + y`, `w = size + 2 * pad`) with the
/// face-layer table of `faceLayers` in `viewer/mesh.js`. `flags` bit 0 is fast
/// leaves; bit 1 merges untinted cube faces (greedy); bit 2 says `tints`
/// has corner grids. `origin_x` and `origin_z` are the world x and z of block
/// (0, 0); `variants` is 9 f64 per texture layer (count, then cumulative
/// weights) as packed by `viewer/meshwasm.js`. Features the Rust mesher
/// doesn't have yet are listed in `viewer/test/mesh_parity.test.mjs`.
///
/// `tints` is the grass, leaf, water and dry tints of each column
/// (`size * size * 3` f32 each, in that order), then with the corners flag the
/// same four per block corner (`(size + 1)^2 * 3` each). `colors` is the block
/// colour table, 256 * 3 u8 (`ctx.COLORS`).
#[wasm_bindgen]
pub fn mesh_blocks(
    blocks: &[u8],
    size: u32,
    pad: u32,
    flags: u32,
    origin_x: i32,
    origin_z: i32,
    face_layer: &[u8],
    tints: &[f32],
    colors: &[u8],
    variants: &[f64],
) -> MeshOut {
    let tints = Tints {
        size: size as usize,
        data: tints,
        corners: flags & CORNERS != 0,
        colors,
    };
    let m = mesh::mesh_blocks(
        blocks,
        size as usize,
        pad as usize,
        flags & FAST != 0,
        flags & GREEDY != 0,
        face_layer,
        &tints,
        [origin_x, origin_z],
        &Variants(variants),
    );
    MeshOut {
        opaque: m.opaque.bytes().to_vec(),
        cutout: m.cutout.bytes().to_vec(),
        water: m.water.bytes().to_vec(),
        ice: m.ice.bytes().to_vec(),
        mean_y: m.mean_y,
        min_y: m.min_y as u32,
    }
}
