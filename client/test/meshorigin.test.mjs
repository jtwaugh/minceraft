import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { B, H, VERTEX_BYTES, meshBlocks, legendContext, columnTints } from '../../viewer/mesh.js';
import { meshBlocksWasm } from '../../viewer/meshwasm.js';
import * as terrain from '../../viewer/pkg/terrain_web.js';
import { initSync, Scene, Walk } from '../pkg/game_web.js';
import { padRegion } from '../regionpad.js';
import { REGION_SIZE } from '../stream.js';

await terrain.default({ module_or_path: fs.readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url)) });
initSync({ module: fs.readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

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

// The two meshes scene.js builds, a streamed region (padded) and the raft scene's
// sea (unpadded), come out of the wasm mesher with the bytes meshBlocks writes.
test('the client region and scene meshes are the same from the wasm mesher', () => {
  const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
  const real = { ...legendContext(terrain.legend()), faceLayer, variants: { 3: { count: 3, cum: [0.6, 0.8, 1] } } };
  const seed = 12345n, walk = Walk.streamed(seed, 32, 127, 32);
  const region = padRegion((a, b) => walk.region_blocks(a, b), 0, 0, REGION_SIZE);
  const scene = new Scene();
  const cases = [
    [region, columnTints(terrain, real, seed, 0, 0, REGION_SIZE), REGION_SIZE, 1, [0, 0]],
    [scene.sea_blocks(), columnTints(terrain, real, seed, scene.min, scene.min, scene.size), scene.size, 0, [scene.min, scene.min]],
  ];
  for (const [blocks, t, size, pad, origin] of cases) {
    const js = meshBlocks(real, blocks, t, size, pad, { origin });
    const rs = meshBlocksWasm(terrain, real, blocks, t, size, pad, { origin });
    assert.ok(js.opaque.faces + js.water.faces > 0);
    for (const k of ['opaque', 'cutout', 'water', 'ice']) assert.deepEqual(new Uint8Array(rs[k].verts), new Uint8Array(js[k].verts), `${k}, size ${size}`);
    assert.equal(rs.meanY, js.meanY);
    assert.equal(rs.minY, js.minY);
  }
});
