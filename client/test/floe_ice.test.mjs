// Floe ice picks one of floe_ice_0..2 per block from its world (x, z) alone
// (sea-ice-floes.md A), in both meshers, with the client's real layer names.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, faceLayers, variantTable, meshBlocks, B, H, VERTEX_BYTES } from '../../viewer/mesh.js';
import { meshBlocksWasm } from '../../viewer/meshwasm.js';
import { VARIANTS } from '../../viewer/tilelist.js';
import { NAMES } from '../tiles.js';
import * as wasm from '../../viewer/pkg/terrain_web.js';

await wasm.default({ module_or_path: fs.readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url)) });

const base = NAMES.indexOf('floe_ice_0');
const faceLayer = faceLayers((b) => (b === B.floe_ice ? base : 0));
const ctx = legendContext(wasm.legend(), faceLayer, variantTable(NAMES, VARIANTS));

// A size x size sheet of floe ice at height y; returns world "x,z" -> layer of its top faces.
function tops(mesher, size, origin, y) {
  const blocks = new Uint8Array(size * size * H);
  for (let c = 0; c < size * size; c++) blocks[c * H + y] = B.floe_ice;
  const white = Array.from({ length: size * size }, () => [1, 1, 1]);
  const m = mesher(ctx, blocks, { grass: white, leaf: white, water: white }, size, 0, { origin, greedy: false });
  const dv = new DataView(m.ice.verts), out = new Map();
  for (let q = 0; q < m.ice.faces; q++) {
    const o = q * 4 * VERTEX_BYTES;
    const ys = [0, 1, 2, 3].map((j) => dv.getInt16(o + j * VERTEX_BYTES + 2, true) / 16);
    if (ys.some((v) => v !== y + 1)) continue;
    const mean = (k) => [0, 1, 2, 3].reduce((s, j) => s + dv.getInt16(o + j * VERTEX_BYTES + k, true) / 16, 0) / 4;
    out.set(`${Math.floor(mean(0)) + origin[0]},${Math.floor(mean(4)) + origin[1]}`, dv.getUint8(o + 8));
  }
  return out;
}

test('floe ice variant depends only on world (x, z), in both meshers', () => {
  const a = tops(meshBlocks, 16, [0, 0], 62);
  const others = [tops(meshBlocks, 16, [8, -8], 62), tops(meshBlocks, 16, [0, 0], 40),
    tops(meshBlocksWasm.bind(null, wasm), 16, [0, 0], 62), tops(meshBlocksWasm.bind(null, wasm), 16, [8, -8], 30)];
  assert.equal(a.size, 256);
  let shared = 0;
  for (const [k, v] of a) for (const m of others) if (m.has(k)) { shared++; assert.equal(m.get(k), v, k); }
  assert.ok(shared > 600, `${shared}`);
});

test('all three floe ice tiles are used, about equally', () => {
  const m = tops(meshBlocks, 48, [-500, 77], 62), counts = [0, 0, 0];
  for (const l of m.values()) { assert.ok(l >= base && l < base + 3, `layer ${l}`); counts[l - base]++; }
  for (const n of counts) assert.ok(Math.abs(n / m.size - 1 / 3) < 0.05, `${counts}`);
  assert.deepEqual([1, 2].map((i) => NAMES[base + i]), ['floe_ice_1', 'floe_ice_2']);
});
