import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H } from '../mesh.js';
import { shadeFractions } from '../shade.js';

// A synthetic world: grass at y = 60 everywhere, and leaf blocks at y = 66 on
// columns picked by a hash of the world coordinate (or by `leaves`).
const hash = (x, z) => (Math.imul(x, 73856093) ^ Math.imul(z, 19349663)) >>> 0;
const hashed = (x, z) => hash(x, z) % 7 === 0;
function area(wx, wz, size, pad, leaves = hashed) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const c = (z * w + x) * H;
    for (let y = 0; y < 60; y++) blocks[c + y] = B.dirt;
    blocks[c + 60] = B.grass;
    if (leaves(wx - pad + x, wz - pad + z)) blocks[c + 66] = B.oak_leaves;
  }
  return blocks;
}
const one = (x, z) => x === 0 && z === 0;

test('a column under one leaf block is partly shaded, an open column far away is not', () => {
  const size = 16, pad = 1, f = shadeFractions(area(-8, -8, size, pad, one), size, pad, 1);
  const under = f[8 * size + 8];
  assert.ok(under > 0 && under < 1);
  assert.equal(under, 1 / 9);
  assert.equal(f[8 * size + 9], 1 / 9);
  assert.equal(f[0], 0);
  assert.equal(f[15 * size + 15], 0);
});

test('radius 0 gives the binary flag', () => {
  const size = 12, pad = 0, blocks = area(3, -5, size, pad);
  const f = shadeFractions(blocks, size, pad, 0);
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) assert.equal(f[z * size + x], hashed(3 + x, -5 + z) ? 1 : 0);
});

test('fractions are integer counts over the window', () => {
  const size = 10, r = 2, f = shadeFractions(area(0, 0, size, 2), size, 2, r);
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    let count = 0;
    for (let dz = -r; dz <= r; dz++) for (let dx = -r; dx <= r; dx++) count += hashed(x + dx, z + dz) ? 1 : 0;
    assert.equal(f[z * size + x], count / 25);
  }
});

test('leaves below the top grass and columns with no grass do not shade', () => {
  const size = 3, pad = 0, blocks = area(0, 0, size, pad, () => false);
  const mid = (1 * size + 1) * H;
  blocks[mid + 50] = B.birch_leaves; // under the grass
  const bare = (0 * size + 0) * H;
  blocks[bare + 60] = B.sand; blocks[bare + 70] = B.spruce_leaves; // leaves but no grass
  assert.deepEqual([...shadeFractions(blocks, size, pad, 0)], Array(9).fill(0));
});

test('the window is clipped at the padded array edge', () => {
  const size = 4, pad = 0, f = shadeFractions(area(0, 0, size, pad, one), size, pad, 1);
  assert.equal(f[0], 1 / 4);
  assert.equal(f[1], 1 / 6);
  assert.equal(f[size + 1], 1 / 9);
});

test('overlapping areas with different origins and sizes agree where the window fits in both', () => {
  const r = 2;
  const a = { wx: 100, wz: -60, size: 24, pad: 2 }, b = { wx: 108, wz: -52, size: 17, pad: 3 };
  const fa = shadeFractions(area(a.wx, a.wz, a.size, a.pad), a.size, a.pad, r);
  const fb = shadeFractions(area(b.wx, b.wz, b.size, b.pad), b.size, b.pad, r);
  const inside = (s, x, z) => x - r >= s.wx - s.pad && x + r < s.wx + s.size + s.pad && z - r >= s.wz - s.pad && z + r < s.wz + s.size + s.pad;
  let shared = 0, shaded = 0;
  for (let z = b.wz; z < b.wz + b.size; z++) for (let x = b.wx; x < b.wx + b.size; x++) {
    if (x >= a.wx + a.size || z >= a.wz + a.size || !inside(a, x, z) || !inside(b, x, z)) continue;
    const va = fa[(z - a.wz) * a.size + x - a.wx], vb = fb[(z - b.wz) * b.size + x - b.wx];
    assert.equal(va, vb, `column ${x}, ${z}`);
    shared++; if (va > 0) shaded++;
  }
  assert.ok(shared > 100 && shaded > 10);
});

test('placing or removing a leaf block changes the result', () => {
  const size = 8, pad = 1, blocks = area(0, 0, size, pad, () => false);
  const before = shadeFractions(blocks, size, pad, 1);
  const c = ((4 + pad) * (size + 2 * pad) + 4 + pad) * H;
  blocks[c + 80] = B.oak_leaves;
  const placed = shadeFractions(blocks, size, pad, 1);
  assert.notDeepEqual(placed, before);
  assert.equal(placed[4 * size + 4], 1 / 9);
  blocks[c + 80] = B.air;
  assert.deepEqual(shadeFractions(blocks, size, pad, 1), before);
});
