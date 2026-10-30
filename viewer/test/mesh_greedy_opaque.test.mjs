import test from 'node:test';
import assert from 'node:assert/strict';
import { meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2).map((_, i) => i % 251) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// Hilly terrain from a hash of the world column: stone, dirt and sand layers, so
// there are flat plateaus, walls and ambient-occlusion gradients.
function terrain(x0, z0, size, pad) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const wx = x0 + x - pad, wz = z0 + z - pad;
    const h = 40 + ((Math.imul(wx >> 2, 7919) ^ Math.imul(wz >> 2, 104729)) >>> 5) % 4;
    for (let y = 0; y < h; y++) blocks[(z * w + x) * H + y] = y >= h - 2 ? (h % 2 ? B.dirt : B.sand) : B.stone;
  }
  return blocks;
}

const vertex = (dv, q, j) => {
  const o = (q * 4 + j) * VERTEX_BYTES;
  return { p: [0, 2, 4].map((k) => dv.getInt16(o + k, true)), uv: [dv.getUint8(o + 6), dv.getUint8(o + 7)],
    layer: dv.getUint8(o + 8), shade: dv.getUint8(o + 9), tint: [10, 11, 12].map((k) => dv.getUint8(o + k)).join(), ao: dv.getUint8(o + 13) };
};
// Split every quad into unit cells: Map "normal|min corner (world, 1/16)" -> attributes, with
// the per-corner position, AO and UV (UV taken relative to the cell, so a repeat doesn't matter).
function cells(part, origin = [0, 0]) {
  const dv = new DataView(part.verts), m = new Map();
  for (let q = 0; q < part.faces; q++) {
    const v = [0, 1, 2, 3].map((j) => vertex(dv, q, j));
    const e1 = v[1].p.map((a, i) => a - v[0].p[i]), e3 = v[3].p.map((a, i) => a - v[0].p[i]);
    const n = [e1[1] * e3[2] - e1[2] * e3[1], e1[2] * e3[0] - e1[0] * e3[2], e1[0] * e3[1] - e1[1] * e3[0]].map(Math.sign).join();
    const len = (e) => Math.max(...e.map(Math.abs)) / 16;
    const [nu, nv] = [len(e1), len(e3)];
    assert.ok(nu <= 15 && nv <= 15 && Number.isInteger(nu) && Number.isInteger(nv), 'rectangle of whole blocks, at most 15');
    for (let j = 0; j < nv; j++) for (let i = 0; i < nu; i++) {
      // the cell's corners in quad parameter space (s, t)
      const corner = ([s, t]) => {
        return { p: v[0].p.map((a, k) => a + (e1[k] / nu) * (i + s) + (e3[k] / nv) * (j + t)),
          uv: v[0].uv.map((a, k) => a + ((v[1].uv[k] - a) / nu) * (i + s) + ((v[3].uv[k] - a) / nv) * (j + t)) };
      };
      const cs = [[0, 0], [1, 0], [1, 1], [0, 1]].map(corner);
      const lo = [0, 1, 2].map((k) => Math.min(...cs.map((c) => c.p[k])));
      const uvLo = [0, 1].map((k) => 16 * Math.floor(Math.min(...cs.map((c) => c.uv[k])) / 16));
      const key = `${n}|${lo[0] + origin[0] * 16},${lo[1]},${lo[2] + origin[1] * 16}`;
      assert.ok(!m.has(key), `cell ${key} covered twice`);
      const corners = cs.map((c, k) => `${c.p.map((a, d) => a - lo[d]).join()}:${c.uv.map((a, d) => a - uvLo[d]).join()}:${[v[0], v[1], v[2], v[3]][k].ao}`);
      m.set(key, `${v[0].layer},${v[0].shade},${v[0].tint},${corners.sort().join('|')}`);
    }
  }
  return m;
}

test('merged output covers the same faces with the same attributes as the unmerged mesh', () => {
  const size = 40, blocks = terrain(0, 0, size, 1);
  const merged = meshBlocks(ctx, blocks, tints(size), size, 1).opaque;
  const plain = meshBlocks(ctx, blocks, tints(size), size, 1, { greedy: false }).opaque;
  const a = cells(merged), b = cells(plain);
  assert.equal(a.size, b.size);
  for (const [k, v] of b) assert.equal(a.get(k), v, k);
  assert.ok(merged.faces < plain.faces, `${merged.faces} < ${plain.faces}`);
});

test('a flat run of an unvaried block merges into quads of at most 15 blocks a side', () => {
  const size = 40, blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) for (let y = 0; y < 30; y++) blocks[c * H + y] = B.dirt;
  const m = meshBlocks(ctx, blocks, tints(size), size, 0).opaque;
  // 40 = 15 + 15 + 10 a side: 9 top quads, at most 4 on each wall's single row
  assert.ok(m.faces <= 9 + 9 + 4 * 3 * 3 + 4 * 9, `${m.faces} faces`);
  assert.equal(cells(m).size, cells(meshBlocks(ctx, blocks, tints(size), size, 0, { greedy: false }).opaque).size);
});

test('overlapping areas agree on the faces of the blocks they share', () => {
  const a = meshBlocks(ctx, terrain(100, -40, 24, 1), tints(24), 24, 1, { origin: [100, -40] }).opaque;
  const b = meshBlocks(ctx, terrain(108, -40, 12, 1), tints(12), 12, 1, { origin: [108, -40] }).opaque;
  const ca = cells(a, [100, -40]), cb = cells(b, [108, -40]);
  let shared = 0;
  for (const [k, v] of cb) {
    const [bx, , bz] = k.split('|')[1].split(',').map(Number);
    if (bx < 109 * 16 || bx > 118 * 16 || bz < -39 * 16 || bz > -29 * 16) continue; // inner blocks of b only
    assert.equal(ca.get(k), v, k);
    shared++;
  }
  assert.ok(shared > 50);
});

test('a block with an AO gradient is not merged into its neighbours', () => {
  const size = 8, blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) for (let y = 0; y < 30; y++) blocks[c * H + y] = B.dirt;
  blocks[(3 * size + 4) * H + 30] = B.dirt; // a post: its neighbours' tops get corner AO
  const m = meshBlocks(ctx, blocks, tints(size), size, 0).opaque;
  const plain = meshBlocks(ctx, blocks, tints(size), size, 0, { greedy: false }).opaque;
  const a = cells(m), b = cells(plain);
  for (const [k, v] of b) assert.equal(a.get(k), v, k);
});
