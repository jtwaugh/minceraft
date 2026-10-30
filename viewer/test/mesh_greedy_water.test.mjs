import test from 'node:test';
import assert from 'node:assert/strict';
import { meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2).map((_, i) => i % 251) };
const tints = (n, water) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]), water });

// Stone to y = 1, water to y = 63 (top block at y = 62 -> top face at 63); `edit` may change blocks.
function ocean(size, pad, edit = () => null) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const i = (z * w + x) * H;
    blocks[i] = B.stone;
    for (let y = 1; y < 63; y++) blocks[i + y] = B.water;
    const e = edit(x - pad, z - pad);
    if (e !== null) for (let y = 1; y < 63; y++) blocks[i + y] = e;
  }
  return blocks;
}
const quads = (part) => {
  const dv = new DataView(part.verts), out = [];
  for (let q = 0; q < part.faces; q++) {
    const vs = [0, 1, 2, 3].map((j) => {
      const o = (q * 4 + j) * VERTEX_BYTES;
      return { x: dv.getInt16(o, true), y: dv.getInt16(o + 2, true), z: dv.getInt16(o + 4, true),
        u: dv.getUint8(o + 6), v: dv.getUint8(o + 7), layer: dv.getUint8(o + 8), shade: dv.getUint8(o + 9),
        tint: [10, 11, 12].map((k) => dv.getUint8(o + k)).join(), depth: dv.getUint8(o + 15) };
    });
    out.push(vs);
  }
  return out;
};
// Up-facing quads (all four vertices at one y; sides vary in y).
const tops = (part) => quads(part).filter((q) => q.every((v) => v.y === q[0].y) && q[0].y === 63 * 16);
// Covered cells -> attributes.
function cells(part) {
  const m = new Map();
  for (const q of tops(part)) {
    const x0 = Math.min(...q.map((v) => v.x)), x1 = Math.max(...q.map((v) => v.x));
    const z0 = Math.min(...q.map((v) => v.z)), z1 = Math.max(...q.map((v) => v.z));
    for (let z = z0; z < z1; z += 16) for (let x = x0; x < x1; x += 16) {
      const k = `${x / 16},${z / 16}`;
      assert.ok(!m.has(k), 'cell covered twice');
      m.set(k, `${q[0].layer},${q[0].shade},${q[0].tint},${q[0].depth}`);
    }
  }
  return m;
}
const unmerged = (size, pad, blocks) => {
  // one-block-at-a-time reference: mesh each column alone is overkill; count cells instead
  const m = new Map();
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    const b = blocks[((z + pad) * (size + 2 * pad) + x + pad) * H + 62];
    if (b === B.water) m.set(`${x},${z}`, true);
  }
  return m;
};

test('flat ocean tops collapse to under 5% of the faces', () => {
  const size = 32, blocks = ocean(size, 1);
  const m = meshBlocks(ctx, blocks, tints(size), size, 1);
  assert.ok(tops(m.water).length <= size * size * 0.05);
  assert.equal(cells(m.water).size, size * size);
});

test('merged quads cover the same cells with the same attributes', () => {
  const size = 20, blocks = ocean(size, 1, (x, z) => (x === 7 && z === 3 ? B.stone : null));
  const m = meshBlocks(ctx, blocks, tints(size), size, 1);
  const c = cells(m.water), ref = unmerged(size, 1, blocks);
  assert.deepEqual([...c.keys()].sort(), [...ref.keys()].sort());
  assert.equal(new Set(c.values()).size, 1);
  assert.ok(!c.has('7,3'));
});

test('a differing tint is not merged across', () => {
  const size = 4, n = size * size, water = Array(n).fill([0.2, 0.4, 0.6]);
  water[1] = [0.9, 0.4, 0.6];
  const m = meshBlocks(ctx, ocean(size, 0), tints(size, water), size, 0);
  const c = cells(m.water);
  assert.equal(c.size, n);
  assert.equal(new Set(c.values()).size, 2);
});

test('quads never exceed 15 blocks a side', () => {
  const size = 40, m = meshBlocks(ctx, ocean(size, 0), tints(size), size, 0);
  for (const q of tops(m.water)) {
    assert.ok(Math.max(...q.map((v) => v.x)) - Math.min(...q.map((v) => v.x)) <= 15 * 16);
    assert.ok(Math.max(...q.map((v) => v.z)) - Math.min(...q.map((v) => v.z)) <= 15 * 16);
    assert.ok(Math.max(...q.map((v) => v.u)) <= 15 * 16 && Math.max(...q.map((v) => v.v)) <= 15 * 16);
  }
});

test('tops next to ice are culled as before', () => {
  const size = 4, blocks = ocean(size, 0);
  blocks[(1 * size + 1) * H + 62] = B.ice;
  const m = meshBlocks(ctx, blocks, tints(size), size, 0);
  const c = cells(m.water);
  assert.ok(!c.has('1,1'));
  assert.equal(c.size, size * size - 1);
});
