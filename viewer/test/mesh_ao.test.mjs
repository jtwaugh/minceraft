import test from 'node:test';
import assert from 'node:assert/strict';
import { meshBlocks, B, H, VERTEX_BYTES, AO_SHADE } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// Blocks for a size x size area at world `origin` with a `pad` ring, from
// height(wx, wz) (stone up to but excluding that y) plus `extra` [wx, y, wz] blocks.
function area(height, origin, size, pad, extra = []) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const wx = origin[0] + x - pad, wz = origin[1] + z - pad;
    for (let y = 0; y < height(wx, wz); y++) blocks[(z * w + x) * H + y] = B.stone;
    for (const [ex, ey, ez] of extra) if (ex === wx && ez === wz) blocks[(z * w + x) * H + ey] = B.stone;
  }
  return blocks;
}

// Quads of the opaque mesh as [{ verts: [{ x, y, z, ao }] }] in world coordinates.
function quads(m, origin) {
  const dv = new DataView(m.opaque.verts), out = [];
  for (let q = 0; q < m.opaque.faces; q++) {
    const verts = [0, 1, 2, 3].map((j) => {
      const a = (q * 4 + j) * VERTEX_BYTES;
      return { x: dv.getInt16(a, true) / 16 + origin[0], y: dv.getInt16(a + 2, true) / 16,
        z: dv.getInt16(a + 4, true) / 16 + origin[1], ao: dv.getUint8(a + 13) };
    });
    out.push(verts);
  }
  return out;
}
const isTop = (v, y) => v.every((p) => p.y === y);
const mesh = (blocks, size, pad, origin = [0, 0]) => meshBlocks(ctx, blocks, tints(size), size, pad, { origin, greedy: false });
const aoAt = (q, x, z) => q.find((p) => p.x === x && p.z === z).ao;

test('AO_SHADE has four levels, brightest first', () => {
  assert.equal(AO_SHADE.length, 4);
  assert.equal(AO_SHADE[0], 1);
  for (let i = 1; i < 4; i++) assert.ok(AO_SHADE[i] < AO_SHADE[i - 1]);
});

test('a flat floor has AO 0 on every vertex', () => {
  const m = mesh(area(() => 60, [0, 0], 8, 1), 8, 1);
  for (const q of quads(m, [0, 0])) for (const p of q) assert.equal(p.ao, 0);
});

test('an inner corner counts the side and diagonal neighbours', () => {
  // floor top at y 60; a wall of two blocks at (4,60,3) and (3,60,4), a third at (4,60,4)
  const wall = [[4, 60, 3], [3, 60, 4]];
  let m = mesh(area(() => 60, [0, 0], 8, 0, wall), 8, 0);
  let q = quads(m, [0, 0]).find((v) => isTop(v, 60) && v.some((p) => p.x === 3 && p.z === 3) && v.some((p) => p.x === 4 && p.z === 4));
  assert.equal(aoAt(q, 4, 4), 2); // both sides, diagonal open
  assert.equal(aoAt(q, 3, 3), 0);
  m = mesh(area(() => 60, [0, 0], 8, 0, [...wall, [4, 60, 4]]), 8, 0);
  q = quads(m, [0, 0]).find((v) => isTop(v, 60) && v.some((p) => p.x === 3 && p.z === 3) && v.some((p) => p.x === 4 && p.z === 4));
  assert.equal(aoAt(q, 4, 4), 3); // both sides and the diagonal
});

test('the diagonal flips so the dark corner lies on it', () => {
  // one block at (4,60,3): the floor block (3,59,4) is dimmed at its (4,4) corner
  const m = mesh(area(() => 60, [0, 0], 8, 0, [[4, 60, 3]]), 8, 0);
  const all = quads(m, [0, 0]).filter((v) => isTop(v, 60));
  for (const v of all) assert.ok(v[0].ao + v[2].ao >= v[1].ao + v[3].ao);
  const q = all.find((v) => v.some((p) => p.x === 3 && p.z === 4) && v.some((p) => p.x === 4 && p.z === 5));
  const dark = q.findIndex((p) => p.ao === 1);
  assert.equal(q[dark].x, 4); assert.equal(q[dark].z, 4);
  assert.ok(dark === 0 || dark === 2);
  assert.equal(q.filter((p) => p.ao > 0).length, 1);
});

test('cross plants keep AO 0', () => {
  const blocks = area(() => 60, [0, 0], 8, 1, [[4, 60, 3]]);
  const w = 10;
  blocks[((4 + 1) * w + 5 + 1) * H + 60] = B.tall_grass;
  const m = mesh(blocks, 8, 1);
  assert.equal(m.cutout.faces, 4);
  const verts = new DataView(m.cutout.verts);
  for (let i = 0; i < m.cutout.faces * 4; i++) assert.equal(verts.getUint8(i * VERTEX_BYTES + 13), 0);
});

test('the same block gets the same AO from areas with different origins and pads', () => {
  const height = (x, z) => 58 + ((Math.imul(x, 73856093) ^ Math.imul(z, 19349663)) >>> 7) % 4;
  const a = mesh(area(height, [100, -40], 8, 1), 8, 1, [100, -40]);
  const b = mesh(area(height, [103, -40], 4, 1), 4, 1, [103, -40]);
  const key = (v) => v.map((p) => `${p.x},${p.y},${p.z}`).sort().join(' ');
  const inA = new Map(quads(a, [100, -40]).map((v) => [key(v), v]));
  const qb = quads(b, [103, -40]).filter((v) => v.every((p) => p.y >= 56));
  assert.ok(qb.length > 0);
  let dim = 0;
  for (const v of qb) {
    const u = inA.get(key(v));
    assert.ok(u, key(v));
    for (const p of v) assert.equal(u.find((r) => r.x === p.x && r.y === p.y && r.z === p.z).ao, p.ao);
    dim += v.filter((p) => p.ao > 0).length;
  }
  assert.ok(dim > 0, 'terrain has some occlusion');
});
