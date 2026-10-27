import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, columnTints, meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });
const ctx = { ...legendContext(wasm.legend()), faceLayer: new Uint8Array(256 * 3 * 2) };
const SEED = 12345n;

// A flat area of `top` over stone at y 0..59, no pad ring.
function flat(top, size) {
  const blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) for (let y = 0; y < 60; y++) blocks[c * H + y] = y === 59 ? top : B.stone;
  return blocks;
}

// Tints of every vertex of `part` at height 60, keyed by world "x,z" (all tints seen there).
function vertexTints(part, origin) {
  const dv = new DataView(part.verts), out = new Map();
  for (let i = 0; i < part.faces * 4; i++) {
    const o = i * VERTEX_BYTES;
    if (dv.getInt16(o + 2, true) / 16 !== 60) continue;
    const key = `${dv.getInt16(o, true) / 16 + origin[0]},${dv.getInt16(o + 4, true) / 16 + origin[1]}`;
    const t = [10, 11, 12].map((k) => dv.getUint8(o + k)).join(',');
    out.set(key, [...(out.get(key) ?? []), t]);
  }
  return out;
}

const mesh = (top, x, z, size = 8) => {
  const tints = columnTints(wasm, ctx, SEED, x, z, size);
  return meshBlocks(ctx, flat(top, size), tints, size, 0, { origin: [x, z] });
};

test('a grass block top and side carry the same tint at a shared corner', () => {
  const v = vertexTints(mesh(B.grass, 100, -60).opaque, [100, -60]);
  let sides = 0;
  for (const ts of v.values()) {
    assert.ok(ts.every((t) => t === ts[0]), ts.join(' | '));
    if (ts.length > 1) sides++;
  }
  assert.ok(sides > 0, 'no corner is shared by a top and a side');
  assert.ok(new Set([...v.values()].map((t) => t[0])).size > 1, 'tints do not vary');
});

test('adjacent areas meshed separately agree on shared edge corners', () => {
  const a = vertexTints(mesh(B.grass, 100, -60).opaque, [100, -60]);
  const b = vertexTints(mesh(B.grass, 108, -60).opaque, [108, -60]);
  for (let z = -60; z <= -52; z++) assert.deepEqual(a.get(`108,${z}`), b.get(`108,${z}`).slice(0, a.get(`108,${z}`).length), `z = ${z}`);
});

test('blocks with no tint stay white', () => {
  for (const [, ts] of vertexTints(mesh(B.sand, 100, -60).opaque, [100, -60])) assert.ok(ts.every((t) => t === '255,255,255'));
});

// Water is meshed in its own part and tinted like grass, by its corners.
test('water quads carry the water tint of their corners and adjacent areas agree', () => {
  const a = vertexTints(mesh(B.water, 100, -60).water, [100, -60]);
  const b = vertexTints(mesh(B.water, 108, -60).water, [108, -60]);
  assert.ok(new Set([...a.values()].map((t) => t[0])).size > 1, 'water tints do not vary');
  assert.ok([...a.values()].every((ts) => ts.every((t) => t === ts[0]) && t0(ts) !== '255,255,255'));
  for (let z = -60; z <= -52; z++) assert.deepEqual(a.get(`108,${z}`), b.get(`108,${z}`).slice(0, a.get(`108,${z}`).length), `z = ${z}`);
  const t = columnTints(wasm, ctx, SEED, 100, -60, 8), k = 0;
  const w = [...t.corners.water.slice(k, k + 3)].map((v) => Math.round(v * 255)).join(',');
  assert.equal(a.get('100,-60')[0], w);
});
const t0 = (ts) => ts[0];
