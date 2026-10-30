// Byte parity of the Rust mesher (via wasm) with meshBlocks in mesh.js on real
// decorated regions.
//
// Not compared yet, because the Rust mesher doesn't have them (later tickets
// remove these restrictions one by one):
//  - greedy merging of opaque faces: off in JS. (Water tops with a flat tint
//    merge in both, greedy or not.)
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { meshBlocks, B, CROSS, H, VERTEX_BYTES } from '../mesh.js';
import { meshBlocksWasm } from '../meshwasm.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });

const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
for (const o of [0, 768]) { faceLayer[o + B.grass * 3] = 249; }
// Grass tops (layer 249) have 3 variants at 249-251, picked by world coordinate.
const ctx = { COLORS: [], faceLayer, variants: { 249: { count: 3, cum: [0.6, 0.8, 1] } } };
ctx.COLORS[B.birch_leaves] = [150, 190, 90];
ctx.COLORS[B.spruce_leaves] = [60, 110, 70];

const KEEP = new Set([B.air, B.dirt, B.grass, B.stone, B.sand, B.gravel, B.water, B.ice, B.oak_log, B.birch_log, B.spruce_log,
  B.oak_leaves, B.birch_leaves, B.spruce_leaves, B.cactus, B.berry_bush, B.bush, B.jungle_bush, B.snow_layer,
  B.leaf_litter, B.lily_pad]);
const restrict = (blocks) => blocks.map((b) => (KEEP.has(b) || CROSS[b] ? b : B.air));

// Quads of a part as strings, vertex order kept.
function quads(p) {
  const u8 = new Uint8Array(p.verts), out = [];
  for (let q = 0; q < p.faces; q++) {
    const vs = [0, 1, 2, 3].map((j) => u8.slice((q * 4 + j) * VERTEX_BYTES, (q * 4 + j + 1) * VERTEX_BYTES));
    out.push(vs.map((v) => v.join()).join(';'));
  }
  return out.sort();
}

// Reproducible tints that differ per column and per corner (exact in f32).
function randomTints(size, corners) {
  let a = 12345;
  const rnd = () => Math.fround(((a = (a * 1103515245 + 12345) & 0x7fffffff) >> 8) / 0x800000);
  const cols = () => Array.from({ length: size * size }, () => [rnd(), rnd(), rnd()]);
  const grids = () => Float32Array.from({ length: (size + 1) * (size + 1) * 3 }, rnd);
  const t = { grass: cols(), leaf: cols(), water: cols(), dry: cols() };
  if (corners) t.corners = { n: size + 1, grass: grids(), leaf: grids(), water: grids(), dry: grids() };
  return t;
}

for (const seed of [12345n, 9n, 42n]) {
  for (const fast of [false, true]) {
    for (const corners of [false, true]) {
      test(`wasm mesher matches meshBlocks, seed ${seed}${fast ? ', fast' : ''}${corners ? ', corners' : ''}`, () => {
        const size = 32, pad = 1, w = size + 2 * pad;
        const blocks = restrict(wasm.generate_region_decorated(seed, -pad, -pad, w, w));
        const tints = randomTints(size, corners);
        const origin = [Number(seed) * 3 - 1000, -77 - Number(seed)]; // negative and positive world coordinates
        const js = meshBlocks(ctx, blocks, tints, size, pad, { fast, greedy: false, origin });
        const rs = meshBlocksWasm(wasm, ctx, blocks, tints, size, pad, { fast, greedy: false, origin });
        assert.equal(rs.minY, js.minY);
        assert.equal(rs.meanY, js.meanY);
        for (const k of ['opaque', 'cutout', 'ice']) assert.deepEqual(quads(rs[k]), quads(js[k]), k);
        // water doesn't depend on greedy; compare it with greedy on
        const jsG = meshBlocks(ctx, blocks, tints, size, pad, { fast, origin });
        const rsG = meshBlocksWasm(wasm, ctx, blocks, tints, size, pad, { fast, origin });
        assert.deepEqual(quads(rsG.water), quads(jsG.water), 'water');
        assert.deepEqual(quads(rs.water), quads(js.water), 'water, greedy off');
        assert.ok(rs.opaque.faces > 0);
      });
    }
  }
}

// Open ocean: water tops merge into the same rectangles as in JS.
for (const corners of [false, true]) {
  test(`wasm mesher merges ocean water tops like meshBlocks${corners ? ', corners' : ''}`, () => {
    const size = 32, pad = 1, w = size + 2 * pad, origin = [2048, 1024];
    const blocks = wasm.generate_region_decorated(12345n, origin[0] - pad, origin[1] - pad, w, w);
    const col = () => Array(size * size).fill([0.2, 0.4, 0.8]);
    const grid = () => Float32Array.from({ length: (size + 1) * (size + 1) * 3 }, (_, i) => [0.2, 0.4, 0.8][i % 3]);
    const tints = { grass: col(), leaf: col(), water: col(), dry: col() };
    if (corners) tints.corners = { n: size + 1, grass: grid(), leaf: grid(), water: grid(), dry: grid() };
    const js = meshBlocks(ctx, blocks, tints, size, pad, { origin });
    const rs = meshBlocksWasm(wasm, ctx, blocks, tints, size, pad, { origin });
    assert.equal(rs.water.faces, js.water.faces);
    assert.ok(js.water.faces < size * size / 4, `${js.water.faces} water quads`);
    assert.deepEqual(quads(rs.water), quads(js.water));
  });
}
