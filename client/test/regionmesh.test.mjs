import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext, columnTints } from '../../viewer/mesh.js';
import { meshBlocksWasm } from '../../viewer/meshwasm.js';
import * as terrain from '../../viewer/pkg/terrain_web.js';
import { initSync, Walk } from '../pkg/game_web.js';
import { padRegion } from '../regionpad.js';
import { meshRegionCpu } from '../regionmesh.js';
import { REGION_SIZE } from '../stream.js';

await terrain.default({ module_or_path: fs.readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url)) });
initSync({ module: fs.readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

test('the module meshes a streamed region the same as padRegion, columnTints and meshBlocksWasm', () => {
  const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
  const ctx = { ...legendContext(terrain.legend()), faceLayer, variants: { 3: { count: 3, cum: [0.6, 0.8, 1] } } };
  const seed = 12345n, blendRadius = 2, walk = Walk.streamed(seed, 32, 127, 32);
  for (const [rx, rz] of [[0, 0], [-1, 0], [0, 1]]) {
    const x = rx * REGION_SIZE, z = rz * REGION_SIZE;
    const blocks = padRegion((a, b) => walk.region_blocks(a, b), (a, b, dx, dz) => walk.region_border(a, b, dx, dz), rx, rz, REGION_SIZE);
    const tints = columnTints(terrain, ctx, seed, x, z, REGION_SIZE, blendRadius);
    const want = meshBlocksWasm(terrain, ctx, blocks, tints, REGION_SIZE, 1, { origin: [x, z] });
    const got = meshRegionCpu({ walk, terrain, ctx, seed, blendRadius }, { rx, rz });
    assert.deepEqual(got.origin, [x, z]);
    assert.deepEqual(got.blocks, blocks);
    assert.deepEqual(got.tints, tints);
    assert.ok(want.opaque.faces + want.water.faces > 0);
    for (const k of ['opaque', 'cutout', 'water', 'ice']) {
      assert.equal(got.mesh[k].faces, want[k].faces, `${rx},${rz} ${k}`);
      assert.deepEqual(new Uint8Array(got.mesh[k].verts), new Uint8Array(want[k].verts), `${rx},${rz} ${k}`);
    }
  }
});
