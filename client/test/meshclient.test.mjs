import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { legendContext } from '../../viewer/mesh.js';
import * as terrain from '../../viewer/pkg/terrain_web.js';
import { initSync, Walk } from '../pkg/game_web.js';
import { createMeshClient } from '../meshclient.js';
import { createMeshHandler } from '../meshworker.js';
import { gatherRegion, meshPadded } from '../regionmesh.js';

const bytes = fs.readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url));
await terrain.default({ module_or_path: bytes });
initSync({ module: fs.readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
const ctx = { ...legendContext(terrain.legend()), faceLayer, variants: { 3: { count: 3, cum: [0.6, 0.8, 1] } } };
const seed = 12345n, blendRadius = 2, walk = Walk.streamed(seed, 32, 127, 32);
const init = { module: new WebAssembly.Module(bytes), ctx, seed, blendRadius };
const KINDS = ['opaque', 'cutout', 'water', 'ice'];

// `msg` as the other thread gets it: a structured clone with `transfer` moved.
// A compiled module is passed as is (postMessage shares it; Node's
// structuredClone may not clone it).
function cross({ module, ...msg }, transfer = []) {
  return { ...structuredClone(msg, { transfer }), ...(module && { module }) };
}

// A Worker stand-in running the handler in this thread; replies are held
// until `deliver(order)` hands them back in that order.
function inProcess() {
  const handle = createMeshHandler(terrain);
  const held = [];
  const worker = {
    onmessage: null,
    postMessage(msg, transfer) {
      held.push(handle(cross(msg, transfer)));
    },
    async deliver(order) {
      const replies = (await Promise.all(held.splice(0))).filter(Boolean);
      for (const i of order(replies.map((_, j) => j))) {
        const [reply, transfer] = replies[i];
        worker.onmessage({ data: cross(reply, transfer) });
      }
    },
  };
  return worker;
}

test("the worker's handler replies with meshPadded's result and transfers every vertex buffer", async () => {
  const handle = createMeshHandler(terrain);
  assert.equal(await handle(cross({ type: 'init', ...init })), null);
  for (const [rx, rz] of [[0, 0], [-1, 1]]) {
    const { blocks } = gatherRegion({ walk }, { rx, rz });
    const want = meshPadded({ terrain, ctx, seed, blendRadius }, { rx, rz, blocks });
    const [reply, transfer] = await handle(cross({ type: 'mesh', id: 7, rx, rz, blocks }));
    assert.equal(reply.id, 7);
    assert.equal(reply.error, undefined);
    assert.deepEqual(reply.result, want);
    for (const k of KINDS) assert.ok(transfer.includes(reply.result.mesh[k].verts), k);
    assert.ok(transfer.includes(reply.result.tints.corners.grass.buffer));
    assert.equal(new Set(transfer).size, transfer.length);
    // the whole reply can cross a thread boundary with that transfer list
    structuredClone(reply, { transfer });
    assert.equal(reply.result.mesh.opaque.verts.byteLength, 0);
  }
});

test('the client resolves overlapping jobs to their own regions, whatever order replies come in', async () => {
  const regions = [[0, 0], [1, 0], [0, -1], [-1, -1]];
  const want = regions.map(([rx, rz]) => meshPadded({ terrain, ctx, seed, blendRadius }, { rx, rz, blocks: gatherRegion({ walk }, { rx, rz }).blocks }));
  for (const order of [(ix) => ix, (ix) => ix.reverse(), (ix) => [2, 0, 3, 1]]) {
    let worker;
    const mesher = createMeshClient({ spawn: () => (worker = inProcess()), init });
    const jobs = regions.map(([rx, rz]) => {
      const { blocks } = gatherRegion({ walk }, { rx, rz });
      const p = mesher.submit(rx, rz, blocks);
      assert.equal(blocks.length, 0, 'blocks are transferred');
      return p;
    });
    await worker.deliver(order);
    const got = await Promise.all(jobs);
    got.forEach((g, i) => {
      assert.deepEqual(g.origin, want[i].origin);
      assert.deepEqual(g.snow, want[i].snow);
      for (const k of KINDS) assert.deepEqual(new Uint8Array(g.mesh[k].verts), new Uint8Array(want[i].mesh[k].verts), `${regions[i]} ${k}`);
    });
  }
});

test('a job the worker fails rejects, and terminate rejects the rest', async () => {
  let worker;
  const mesher = createMeshClient({ spawn: () => (worker = inProcess()) }); // no init: meshing throws
  const failed = mesher.submit(0, 0, new Uint8Array(4));
  await worker.deliver((ix) => ix);
  await assert.rejects(failed);
  const left = mesher.submit(0, 0, new Uint8Array(4));
  mesher.terminate();
  await assert.rejects(left, /terminated/);
});
