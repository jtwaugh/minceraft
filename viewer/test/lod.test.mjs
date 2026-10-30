import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { meshBlocks, legendContext, columnTints, B, H, VERTEX_BYTES } from '../mesh.js';
import { meshBlocksWasm } from '../meshwasm.js';
import { downsampleBlocks, meshHalf } from '../lod.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// One 2x2x2 cell at the origin holding `list` (up to 8 blocks, rest air).
function cell(list) {
  const blocks = new Uint8Array(2 * 2 * H);
  let i = 0;
  for (let z = 0; z < 2; z++) for (let x = 0; x < 2; x++) for (let y = 0; y < 2; y++) blocks[(z * 2 + x) * H + y] = list[i++] ?? 0;
  return downsampleBlocks(blocks, 2, 0).cells[0];
}

test('a cell takes the most common non-air block', () => {
  assert.equal(cell([B.dirt, B.stone, B.stone, B.air, B.air, B.air, B.air, B.air]), B.stone);
  assert.equal(cell([B.stone, B.dirt, B.dirt, B.dirt, B.stone, 0, 0, 0]), B.dirt);
});

test('ties go to the lowest id', () => {
  assert.equal(cell([B.sand, B.stone, B.sand, B.stone]), B.stone);
  assert.equal(cell([B.water, B.dirt]), B.dirt);
});

test('all air stays air, and water beats air', () => {
  assert.equal(cell([]), B.air);
  assert.equal(cell([B.water, 0, 0, 0, 0, 0, 0, 0]), B.water);
});

// Hilly world: stone to a height from a hash of the world column.
function world(x0, z0, n) {
  const blocks = new Uint8Array(n * n * H);
  for (let z = 0; z < n; z++) for (let x = 0; x < n; x++) {
    const wx = x0 + x, wz = z0 + z, h = 40 + ((Math.imul(wx >> 1, 7919) ^ Math.imul(wz >> 1, 104729)) & 7) * 2;
    for (let y = 0; y < h; y++) blocks[(z * n + x) * H + y] = y === h - 1 ? B.grass : B.stone;
  }
  return blocks;
}

test('a 64x64 chunk has about a quarter of the faces of full detail', () => {
  const blocks = world(0, 0, 66), full = meshBlocks(ctx, blocks, tints(64), 64, 1);
  const half = meshHalf(ctx, blocks, tints(64), 64, 1);
  const f = (m) => m.opaque.faces + m.cutout.faces + m.water.faces + m.ice.faces;
  const ratio = f(half) / f(full);
  assert.ok(ratio > 0.15 && ratio < 0.4, `ratio ${ratio}`);
});

test('vertices keep the format and stay in range', () => {
  const m = meshHalf(ctx, world(0, 0, 66), tints(64), 64, 1);
  assert.equal(m.opaque.verts.byteLength, m.opaque.faces * 4 * VERTEX_BYTES);
  const dv = new DataView(m.opaque.verts);
  for (let v = 0; v < m.opaque.faces * 4; v++) {
    const o = v * VERTEX_BYTES;
    for (let j = 0; j < 3; j++) {
      const p = dv.getInt16(o + j * 2, true) / 16;
      assert.ok(p >= 0 && p <= 128, `pos ${p}`);
    }
    assert.ok(dv.getUint8(o + 6) <= 240 && dv.getUint8(o + 7) <= 240);
  }
  const dvPos = (i) => dv.getInt16(i, true) / 16;
  assert.equal(dvPos(0) % 1, 0);
});

test('overlapping regions agree on shared cells', () => {
  // two 16x16 areas, the second starting 8 columns later; compare the shared 8x16 strip
  const a = meshHalf(ctx, world(0, 0, 18), tints(16), 16, 1, { origin: [0, 0], greedy: false });
  const b = meshHalf(ctx, world(8, 0, 18), tints(16), 16, 1, { origin: [8, 0], greedy: false });
  const quads = (m, dx) => {
    const dv = new DataView(m.opaque.verts), set = new Set();
    for (let q = 0; q < m.opaque.faces; q++) {
      const o = q * 4 * VERTEX_BYTES, k = [];
      for (let j = 0; j < 4; j++) {
        const p = o + j * VERTEX_BYTES;
        k.push([dv.getInt16(p, true) / 16 + dx, dv.getInt16(p + 2, true) / 16, dv.getInt16(p + 4, true) / 16, dv.getUint8(p + 6), dv.getUint8(p + 7), dv.getUint8(p + 8)].join(':'));
      }
      const xs = k.map((s) => +s.split(':')[0]);
      if (Math.min(...xs) >= 10 && Math.max(...xs) <= 14) set.add(k.join('|'));
    }
    return set;
  };
  const qa = quads(a, 0), qb = quads(b, 8);
  assert.ok(qa.size > 0);
  assert.deepEqual([...qa].sort(), [...qb].sort());
});

test('meshHalf with the wasm mesher matches the JS one on a real chunk', () => {
  const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
  const real = { ...legendContext(wasm.legend()), faceLayer };
  for (const [seed, x, z] of [[12345n, 0, 0], [9n, -128, 64], [12345n, 2048, 1024]]) {
    const blocks = wasm.generate_region_decorated(seed, x - 1, z - 1, 66, 66);
    const t = columnTints(wasm, real, seed, x, z, 64);
    const js = meshHalf(real, blocks, t, 64, 1, { origin: [x, z] });
    const rs = meshHalf(real, blocks, t, 64, 1, { origin: [x, z], mesher: (...a) => meshBlocksWasm(wasm, ...a) });
    for (const k of ['opaque', 'cutout', 'water', 'ice']) {
      assert.equal(rs[k].faces, js[k].faces, `${k} at ${x}, ${z}`);
      assert.deepEqual(new Uint8Array(rs[k].verts), new Uint8Array(js[k].verts), `${k} at ${x}, ${z}`);
    }
    assert.equal(rs.meanY, js.meanY);
    assert.equal(rs.minY, js.minY);
  }
});
