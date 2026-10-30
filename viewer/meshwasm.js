// The Rust block mesher (crates/mesh) behind the same call shape as meshBlocks
// in mesh.js. Not used by the viewer or client yet.
import { VERTEX_BYTES } from './mesh.js';

const FAST = 1, GREEDY = 2;

const part = (bytes) => ({ verts: bytes.buffer, faces: bytes.length / VERTEX_BYTES / 4 });

/**
 * `wasm` is the initialised terrain_web module. `tints` and `origin` are passed
 * on but the Rust mesher doesn't read them yet.
 */
export function meshBlocksWasm(wasm, ctx, blocks, tints, size, pad = 0, { fast = false, origin = [0, 0], greedy = true } = {}) {
  const r = wasm.mesh_blocks(blocks, size, pad, (fast ? FAST : 0) | (greedy ? GREEDY : 0), origin[0], origin[1], ctx.faceLayer, new Float32Array(0));
  const out = { opaque: part(r.opaque), cutout: part(r.cutout), water: part(r.water), ice: part(r.ice), meanY: r.meanY, minY: r.minY };
  r.free();
  return out;
}
