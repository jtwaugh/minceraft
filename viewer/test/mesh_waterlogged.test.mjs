// Waterlogged cells (sea-flora-and-shellfish 2, Meshing): drawn as water plus
// a cross plant, with no faces between water and waterlogged cells, and the
// same bytes from the Rust mesher (via wasm) as from meshBlocks.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';
import { meshBlocksWasm } from '../meshwasm.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });

const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
const ctx = { COLORS: [], faceLayer };
ctx.COLORS[B.birch_leaves] = [150, 190, 90];
ctx.COLORS[B.spruce_leaves] = [60, 110, 70];
const LOGGED = B.test_waterlogged_plant;
const white = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// Quads of a part as strings, vertex order kept.
function quads(p) {
  const u8 = new Uint8Array(p.verts), out = [];
  for (let q = 0; q < p.faces; q++) {
    const vs = [0, 1, 2, 3].map((j) => u8.slice((q * 4 + j) * VERTEX_BYTES, (q * 4 + j + 1) * VERTEX_BYTES));
    out.push(vs.map((v) => v.join()).join(';'));
  }
  return out.sort();
}

// One column: stone to 40, water 41..=44, waterlogged at 42 and at the surface 44 if `logged`.
function column(logged) {
  const b = new Uint8Array(H);
  for (let y = 0; y <= 44; y++) b[y] = y <= 40 ? B.stone : logged && (y === 42 || y === 44) ? LOGGED : B.water;
  return b;
}

test('a waterlogged column meshes as plain water plus the plants', () => {
  const opts = { greedy: false };
  const m = meshBlocks(ctx, column(true), white(1), 1, 0, opts);
  const w = meshBlocks(ctx, column(false), white(1), 1, 0, opts);
  // one top and 4 sides * 4 cells, nothing between water and waterlogged cells
  assert.equal(m.water.faces, 1 + 16);
  // the same water bytes as plain water: faces, top quad, tint and depth
  assert.deepEqual(quads(m.water), quads(w.water));
  const dv = new DataView(m.water.verts);
  const ys = Array.from({ length: m.water.faces * 4 }, (_, i) => dv.getInt16(i * VERTEX_BYTES + 2, true));
  const tops = Array.from({ length: m.water.faces }, (_, q) => ys.slice(q * 4, q * 4 + 4).every((y) => y === 45 * 16));
  assert.equal(tops.filter(Boolean).length, 1, 'one top quad at the surface');
  assert.equal(Math.max(...Array.from({ length: m.water.faces * 4 }, (_, i) => dv.getUint8(i * VERTEX_BYTES + 15))), 4);
  // four cross quads in each waterlogged cell, at y 42 and 44
  assert.equal(m.cutout.faces, 8);
  assert.equal(w.cutout.faces, 0);
  const cv = new DataView(m.cutout.verts);
  const cy = new Set(Array.from({ length: m.cutout.faces * 4 }, (_, i) => Math.floor(cv.getInt16(i * VERTEX_BYTES + 2, true) / 16)));
  assert.deepEqual([...cy].sort(), [42, 43, 44, 45]);
  assert.deepEqual([m.minY, m.meanY], [w.minY, w.meanY]);
  // and the Rust mesher draws the same
  const rs = meshBlocksWasm(wasm, ctx, column(true), white(1), 1, 0, opts);
  for (const k of ['opaque', 'cutout', 'water', 'ice']) assert.deepEqual(quads(rs[k]), quads(m[k]), k);
});

test('kelp meshes as plain water plus the plants, in both meshers', () => {
  // 3x3 pool: stone to 40, water 41..=44, kelp_0 at 42 and 43 and kelp_1 at 44 in the middle
  const pool = (kelp) => {
    const b = new Uint8Array(9 * H);
    for (let c = 0; c < 9; c++) for (let y = 0; y <= 44; y++) {
      const mid = kelp && c === 4;
      b[c * H + y] = y <= 40 ? B.stone : mid && (y === 42 || y === 43) ? B.kelp_0 : mid && y === 44 ? B.kelp_1 : B.water;
    }
    return b;
  };
  const opts = { greedy: false };
  const m = meshBlocks(ctx, pool(true), white(3), 3, 0, opts);
  const w = meshBlocks(ctx, pool(false), white(3), 3, 0, opts);
  // no faces between kelp and the water around it
  assert.deepEqual(quads(m.water), quads(w.water));
  assert.equal(m.cutout.faces, 12);
  assert.equal(w.cutout.faces, 0);
  const rs = meshBlocksWasm(wasm, ctx, pool(true), white(3), 3, 0, opts);
  for (const k of ['opaque', 'cutout', 'water', 'ice']) assert.deepEqual(quads(rs[k]), quads(m[k]), k);
});

