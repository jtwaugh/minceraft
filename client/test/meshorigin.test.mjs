import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H, VERTEX_BYTES, meshBlocks } from '../../viewer/mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

function flat(size, pad) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let c = 0; c < w * w; c++) for (let y = 0; y < 60; y++) blocks[c * H + y] = y === 59 ? B.grass : B.stone;
  return blocks;
}

// UV bytes of every top face, keyed by world "x,z".
function topUVs(m, [ox, oz]) {
  const dv = new DataView(m.opaque.verts), out = new Map();
  for (let q = 0; q < m.opaque.faces; q++) {
    const o = q * 4 * VERTEX_BYTES;
    if (![0, 1, 2, 3].every((j) => dv.getInt16(o + j * VERTEX_BYTES + 2, true) / 16 === 60)) continue;
    const uv = [0, 1, 2, 3].map((j) => `${dv.getUint8(o + j * VERTEX_BYTES + 6)}:${dv.getUint8(o + j * VERTEX_BYTES + 7)}`);
    out.set(`${dv.getInt16(o, true) / 16 + ox},${dv.getInt16(o + 4, true) / 16 + oz}`, uv.join(','));
  }
  return out;
}

test('a client region mesh and a preview-style mesh give the same world block the same UVs', () => {
  // client: padded region at chunk origin (64, -32); preview: unpadded area starting at (70, -30)
  const client = meshBlocks(ctx, flat(16, 1), tints(16), 16, 1, { origin: [64, -32] });
  const preview = meshBlocks(ctx, flat(8, 0), tints(8), 8, 0, { origin: [70, -30] });
  const a = topUVs(client, [64, -32]), b = topUVs(preview, [70, -30]);
  assert.equal(b.size, 64);
  for (const [k, v] of b) assert.equal(a.get(k), v, k);
});
