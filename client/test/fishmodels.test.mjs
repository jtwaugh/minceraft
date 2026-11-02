import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { packAtlas, boxVertices } from '../models.js';
import { FISH_MODELS, fishBoxes, fishPose, createTailTracker, tailHinge, TAIL_AMPLITUDE } from '../fishmodels.js';

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

test('tail tracker: still fish do not swing, faster fish beat faster, within amplitude', () => {
  const tr = createTailTracker();
  const at = (x) => [{ id: 1, pos: [x, 0, 0] }];
  tr.update(at(0), 0);
  assert.equal(tr.update(at(0), 0.1).get(1), 0);
  const run = (v) => {
    const t = createTailTracker();
    let max = 0, phase = 0, prev = 0, crossings = 0;
    for (let i = 0; i <= 200; i++) {
      const a = t.update(at(v * i * 0.05), i * 0.05).get(1);
      assert.ok(Math.abs(a) <= TAIL_AMPLITUDE + 1e-9);
      max = Math.max(max, Math.abs(a));
      if (prev < 0 && a >= 0) crossings++;
      prev = a; phase = a;
    }
    return { max, crossings };
  };
  const slow = run(0.5), fast = run(3);
  assert.ok(fast.crossings > slow.crossings);
  assert.ok(fast.max > slow.max);
});

test('tail tracker forgets fish no longer listed', () => {
  const tr = createTailTracker();
  tr.update([{ id: 1, pos: [0, 0, 0] }, { id: 2, pos: [0, 0, 0] }], 0);
  assert.equal(tr.size(), 2);
  tr.update([{ id: 2, pos: [1, 0, 0] }], 1);
  assert.equal(tr.size(), 1);
});

test('fishPose swing turns only the tail about its hinge', () => {
  const hinge = tailHinge('salmon', 4);
  const p = fishPose({ pos: [10, 5, -3], heading: 0 }, 90, hinge);
  assert.deepEqual(Array.from(p.model), Array.from(fishPose({ pos: [10, 5, -3], heading: 0 }).model));
  const m = (a, v) => [0, 1, 2].map((i) => a[i] * v[0] + a[4 + i] * v[1] + a[8 + i] * v[2] + a[12 + i]);
  const near = (a, b) => a.every((v, i) => Math.abs(v - b[i]) < 1e-4);
  assert.ok(near(m(p.tail, [0, 0, hinge]), [10, 5, -3 + hinge]));
  assert.ok(near(m(p.tail, [0, 0, hinge + 1]), [11, 5, -3 + hinge]));
  assert.deepEqual(Array.from(fishPose({ pos: [1, 2, 3], heading: 20 }).tail), Array.from(fishPose({ pos: [1, 2, 3], heading: 20 }).model));
});
