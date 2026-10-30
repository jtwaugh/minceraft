// Byte parity of the Rust mesher (via wasm) with meshBlocks in mesh.js on real
// decorated regions.
//
// Not compared yet, because the Rust mesher doesn't have them (later tickets
// remove these restrictions one by one):
//  - tinted blocks other than grass, water and ice (leaves, plants): removed;
//  - plants, leaves, snow, cacti and other shapes: removed (become air);
//  - ambient occlusion: byte 13 is zeroed, and quads are rotated to a canonical
//    first vertex, since AO can flip a quad's diagonal;
//  - greedy merging: off in JS; water tops are always merged in JS when the
//    tint is flat, so without corners horizontal water quads are left out.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';
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

const KEEP = new Set([B.air, B.dirt, B.grass, B.stone, B.sand, B.gravel, B.water, B.ice, B.oak_log, B.birch_log, B.spruce_log]);
const restrict = (blocks) => blocks.map((b) => (KEEP.has(b) ? b : B.air));

// Quads of a part as canonical strings: AO zeroed, rotated to start at the smallest vertex.
function quads(p, skipFlat) {
  const u8 = new Uint8Array(p.verts), out = [];
  for (let q = 0; q < p.faces; q++) {
    const vs = [0, 1, 2, 3].map((j) => {
      const v = u8.slice((q * 4 + j) * VERTEX_BYTES, (q * 4 + j + 1) * VERTEX_BYTES);
      v[13] = 0;
      return v;
    });
    if (skipFlat && vs.every((v) => v[2] === vs[0][2] && v[3] === vs[0][3])) continue;
    let s = 0;
    for (let j = 1; j < 4; j++) if (vs[j].join() < vs[s].join()) s = j;
    out.push([0, 1, 2, 3].map((j) => vs[(s + j) % 4].join()).join(';'));
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
        assert.deepEqual(quads(rs.water, !corners), quads(js.water, !corners), 'water');
        assert.ok(rs.opaque.faces > 0);
      });
    }
  }
}
