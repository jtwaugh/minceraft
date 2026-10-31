import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { H, VERTEX_BYTES, legendContext, columnTints } from '../../viewer/mesh.js';
import { meshBlocksWasm } from '../../viewer/meshwasm.js';
import * as terrain from '../../viewer/pkg/terrain_web.js';
import { initSync, Walk } from '../pkg/game_web.js';
import { padRegion } from '../regionpad.js';
import { meshRegionCpu } from '../regionmesh.js';
import { REGION_SIZE } from '../stream.js';

await terrain.default({ module_or_path: fs.readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url)) });
initSync({ module: fs.readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

// A face's block's region is not what decides it: regions meshed apart, each padded
// from its neighbours, draw the same opaque faces at their shared borders as the
// same area meshed in one piece.

const S = REGION_SIZE, N = 2 * S;
const faceLayer = new Uint8Array(256 * 3 * 2).map((_, i) => (i * 7) % 241);
const ctx = { ...legendContext(terrain.legend()), faceLayer, variants: { 3: { count: 3, cum: [0.6, 0.8, 1] } } };
const seed = 12345n, blendRadius = 2;
// regions 0..2 each way are loaded around a player in region (1, 1)
const walk = Walk.streamed(seed, S + 32, 127, S + 32);
const REGIONS = [[0, 0], [1, 0], [0, 1], [1, 1]];

// The opaque faces of a mesh at world origin [ox, oz], split into unit faces so
// greedy merging does not matter: "block x,y,z|normal|layer" -> { n: count, at: block }.
// Faces that are not whole blocks wide (snow, cactus) are kept whole, keyed by their corners.
function unitFaces({ opaque }, [ox, oz], out = new Map()) {
  const dv = new DataView(opaque.verts);
  const add = (k, at) => out.set(k, { n: (out.get(k)?.n ?? 0) + 1, at });
  for (let q = 0; q < opaque.faces; q++) {
    const o = q * 4 * VERTEX_BYTES;
    const v = [0, 1, 2, 3].map((j) => [0, 1, 2].map((a) => dv.getInt16(o + j * VERTEX_BYTES + a * 2, true) + [ox * 16, 0, oz * 16][a]));
    const layer = dv.getUint8(o + 8);
    const e1 = v[1].map((t, a) => t - v[0][a]), e2 = v[2].map((t, a) => t - v[0][a]);
    const n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]].map(Math.sign);
    const lo = [0, 1, 2].map((a) => Math.min(...v.map((p) => p[a]))), hi = [0, 1, 2].map((a) => Math.max(...v.map((p) => p[a])));
    const pa = n.findIndex((t) => t !== 0);
    if (![0, 1, 2].every((a) => a === pa || (lo[a] % 16 === 0 && hi[a] % 16 === 0)) || lo[pa] % 16 !== 0) {
      add(`${v.map((p) => p.join(',')).join(' ')}|${n}|${layer}`, lo.map((t) => Math.floor(t / 16)));
      continue;
    }
    const [ua, va] = [0, 1, 2].filter((a) => a !== pa), b = [0, 0, 0];
    b[pa] = lo[pa] / 16 - (n[pa] > 0 ? 1 : 0);
    for (let u = lo[ua] / 16; u < hi[ua] / 16; u++) for (let w = lo[va] / 16; w < hi[va] / 16; w++) {
      b[ua] = u; b[va] = w;
      add(`${b}|${n}|${layer}`, [...b]);
    }
  }
  return out;
}

// The faces whose block is within two of a shared border of the 2 x 2 regions
// from (0, 0), away from the area's outer edge and above every mesh's floor
// (the mesher skips blocks more than 8 below its lowest column).
function nearBorders(faces, floor) {
  const out = new Map();
  for (const [k, f] of faces) {
    const [x, y, z] = f.at;
    const near = Math.abs(x - S + 0.5) < 2.5 || Math.abs(z - S + 0.5) < 2.5;
    if (near && x > 0 && z > 0 && x < N - 1 && z < N - 1 && y > floor) out.set(k, f);
  }
  return out;
}

const floorOf = (m) => Math.max(0, m.minY - 8);

// The four regions, meshed in one piece from their unpadded blocks.
function wholeMesh() {
  const blocks = new Uint8Array(N * N * H);
  for (const [rx, rz] of REGIONS) {
    const r = walk.region_blocks(rx, rz);
    assert.ok(r.length, `${rx},${rz} loaded`);
    for (let z = 0; z < S; z++) blocks.set(r.subarray(z * S * H, (z + 1) * S * H), ((rz * S + z) * N + rx * S) * H);
  }
  return meshBlocksWasm(terrain, ctx, blocks, columnTints(terrain, ctx, seed, 0, 0, N, blendRadius), N, 0, { origin: [0, 0] });
}

// The four regions meshed apart; `border` stands in for Walk.region_border on region (0, 0).
function apartMeshes(border) {
  return REGIONS.map(([rx, rz]) => {
    if (rx || rz || !border) return meshRegionCpu({ walk, terrain, ctx, seed, blendRadius }, { rx, rz });
    const blocks = padRegion((a, b) => walk.region_blocks(a, b), border, 0, 0, S);
    return { mesh: meshBlocksWasm(terrain, ctx, blocks, columnTints(terrain, ctx, seed, 0, 0, S, blendRadius), S, 1, { origin: [0, 0] }), origin: [0, 0] };
  });
}

// Keys counted differently by the two meshes, split by which border they are on.
function compare(apart) {
  const whole = wholeMesh();
  const floor = Math.max(floorOf(whole), ...apart.map(({ mesh }) => floorOf(mesh)));
  const merged = new Map();
  for (const { mesh, origin } of apart) unitFaces(mesh, origin, merged);
  const a = nearBorders(merged, floor), b = nearBorders(unitFaces(whole, [0, 0]), floor);
  const diff = [...new Set([...a.keys(), ...b.keys()])].filter((k) => a.get(k)?.n !== b.get(k)?.n);
  const onX = (k) => Math.abs((a.get(k) ?? b.get(k)).at[0] - S + 0.5) < 2.5;
  return { diff, x: diff.filter(onX), z: diff.filter((k) => !onX(k)), count: b.size };
}

test('regions meshed apart draw the same opaque faces at their x and z borders as meshed together', () => {
  const { diff, count } = compare(apartMeshes(null));
  assert.ok(count > 1000, `${count} faces compared`);
  assert.deepEqual(diff, []);
});

test('padding from the region\'s own edge instead of a loaded neighbour shows as different faces', () => {
  const real = (a, b, dx, dz) => walk.region_border(a, b, dx, dz);
  // the unloaded-neighbour fallback, wrongly taken for the east, then the south, neighbour
  const east = compare(apartMeshes((a, b, dx, dz) => (dx === 1 && dz === 0 ? new Uint8Array(0) : real(a, b, dx, dz))));
  assert.ok(east.x.length > 0, 'x border fault found');
  const south = compare(apartMeshes((a, b, dx, dz) => (dx === 0 && dz === 1 ? new Uint8Array(0) : real(a, b, dx, dz))));
  assert.ok(south.z.length > 0, 'z border fault found');
});
