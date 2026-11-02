import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { packAtlas, boxVertices } from '../models.js';
import { FISH_MODELS, fishBoxes, fishPose } from '../fishmodels.js';

const exportJson = JSON.parse(readFileSync(new URL('../../viewer/textures.json', import.meta.url)));
const atlas = packAtlas(exportJson);
const rect = (n) => atlas.rects[n.replace('~', '')];

test('fish boxes name only faces in the export, at the art proportions', () => {
  for (const sp of FISH_MODELS) {
    const boxes = fishBoxes(sp, 2);
    assert.equal(boxes.length, 3);
    for (const b of boxes) {
      for (const n of Object.values(b.faces)) assert.ok(rect(n), `${n} missing`);
    }
    const [body, dorsal, tail] = boxes;
    assert.equal(body.size[1], 2);
    const k = 2 / rect(`${sp}.body_left`).h;
    assert.ok(Math.abs(body.size[2] - rect(`${sp}.body_left`).w * k) < 1e-9);
    assert.ok(Math.abs(body.size[0] - rect(`${sp}.body_top`).w * k) < 1e-9);
    assert.ok(Math.abs(dorsal.size[2] - rect(`${sp}.dorsal`).w * k) < 1e-9);
    assert.ok(Math.abs(dorsal.size[1] - rect(`${sp}.dorsal`).h * k) < 1e-9);
    assert.ok(Math.abs(tail.size[2] - rect(`${sp}.tail`).w * k) < 1e-9);
    assert.ok(Math.abs(tail.size[1] - rect(`${sp}.tail`).h * k) < 1e-9);
    assert.ok(dorsal.lo[1] === body.size[1] && tail.lo[2] === body.lo[2] + body.size[2]);
  }
});

test('right faces are the left art mirrored', () => {
  const [body] = fishBoxes('salmon', 4);
  const { vertices } = boxVertices(body, atlas.uvOf);
  const uv = (face) => [0, 1, 2, 3].map((k) => [vertices[(face * 4 + k) * 5 + 3], vertices[(face * 4 + k) * 5 + 4]]);
  const left = uv(2), right = uv(3); // face order: front back left right
  assert.deepEqual(right.map(([u]) => u), [left[2][0], left[3][0], left[0][0], left[1][0]]);
  assert.deepEqual(right.map(([, v]) => v), left.map(([, v]) => v));
  const r = atlas.rects['salmon.body_left'];
  assert.equal(left[0][0] * atlas.width, r.x);
  assert.equal(right[0][0] * atlas.width, r.x + r.w);
});

test('fishPose puts the bottom centre at pos and turns the nose', () => {
  const m = (p, pose) => {
    const a = pose.model;
    return [0, 1, 2].map((i) => a[i] * p[0] + a[4 + i] * p[1] + a[8 + i] * p[2] + a[12 + i]);
  };
  const near = (a, b) => a.every((v, i) => Math.abs(v - b[i]) < 1e-5);
  const at = [10, 5, -3];
  assert.ok(near(m([0, 0, 0], fishPose({ pos: at, heading: 37 })), at));
  assert.ok(near(m([0, 0, -1], fishPose({ pos: at, heading: 0 })), [10, 5, -4]));
  assert.ok(near(m([0, 0, -1], fishPose({ pos: at, heading: 90 })), [11, 5, -3]));
  assert.deepEqual(fishPose({ pos: at, heading: 0 }).tint, [1, 1, 1]);
});

test('unknown species draw grey', () => {
  const grey = atlas.uvOf('(grey)');
  const { vertices } = boxVertices(fishBoxes('kraken', 1)[0], atlas.uvOf);
  for (let i = 0; i < 24; i++) assert.deepEqual([vertices[i * 5 + 3], vertices[i * 5 + 4]].map((v) => v >= 0), [true, true]);
  assert.ok(vertices[3] >= grey.u0 && vertices[3] <= grey.u1);
});
