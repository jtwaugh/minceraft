// Byte parity of the Rust mesher (via wasm) with meshBlocks in mesh.js on real
// decorated regions.
//
// Not compared yet, because the Rust mesher doesn't have them (later tickets
// remove these restrictions one by one):
//  - tints: every tint is white, so only untinted blocks are kept;
//  - tile variants and grass tops: varying blocks (stone, sand, gravel, grass) become dirt;
//  - plants, leaves, snow, cacti and other shapes: removed (become air);
//  - ambient occlusion: byte 13 is zeroed, and quads are rotated to a canonical
//    first vertex, since AO can flip a quad's diagonal;
//  - greedy merging: off in JS; water tops are always merged in JS, so
//    horizontal water quads are left out.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';
import { meshBlocksWasm } from '../meshwasm.js';

const pkg = new URL('../pkg/', import.meta.url);
const wasm = await import(new URL('terrain_web.js', pkg));
await wasm.default({ module_or_path: fs.readFileSync(new URL('terrain_web_bg.wasm', pkg)) });

const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 251);
const ctx = { COLORS: [], faceLayer };

const KEEP = new Set([B.air, B.dirt, B.water, B.ice, B.oak_log, B.birch_log, B.spruce_log]);
const TO_DIRT = new Set([B.stone, B.sand, B.gravel, B.grass]);
const restrict = (blocks) => blocks.map((b) => (TO_DIRT.has(b) ? B.dirt : KEEP.has(b) ? b : B.air));

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

for (const seed of [12345n, 9n, 42n]) {
  for (const fast of [false, true]) {
    test(`wasm mesher matches meshBlocks, seed ${seed}${fast ? ', fast' : ''}`, () => {
      const size = 32, pad = 1, w = size + 2 * pad;
      const blocks = restrict(wasm.generate_region_decorated(seed, -pad, -pad, w, w));
      const n = size * size;
      const tints = { grass: Array(n).fill([1, 1, 1]), leaf: Array(n).fill([1, 1, 1]) };
      const js = meshBlocks(ctx, blocks, tints, size, pad, { fast, greedy: false });
      const rs = meshBlocksWasm(wasm, ctx, blocks, tints, size, pad, { fast, greedy: false });
      assert.equal(rs.minY, js.minY);
      assert.equal(rs.meanY, js.meanY);
      for (const k of ['opaque', 'cutout', 'ice']) assert.deepEqual(quads(rs[k]), quads(js[k]), k);
      assert.deepEqual(quads(rs.water, true), quads(js.water, true), 'water');
      assert.ok(rs.opaque.faces > 0);
    });
  }
}
