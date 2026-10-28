import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H, VERTEX_BYTES, meshBlocks } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2).fill(3) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([0.5, 0.25, 0.1]) });
function litter(size) {
  const blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) {
    for (let y = 0; y < 60; y++) blocks[c * H + y] = B.stone;
    blocks[c * H + 60] = B.leaf_litter;
  }
  return blocks;
}
const verts = (q, i) => { const dv = new DataView(q.verts); return [0, 1, 2, 3].map((j) => { const o = (i * 4 + j) * VERTEX_BYTES;
  return { x: dv.getInt16(o, true), y: dv.getInt16(o + 2, true), z: dv.getInt16(o + 4, true),
    uv: `${dv.getUint8(o + 6)}:${dv.getUint8(o + 7)}`, t: [10, 11, 12].map((k) => dv.getUint8(o + k)) }; }); };

test('leaf litter is one up-facing quad 1/16 above the block below, leaf-tinted, inside its column', () => {
  const m = meshBlocks(ctx, litter(4), tints(4), 4, 0);
  assert.equal(m.cutout.faces, 16);
  for (let i = 0; i < 16; i++) for (const v of verts(m.cutout, i)) {
    assert.equal(v.y, 60 * 16 + 1);
    assert.ok(v.x >= 0 && v.x <= 64 && v.z >= 0 && v.z <= 64);
    assert.deepEqual(v.t, [128, 64, 26]);
  }
});

test('it adds no side faces and does not hide the face below', () => {
  const m = meshBlocks(ctx, litter(4), tints(4), 4, 0);
  assert.equal(m.cutout.faces, 16);
  assert.equal(m.water.faces + m.ice.faces, 0);
  // the stone top at y = 60 is still drawn (60 * 16 .. exactly), 16 top faces
  const tops = [...Array(m.opaque.faces).keys()].filter((i) => verts(m.opaque, i).every((v) => v.y === 60 * 16));
  assert.equal(tops.length, 16);
});

test('the same block gets the same variant in areas with different origins', () => {
  const a = meshBlocks(ctx, litter(8), tints(8), 8, 0, { origin: [100, 40] });
  const b = meshBlocks(ctx, litter(4), tints(4), 4, 0, { origin: [102, 42] });
  const uvs = (m, o) => { const r = new Map(); for (let i = 0; i < m.cutout.faces; i++) { const v = verts(m.cutout, i);
    r.set(`${Math.floor(v[0].x / 16) + o[0]},${Math.floor(v[0].z / 16) + o[1]}`, v.map((q) => q.uv).join()); } return r; };
  const ma = uvs(a, [100, 40]);
  for (const [k, v] of uvs(b, [102, 42])) assert.equal(ma.get(k), v, k);
});

test('a lily pad is one leaf-tinted quad 1/16 above the water surface it floats on', () => {
  const size = 4, blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) {
    for (let y = 0; y < 60; y++) blocks[c * H + y] = B.stone;
    blocks[c * H + 60] = B.water;
    blocks[c * H + 61] = B.lily_pad;
  }
  const m = meshBlocks(ctx, blocks, tints(size), size, 0);
  assert.equal(m.cutout.faces, 16);
  for (let i = 0; i < 16; i++) for (const v of verts(m.cutout, i)) {
    assert.equal(v.y, 61 * 16 + 1);
    assert.deepEqual(v.t, [128, 64, 26]);
  }
});
