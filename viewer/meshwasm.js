// The Rust block mesher (crates/mesh) behind the same call shape as meshBlocks
// in mesh.js. The viewer's chunk workers (worker.js) mesh with it; the client
// still uses meshBlocks.
import { VERTEX_BYTES } from './mesh.js';

const FAST = 1, GREEDY = 2, CORNERS = 4;
const GRIDS = ['grass', 'leaf', 'water', 'dry'];

// The wasm layout of `tints`: per-column grass, leaf, water and dry tints
// (size * size * 3 f32 each), then, if `corners` is set, the same four per
// block corner ((size + 1)^2 * 3 each). A missing water or dry grid is white.
function packTints(tints, size) {
  const n = size * size * 3, cn = (size + 1) * (size + 1) * 3, c = tints.corners;
  const out = new Float32Array(GRIDS.length * (n + (c ? cn : 0))).fill(1);
  GRIDS.forEach((g, i) => {
    tints[g]?.forEach((t, j) => out.set(t, i * n + j * 3));
    if (c?.[g]) out.set(c[g], GRIDS.length * n + i * cn);
  });
  return out;
}

// ctx.COLORS (block id -> [r, g, b]) as 256 * 3 bytes.
function packColors(colors) {
  const out = new Uint8Array(256 * 3);
  for (let b = 0; b < 256; b++) if (colors[b]) out.set(colors[b], b * 3);
  return out;
}

// ctx.variants (layer -> { count, cum }) as 9 numbers per layer: count, then cum.
function packVariants(variants) {
  const out = new Float64Array(256 * 9);
  for (const [layer, { count, cum }] of Object.entries(variants ?? {})) {
    out[layer * 9] = count;
    out.set(cum.slice(0, 8), layer * 9 + 1);
  }
  return out;
}

const part = (bytes) => ({ verts: bytes.buffer, faces: bytes.length / VERTEX_BYTES / 4 });

/**
 * `wasm` is the initialised terrain_web module. `origin` is the world x and z of block (0, 0).
 */
export function meshBlocksWasm(wasm, ctx, blocks, tints, size, pad = 0, { fast = false, origin = [0, 0], greedy = true } = {}) {
  const r = wasm.mesh_blocks(blocks, size, pad, (fast ? FAST : 0) | (greedy ? GREEDY : 0) | (tints.corners ? CORNERS : 0), origin[0], origin[1], ctx.faceLayer, packTints(tints, size), packColors(ctx.COLORS), packVariants(ctx.variants));
  const out = { opaque: part(r.opaque), cutout: part(r.cutout), water: part(r.water), ice: part(r.ice), meanY: r.meanY, minY: r.minY };
  r.free();
  return out;
}
