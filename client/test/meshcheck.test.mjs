import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { B, H, VERTEX_BYTES, legendContext } from '../../viewer/mesh.js';
import { meshBlocksWasm } from '../../viewer/meshwasm.js';
import * as terrain from '../../viewer/pkg/terrain_web.js';
import { initSync, Walk } from '../pkg/game_web.js';
import { meshRegionCpu } from '../regionmesh.js';
import { REGION_SIZE } from '../stream.js';
import { meshCracks } from '../meshcheck.js';

await terrain.default({ module_or_path: fs.readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url)) });
initSync({ module: fs.readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
const ctx = { ...legendContext(terrain.legend()), faceLayer, variants: { 3: { count: 3, cum: [0.6, 0.8, 1] } } };
const white = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// A flat world: stone up to y 58, grass at 59, with a one-block step of stone and a pit
// so the check sees edges that meet faces of other directions.
function flatMesh(size = 16, pad = 1) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let c = 0; c < w * w; c++) for (let y = 0; y < 60; y++) blocks[c * H + y] = y === 59 ? B.grass : B.stone;
  blocks[((pad + 4) * w + pad + 4) * H + 60] = B.stone;
  blocks[((pad + 9) * w + pad + 9) * H + 59] = B.air;
  return meshBlocksWasm(terrain, ctx, blocks, white(size), size, pad, { origin: [0, 0] });
}

const corner = (m, q, j, a) => new DataView(m.verts).getInt16((q * 4 + j) * VERTEX_BYTES + a * 2, true);
const topAt = (m, y) => [...Array(m.faces).keys()].filter((q) => [0, 1, 2, 3].every((j) => corner(m, q, j, 1) === y * 16));

test('the opaque mesh of streamed regions of a fixed seed has no cracks', () => {
  const seed = 12345n, walk = Walk.streamed(seed, 32, 127, 32);
  for (const [rx, rz] of [[0, 0], [-1, 0], [0, 1], [1, 1]]) {
    const { mesh } = meshRegionCpu({ walk, terrain, ctx, seed, blendRadius: 2 }, { rx, rz });
    assert.ok(mesh.opaque.faces > 0);
    assert.deepEqual(meshCracks(mesh.opaque, REGION_SIZE), [], `region ${rx},${rz}`);
  }
});

test('a flat synthetic world has no cracks, padded or not', () => {
  assert.deepEqual(meshCracks(flatMesh().opaque, 16), []);
  assert.deepEqual(meshCracks(flatMesh(8, 0).opaque, 8), []);
});

test('a vertex moved by one raw unit is reported, naming the face', () => {
  const { opaque } = flatMesh(), q = topAt(opaque, 60)[0], verts = opaque.verts.slice(0);
  const dv = new DataView(verts), o = (q * 4 + 2) * VERTEX_BYTES;
  dv.setInt16(o, dv.getInt16(o, true) + 1, true);
  const faults = meshCracks({ verts, faces: opaque.faces }, 16);
  const grid = faults.filter((f) => f.check === 'grid');
  assert.equal(grid.length, 1);
  assert.equal(grid[0].face, q);
  assert.match(grid[0].detail, new RegExp(`^face ${q} \\(\\+y at `));
});

test('a face dropped from a flat area leaves open edges that name it', () => {
  const { opaque } = flatMesh(), tops = topAt(opaque, 60);
  // drop the top that covers block (2, 59, 2), inside the flat area
  const q = tops.find((t) => [0, 2].every((a) => Math.min(...[0, 1, 2, 3].map((j) => corner(opaque, t, j, a))) <= 32
    && Math.max(...[0, 1, 2, 3].map((j) => corner(opaque, t, j, a))) >= 48));
  const keep = new Uint8Array(opaque.verts), bytes = 4 * VERTEX_BYTES;
  const verts = new Uint8Array(opaque.verts.byteLength - bytes);
  verts.set(keep.subarray(0, q * bytes));
  verts.set(keep.subarray((q + 1) * bytes), q * bytes);
  const faults = meshCracks({ verts: verts.buffer, faces: opaque.faces - 1 }, 16);
  assert.ok(faults.length > 0);
  assert.ok(faults.every((f) => f.check === 'gap'));
  assert.ok(faults.some((f) => /open edge toward the \+y face of block \d+,59,\d+$/.test(f.detail)));
});

test('a face drawn twice is an overlap', () => {
  const { opaque } = flatMesh(), q = topAt(opaque, 60)[0], bytes = 4 * VERTEX_BYTES;
  const verts = new Uint8Array(opaque.verts.byteLength + bytes);
  verts.set(new Uint8Array(opaque.verts));
  verts.set(new Uint8Array(opaque.verts, q * bytes, bytes), opaque.verts.byteLength);
  const faults = meshCracks({ verts: verts.buffer, faces: opaque.faces + 1 }, 16);
  assert.ok(faults.length > 0);
  assert.ok(faults.every((f) => f.check === 'overlap' && f.face === opaque.faces));
  assert.match(faults[0].detail, new RegExp(`overlaps face ${q} at`));
});
