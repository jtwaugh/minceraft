import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });

const ctx = legendContext(wasm.legend(), new Uint8Array(256 * 3 * 2).fill(3));
const tints = (n) => ({ grass: Array.from({ length: n * n }, () => [1, 1, 1]), leaf: Array.from({ length: n * n }, () => [1, 1, 1]) });
function meadow(size) {
  const blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) {
    for (let y = 0; y < 60; y++) blocks[c * H + y] = B.stone;
    blocks[c * H + 60] = B.tall_grass;
  }
  return blocks;
}

// Per world column: [min, max] vertex position in x and z (1/16 of a block).
// A quad's first two vertices average to the column centre whatever the offset.
function spans(size, origin) {
  const m = meshBlocks(ctx, meadow(size), tints(size), size, 0, { origin });
  const dv = new DataView(m.cutout.verts), out = new Map();
  for (let i = 0; i < m.cutout.faces * 4; i += 4) {
    const o = i * VERTEX_BYTES;
    const cx = Math.floor((dv.getInt16(o, true) + dv.getInt16(o + VERTEX_BYTES, true)) / 32);
    const cz = Math.floor((dv.getInt16(o + 4, true) + dv.getInt16(o + VERTEX_BYTES + 4, true)) / 32);
    const key = `${cx + origin[0]},${cz + origin[1]}`;
    const s = out.get(key) ?? { x: [Infinity, -Infinity], z: [Infinity, -Infinity], col: [cx, cz] };
    for (let j = 0; j < 4; j++) {
      const x = dv.getInt16(o + j * VERTEX_BYTES, true), z = dv.getInt16(o + j * VERTEX_BYTES + 4, true);
      s.x = [Math.min(s.x[0], x), Math.max(s.x[1], x)]; s.z = [Math.min(s.z[0], z), Math.max(s.z[1], z)];
    }
    out.set(key, s);
  }
  return out;
}

test('plant stays inside its column and offsets vary', () => {
  const s = spans(16, [0, 0]);
  const shifts = new Set();
  for (const v of s.values()) {
    const [cx, cz] = v.col;
    assert.ok(v.x[0] >= cx * 16 && v.x[1] <= cx * 16 + 16, 'x inside');
    assert.ok(v.z[0] >= cz * 16 && v.z[1] <= cz * 16 + 16, 'z inside');
    shifts.add(v.x[0] - cx * 16 - 2); shifts.add(v.z[0] - cz * 16 - 2);
  }
  assert.ok(shifts.size > 1);
  for (const d of shifts) assert.ok(Number.isInteger(d) && Math.abs(d) <= 3);
});

test('same plant gets the same offset in areas with different origins', () => {
  const a = spans(16, [0, 0]), b = spans(16, [5, -3]);
  let shared = 0;
  for (const [k, v] of b) {
    const w = a.get(k);
    if (!w) continue;
    shared++;
    assert.equal(v.x[0] - v.col[0] * 16, w.x[0] - w.col[0] * 16);
    assert.equal(v.z[0] - v.col[1] * 16, w.z[0] - w.col[1] * 16);
  }
  assert.ok(shared > 50);
});

// Per world column: top/base vertex heights (1/16), tints and u values.
function shapes(size, origin, block = B.tall_grass) {
  const blocks = meadow(size);
  for (let c = 0; c < size * size; c++) blocks[c * H + 60] = block;
  const m = meshBlocks(ctx, blocks, tints(size), size, 0, { origin });
  const dv = new DataView(m.cutout.verts), out = new Map();
  for (let i = 0; i < m.cutout.faces * 4; i += 4) {
    const q = i * VERTEX_BYTES;
    // the first two vertices of a quad average to the column centre
    const cx = Math.floor((dv.getInt16(q, true) + dv.getInt16(q + VERTEX_BYTES, true)) / 32);
    const cz = Math.floor((dv.getInt16(q + 4, true) + dv.getInt16(q + VERTEX_BYTES + 4, true)) / 32);
    const key = `${cx + origin[0]},${cz + origin[1]}`;
    const s = out.get(key) ?? { top: [], base: [], u: [] };
    for (let j = 0; j < 4; j++) {
      const o = q + j * VERTEX_BYTES;
      const v = { y: dv.getInt16(o + 2, true) - 60 * 16, t: dv.getUint8(o + 10), u: dv.getUint8(o + 6) };
      (dv.getUint8(o + 14) ? s.top : s.base).push(v);
      s.u.push(v.u);
    }
    out.set(key, s);
  }
  return out;
}

test('tall grass height, base darkening, jitter and mirroring', () => {
  const s = shapes(16, [0, 0]);
  const heights = new Set(), mirrored = new Set();
  for (const v of s.values()) {
    const top = v.top[0].y;
    assert.ok(top >= 12 && top <= 16);
    heights.add(top);
    assert.ok(v.top.every((q) => q.y === top) && v.base.every((q) => q.y === 0));
    const tt = v.top[0].t, bt = v.base[0].t;
    assert.ok(tt <= 255 && Math.abs(tt - 255) <= 255 * 0.07 + 1);
    assert.ok(Math.abs(bt / tt - 0.8) < 0.02);
    mirrored.add(v.u[0]);
  }
  assert.ok(heights.size > 2 && mirrored.size > 1);
});

test('other plants keep full height', () => {
  for (const v of shapes(8, [0, 0], B.dandelion).values()) assert.ok(v.top.every((q) => q.y === 16));
});

test('plant shape is the same from different origins', () => {
  const a = shapes(16, [0, 0]), b = shapes(16, [5, -3]);
  let shared = 0;
  for (const [k, v] of b) {
    const w = a.get(k);
    if (!w) continue;
    shared++;
    assert.deepEqual(v, w);
  }
  assert.ok(shared > 50);
});