for (const shell of ['oyster', 'scallop']) {
  test(`${shell} meshes as plain water plus a cluster, in both meshers`, () => {
    // 3x3 pool: stone to 40, water 41..=44, the shellfish on the floor at 41
    // and floating at 43 in the middle
    const pool = (logged) => {
      const b = new Uint8Array(9 * H);
      for (let c = 0; c < 9; c++) for (let y = 0; y <= 44; y++) {
        b[c * H + y] = y <= 40 ? B.stone : logged && c === 4 && (y === 41 || y === 43) ? B[shell] : B.water;
      }
      return b;
    };
    for (const origin of [[0, 0], [-7, 13]]) {
      const opts = { greedy: false, origin };
      const m = meshBlocks(ctx, pool(true), white(3), 3, 0, opts);
      const w = meshBlocks(ctx, pool(false), white(3), 3, 0, opts);
      // no faces between the cluster cells and the water around them
      assert.deepEqual(quads(m.water), quads(w.water));
      // 3 boxes of 5 faces on stone, of 6 faces in the water
      assert.equal(m.cutout.faces, 15 + 18);
      assert.equal(w.cutout.faces, 0);
      const dv = new DataView(m.cutout.verts);
      for (let i = 0; i < m.cutout.faces * 4; i++) {
        const p = (k) => dv.getInt16(i * VERTEX_BYTES + k, true);
        // inside the middle cell, at most 3/16 above its floor
        for (const k of [0, 4]) assert.ok(p(k) >= 16 + 2 && p(k) <= 32 - 2, `${p(k)}`);
        const floor = p(2) >= 43 * 16 ? 43 * 16 : 41 * 16;
        assert.ok(p(2) - floor >= 0 && p(2) - floor <= 3, `${p(2)}`);
      }
      const rs = meshBlocksWasm(wasm, ctx, pool(true), white(3), 3, 0, opts);
      for (const k of ['opaque', 'cutout', 'water', 'ice']) assert.deepEqual(quads(rs[k]), quads(m[k]), k);
    }
  });
}

test('wasm mesher matches meshBlocks on a region strewn with shellfish', () => {
  const size = 32, pad = 1, w = size + 2 * pad, origin = [2040, 1016];
  const plain = wasm.generate_region_decorated(12345n, origin[0] - pad, origin[1] - pad, w, w);
  const shells = [B.oyster, B.scallop];
  const blocks = plain.map((b, i) => (b === B.water && (i * 7 + (i >> 7) * 3) % 5 === 0 ? shells[i & 1] : b));
  const tints = randomTints(size, false);
  for (const greedy of [false, true]) {
    const js = meshBlocks(ctx, blocks, tints, size, pad, { greedy, origin });
    const rs = meshBlocksWasm(wasm, ctx, blocks, tints, size, pad, { greedy, origin });
    for (const k of ['opaque', 'cutout', 'water', 'ice']) assert.deepEqual(quads(rs[k]), quads(js[k]), k);
    assert.ok(js.cutout.faces > 100, `${js.cutout.faces} cutout quads`);
  }
});

// Reproducible tints that differ per column and per corner (exact in f32).
function randomTints(size, corners) {
  let a = 777;
  const rnd = () => Math.fround(((a = (a * 1103515245 + 12345) & 0x7fffffff) >> 8) / 0x800000);
  const cols = () => Array.from({ length: size * size }, () => [rnd(), rnd(), rnd()]);
  const grids = () => Float32Array.from({ length: (size + 1) * (size + 1) * 3 }, rnd);
  const t = { grass: cols(), leaf: cols(), water: cols(), dry: cols() };
  if (corners) t.corners = { n: size + 1, grass: grids(), leaf: grids(), water: grids(), dry: grids() };
  return t;
}

for (const corners of [false, true]) {
  for (const greedy of [false, true]) {
    test(`wasm mesher matches meshBlocks on a region with waterlogged cells${corners ? ', corners' : ''}${greedy ? ', greedy' : ''}`, () => {
      // shore of seed 12345 with every few water cells waterlogged
      const size = 32, pad = 1, w = size + 2 * pad, origin = [2040, 1016];
      const plain = wasm.generate_region_decorated(12345n, origin[0] - pad, origin[1] - pad, w, w);
      const blocks = plain.map((b, i) => (b === B.water && (i * 7 + (i >> 7) * 3) % 5 === 0 ? LOGGED : b));
      const tints = randomTints(size, corners);
      const before = meshBlocks(ctx, plain, tints, size, pad, { greedy, origin });
      const js = meshBlocks(ctx, blocks, tints, size, pad, { greedy, origin });
      const rs = meshBlocksWasm(wasm, ctx, blocks, tints, size, pad, { greedy, origin });
      assert.equal(rs.minY, js.minY);
      assert.equal(rs.meanY, js.meanY);
      for (const k of ['opaque', 'cutout', 'water', 'ice']) {
        assert.equal(rs[k].faces, js[k].faces, k);
        assert.deepEqual(quads(rs[k]), quads(js[k]), k);
      }
      // the plants are drawn, and the water is the same as without them
      assert.ok(js.cutout.faces > before.cutout.faces + 100, `${js.cutout.faces} cutout quads, ${before.cutout.faces} before`);
      assert.deepEqual(quads(js.water), quads(before.water));
    });
  }
}
