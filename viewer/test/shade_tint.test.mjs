import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, columnTints, meshBlocks, lerp3, B, H, VERTEX_BYTES } from '../mesh.js';
import { shadeFractions } from '../shade.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });
const ctx = { ...legendContext(wasm.legend()), faceLayer: new Uint8Array(256 * 3 * 2) };
const off = { ...ctx, legend: { ...ctx.legend, blend: { ...ctx.legend.blend, shade_mix: 0 } } };
const { shade_mix: MIX, shade_radius: R } = ctx.legend.blend;
const SEED = 12345n;

// Grass at y = 60 over stone, with a leaf block at y = 66 where `leaves(x, z)` (world coordinates).
function area(wx, wz, size, pad, leaves) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const c = (z * w + x) * H;
    for (let y = 0; y < 60; y++) blocks[c + y] = B.stone;
    blocks[c + 60] = B.grass;
    if (leaves(wx - pad + x, wz - pad + z)) blocks[c + 66] = B.oak_leaves;
  }
  return blocks;
}
const grove = (x, z) => x >= 2 && x < 5 && z >= 2 && z < 4;

// Tint bytes of every grass vertex at the top (y 61), keyed by world "x,z".
function topTints(m, origin) {
  const dv = new DataView(m.opaque.verts), out = new Map();
  for (let i = 0; i < m.opaque.faces * 4; i++) {
    const o = i * VERTEX_BYTES;
    if (dv.getInt16(o + 2, true) / 16 !== 61) continue;
    const key = `${dv.getInt16(o, true) / 16 + origin[0]},${dv.getInt16(o + 4, true) / 16 + origin[1]}`;
    out.set(key, [...(out.get(key) ?? []), [10, 11, 12].map((k) => dv.getUint8(o + k)).join(',')]);
  }
  return out;
}
const mesh = (c, blocks, wx, wz, size, pad) =>
  topTints(meshBlocks(c, blocks, columnTints(wasm, c, SEED, wx, wz, size), size, pad, { origin: [wx, wz] }), [wx, wz]);
const bytes = (t) => t.map((v) => Math.round(v * 255)).join(',');

test('a shaded grass corner is mixed toward dry by shade_mix times its fraction, never further', () => {
  assert.ok(MIX > 0);
  const size = 8, pad = 1, blocks = area(0, 0, size, pad, grove);
  const got = mesh(ctx, blocks, 0, 0, size, pad), tints = columnTints(wasm, ctx, SEED, 0, 0, size);
  const n = size + 2, f = shadeFractions(blocks, n, 0, R), fAt = (x, z) => f[(z + 1) * n + x + 1];
  let shaded = 0;
  for (let z = 0; z <= size; z++) for (let x = 0; x <= size; x++) {
    const k = (z * (size + 1) + x) * 3, g = [...tints.corners.grass.slice(k, k + 3)], d = [...tints.corners.dry.slice(k, k + 3)];
    const fc = (fAt(x - 1, z - 1) + fAt(x, z - 1) + fAt(x - 1, z) + fAt(x, z)) / 4;
    assert.ok(fc >= 0 && fc <= 1);
    const want = bytes(lerp3(g, d, MIX * fc));
    assert.ok(got.get(`${x},${z}`).every((t) => t === want), `corner ${x}, ${z}`);
    if (fc > 0) {
      // between grass and the shade_mix point toward dry, channel by channel
      const lo = lerp3(g, d, 0), hi = lerp3(g, d, MIX);
      want.split(',').forEach((v, j) => assert.ok(v >= Math.round(Math.min(lo[j], hi[j]) * 255) && v <= Math.round(Math.max(lo[j], hi[j]) * 255)));
      if (want !== bytes(g)) shaded++;
    }
  }
  assert.ok(shaded > 4);
});

test('open columns keep the grass tint exactly, and shade_mix 0 equals no shading', () => {
  const size = 16, pad = 1, blocks = area(0, 0, size, pad, grove);
  const on = mesh(ctx, blocks, 0, 0, size, pad), zero = mesh(off, blocks, 0, 0, size, pad);
  const bare = mesh(ctx, area(0, 0, size, pad, () => false), 0, 0, size, pad);
  assert.deepEqual(zero, bare);
  let open = 0, diff = 0;
  for (const [key, ts] of on) {
    const [x, z] = key.split(',').map(Number);
    if (x >= 8 || z >= 8) { assert.deepEqual(ts, zero.get(key), key); open++; } else if (ts.join() !== zero.get(key).join()) diff++;
  }
  assert.ok(open > 50 && diff > 4);
});

test('overlapping areas with different origins agree on shared corners whose windows fit in both', () => {
  const leaves = (x, z) => ((Math.imul(x, 73856093) ^ Math.imul(z, 19349663)) >>> 0) % 5 === 0;
  const a = { wx: 100, wz: -60, size: 16, pad: 1 }, b = { wx: 106, wz: -54, size: 12, pad: 1 };
  const ta = mesh(ctx, area(a.wx, a.wz, a.size, a.pad, leaves), a.wx, a.wz, a.size, a.pad);
  const tb = mesh(ctx, area(b.wx, b.wz, b.size, b.pad, leaves), b.wx, b.wz, b.size, b.pad);
  // a corner reads columns cx - 1 - R .. cx + R
  const fits = (s, cx, cz) => cx - 1 - R >= s.wx - s.pad && cx + R < s.wx + s.size + s.pad && cz - 1 - R >= s.wz - s.pad && cz + R < s.wz + s.size + s.pad;
  let shared = 0;
  for (const [key, ts] of tb) {
    const [x, z] = key.split(',').map(Number);
    if (!ta.has(key) || !fits(a, x, z) || !fits(b, x, z)) continue;
    assert.equal(ts[0], ta.get(key)[0], key);
    shared++;
  }
  assert.ok(shared > 30);
});

test('placing and breaking a leaf block changes the remeshed tint', () => {
  const size = 8, pad = 1, blocks = area(0, 0, size, pad, () => false);
  const before = mesh(ctx, blocks, 0, 0, size, pad);
  const c = ((4 + pad) * (size + 2 * pad) + 4 + pad) * H + 80;
  blocks[c] = B.oak_leaves;
  assert.notDeepEqual(mesh(ctx, blocks, 0, 0, size, pad), before);
  blocks[c] = B.air;
  assert.deepEqual(mesh(ctx, blocks, 0, 0, size, pad), before);
});
