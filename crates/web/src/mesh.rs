//! Wasm export of the Rust block mesher (`crates/mesh`), the counterpart of
//! `meshBlocks` in `viewer/mesh.js`. `viewer/meshwasm.js` turns the result
//! into the JS mesher's `{opaque, cutout, water, ice, meanY, minY}` shape.
use alloc::vec::Vec;
use wasm_bindgen::prelude::*;

/// Bit in `mesh_blocks`' `flags`: fast leaves.
const FAST: u32 = 1;

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
/// leaves; bit 1 (greedy) is accepted but not used yet. `origin_x`, `origin_z`
/// and `tints` (per-column data, layout still to be fixed) are likewise
/// reserved for the features the Rust mesher doesn't have yet: see the list in
/// `viewer/test/mesh_parity.test.mjs`.
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
) -> MeshOut {
    let _ = (origin_x, origin_z, tints);
    let m = mesh::mesh_blocks(
        blocks,
        size as usize,
        pad as usize,
        flags & FAST != 0,
        face_layer,
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
