import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H, VERTEX_BYTES, meshBlocks } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2).fill(3) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([0.5, 0.25, 0.1]) });
const verts = (q, i) => { const dv = new DataView(q.verts); return [0, 1, 2, 3].map((j) => { const o = (i * 4 + j) * VERTEX_BYTES;
  return { x: dv.getInt16(o, true), y: dv.getInt16(o + 2, true), z: dv.getInt16(o + 4, true), t: [10, 11, 12].map((k) => dv.getUint8(o + k)) }; }); };
function field(size) {
  const blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) {
    for (let y = 0; y < 60; y++) blocks[c * H + y] = B.stone;
    blocks[c * H + 60] = B.berry_bush;
  }
  return blocks;
}

test('a berry bush is a leaf-tinted cutout cube inside its cell, strictly inside on the sides', () => {
  const m = meshBlocks(ctx, field(1), tints(1), 1, 0);
  assert.equal(m.cutout.faces, 5);
  for (let i = 0; i < 5; i++) for (const v of verts(m.cutout, i)) {
    assert.ok(v.x > 0 && v.x < 16 && v.z > 0 && v.z < 16, 'sides inset');
    assert.ok(v.y >= 60 * 16 && v.y < 61 * 16, 'top inset');
    assert.deepEqual(v.t, [128, 64, 26]);
  }
});

test('bushes do not hide the solid faces next to them', () => {
  const m = meshBlocks(ctx, field(4), tints(4), 4, 0);
  const tops = [...Array(m.opaque.faces).keys()].filter((i) => verts(m.opaque, i).every((v) => v.y === 60 * 16));
  assert.equal(tops.length, 16);
});

test('a plain bush meshes like a berry bush', () => {
  const mesh = (id) => {
    const blocks = field(1);
    blocks[60] = id;
    return meshBlocks(ctx, blocks, tints(1), 1, 0).cutout;
  };
  const [a, b] = [mesh(B.berry_bush), mesh(B.bush)];
  assert.equal(b.faces, 5);
  for (let i = 0; i < 5; i++) assert.deepEqual(verts(b, i), verts(a, i));
});
