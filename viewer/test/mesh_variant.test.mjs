import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, variantTable, meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });

// Layers: grass top at 10 with variants 11, 12; tall grass at 20 with 21, 22.
const faceLayer = new Uint8Array(256 * 3 * 2);
for (const k of [0, 768]) {
  faceLayer.fill(10, k + B.grass * 3, k + B.grass * 3 + 1);
  faceLayer.fill(3, k + B.grass * 3 + 1, k + B.grass * 3 + 3);
  faceLayer.fill(20, k + B.tall_grass * 3, k + B.tall_grass * 3 + 3);
}
const names = [];
names[10] = 'grass_top'; names[20] = 'tall_grass';
const variants = variantTable(names, { grass_top: { count: 3, weights: [0.6, 0.2, 0.2] }, tall_grass: { count: 3, weights: [0.5, 0.25, 0.25] } });
const legend = wasm.legend();
const ctxV = legendContext(legend, faceLayer, variants);
const ctxN = legendContext(legend, faceLayer);

// size x size grass at y 59 with tall grass at y 60 over stone.
function area(size) {
  const blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) {
    for (let y = 0; y < 59; y++) blocks[c * H + y] = B.stone;
    blocks[c * H + 59] = B.grass; blocks[c * H + 60] = B.tall_grass;
  }
  return blocks;
}
const tints = (n) => ({ grass: Array.from({ length: n * n }, () => [1, 1, 1]), leaf: Array.from({ length: n * n }, () => [1, 1, 1]) });

// Layer of every top-facing grass quad / plant quad, keyed by world "x,z".
function layers(ctx, size, origin) {
  const m = meshBlocks(ctx, area(size), tints(size), size, 0, { origin });
  const out = { grass: new Map(), plant: new Map() };
  for (const [part, map, y] of [[m.opaque, out.grass, 60], [m.cutout, out.plant, 60]]) {
    const dv = new DataView(part.verts);
    for (let i = 0; i < part.faces * 4; i += 4) {
      const o = i * VERTEX_BYTES;
      const wy = dv.getInt16(o + 2, true) / 16;
      const layer = dv.getUint8(o + 8);
      if (map === out.grass && wy !== y) continue;
      // plants are offset within their column, so locate a quad by its first two vertices' mean
      const mx = (dv.getInt16(o, true) + dv.getInt16(o + VERTEX_BYTES, true)) / 32, mz = (dv.getInt16(o + 4, true) + dv.getInt16(o + VERTEX_BYTES + 4, true)) / 32;
      map.set(`${Math.floor(mx + origin[0])},${Math.floor(mz + origin[1])}`, layer);
    }
  }
  return out;
}

test('layer depends on world coordinate, not origin', () => {
  const a = layers(ctxV, 16, [0, 0]), b = layers(ctxV, 16, [8, 8]);
  let shared = 0;
  for (const kind of ['grass', 'plant']) for (const [k, v] of a[kind]) if (b[kind].has(k)) { shared++; assert.equal(b[kind].get(k), v, k); }
  assert.ok(shared > 50);
});

test('variants appear in proportion to weights', () => {
  const size = 64, a = layers(ctxV, size, [-100, 37]);
  for (const [kind, base, w] of [['grass', 10, [0.6, 0.2, 0.2]], ['plant', 20, [0.5, 0.25, 0.25]]]) {
    const counts = [0, 0, 0];
    for (const l of a[kind].values()) { assert.ok(l >= base && l < base + 3, `${kind} layer ${l}`); counts[l - base]++; }
    const total = a[kind].size;
    assert.ok(total >= size * size / 2);
    w.forEach((p, i) => assert.ok(Math.abs(counts[i] / total - p) < 0.05, `${kind} ${i}: ${counts[i] / total}`));
  }
});

test('without a variant table every block uses its base layer', () => {
  const a = layers(ctxN, 16, [0, 0]);
  assert.ok(new Set(a.grass.values()).size === 1 && [...a.grass.values()][0] === 10);
  assert.ok(new Set(a.plant.values()).size === 1 && [...a.plant.values()][0] === 20);
});
