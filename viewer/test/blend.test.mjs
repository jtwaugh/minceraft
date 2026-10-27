import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, columnTints } from '../mesh.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });
const ctx = legendContext(wasm.legend());
const SEED = 12345n;

for (const R of [0, 7]) {
  test(`blurred tints agree on shared world columns, R = ${R}`, () => {
    const size = 24, a = columnTints(wasm, ctx, SEED, 100, -60, size, R);
    const b = columnTints(wasm, ctx, SEED, 108, -52, size, R);
    for (let z = 0; z < size - 8; z++) for (let x = 0; x < size - 8; x++) {
      const ia = (z + 8) * size + x + 8, ib = z * size + x;
      assert.deepEqual(a.grass[ia], b.grass[ib]);
      assert.deepEqual(a.leaf[ia], b.leaf[ib]);
    }
  });
}

test('R = 0 gives the unblended ramp colours', () => {
  const t = columnTints(wasm, ctx, SEED, 0, 0, 16, 0);
  const hum = wasm.humidity_map(SEED, 0, 0, 16, 16);
  for (let c = 0; c < 256; c++) {
    const g = ctx.BIOMES[t.bio[c]].grass;
    const lo = hum[c] < 0.5;
    const f = lo ? hum[c] * 2 : (hum[c] - 0.5) * 2, s0 = g[lo ? 0 : 1], s1 = g[lo ? 1 : 2];
    t.grass[c].forEach((v, k) => assert.ok(Math.abs(v - (s0[k] + (s1[k] - s0[k]) * f) / 255) < 1e-9));
  }
});

test('R = 3 softens the Plains / Forest border', () => {
  const name = (id) => ctx.BIOMES[id].name.toLowerCase();
  const len = 32, row = (R) => columnTints(wasm, ctx, SEED, -98, -1024, len, R);
  const raw = row(0), blur = row(3);
  // Seed 12345 has Plains next to Forest at x = -82, z = -1024.
  // Find a horizontal neighbour pair across a plains / forest border.
  let at = -1;
  for (let z = 0; z < len && at < 0; z++) for (let x = 4; x < len - 4; x++) {
    const a = name(raw.bio[z * len + x - 1]), b = name(raw.bio[z * len + x]);
    if ((a === 'plains' && b === 'forest') || (a === 'forest' && b === 'plains')) { at = z * len + x; break; }
  }
  assert.ok(at >= 0, 'no plains / forest border in the area');
  const step = (t, c) => Math.hypot(...t.grass[c].map((v, k) => v - t.grass[c - 1][k]));
  const maxStep = (t) => {
    let m = 0;
    for (let c = at - 4; c <= at + 3; c++) m = Math.max(m, step(t, c));
    return m;
  };
  assert.ok(maxStep(raw) > 0.02, 'border has a visible step');
  assert.ok(maxStep(blur) < maxStep(raw) / 3);
});
