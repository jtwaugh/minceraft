import test from 'node:test';
import assert from 'node:assert/strict';
import { packAtlas, boxVertices, FALLBACK, animalPose, animalBoxes, ANIMAL_MODELS } from '../models.js';

const fixture = {
  palette: { 0: '#101010', 1: '#ff0000', 2: '#00ff00' },
  batches: {
    'Batch 1': { textures: { dirt: ['01', '10'], unused: ['22', '22'] } },
    'Batch 2': {
      models: {
        fish: { body_left: ['0120', '1021'], body_top: ['01', '10', '22'] },
        crab: { shell: ['.1', '10'] },
      },
    },
  },
};

test('packAtlas is deterministic and packs without overlap', () => {
  const a = packAtlas(fixture, ['dirt']);
  const b = packAtlas(fixture, ['dirt']);
  assert.deepEqual(a.rects, b.rects);
  assert.deepEqual(a.data, b.data);
  assert.deepEqual(Object.keys(a.rects).sort(), [FALLBACK, 'crab.shell', 'dirt', 'fish.body_left', 'fish.body_top'].sort());
  assert.equal(a.data.length, a.width * a.height * 4);
  const rs = Object.values(a.rects);
  for (const r of rs) assert.ok(r.x >= 0 && r.y >= 0 && r.x + r.w <= a.width && r.y + r.h <= a.height);
  for (const [i, p] of rs.entries()) {
    for (const q of rs.slice(i + 1)) {
      assert.ok(p.x + p.w <= q.x || q.x + q.w <= p.x || p.y + p.h <= q.y || q.y + q.h <= p.y, 'overlap');
    }
  }
  assert.equal(a.rects['fish.body_left'].w, 4);
  assert.equal(a.rects['fish.body_top'].h, 3);
});

test('packAtlas writes palette pixels and transparency', () => {
  const a = packAtlas(fixture);
  const px = (name, i, j) => {
    const r = a.rects[name];
    const o = ((r.y + j) * a.width + r.x + i) * 4;
    return [...a.data.slice(o, o + 4)];
  };
  assert.deepEqual(px('fish.body_left', 1, 0), [255, 0, 0, 255]);
  assert.deepEqual(px('fish.body_left', 3, 0), [16, 16, 16, 255]);
  assert.deepEqual(px('fish.body_left', 3, 1), [255, 0, 0, 255]);
  assert.deepEqual(px('crab.shell', 0, 0), [0, 0, 0, 0]);
  assert.equal(a.rects.dirt, undefined, 'textures are packed only when asked for');
  assert.deepEqual(px(FALLBACK, 0, 0), [128, 128, 128, 255]);
});

const rect = { u0: 0.25, v0: 0.5, u1: 0.5, v1: 0.75 };
const box = {
  lo: [0, 0, 0], size: [2, 3, 4],
  faces: { front: 'f', back: 'k', left: 'l', right: 'r', top: 't', bottom: 'b' },
};
const verts = boxVertices(box, () => rect);
const at = (face, corner) => {
  const o = (['front', 'back', 'left', 'right', 'top', 'bottom'].indexOf(face) * 4 + corner) * 5;
  return [...verts.vertices.slice(o, o + 5)];
};

test('boxVertices makes 24 vertices and 36 indices', () => {
  assert.equal(verts.vertices.length, 24 * 5);
  assert.equal(verts.indices.length, 36);
  assert.ok(verts.indices.every((i) => i < 24));
});

test('each face maps its top-left pixel to the right outside corner', () => {
  // corner 0 is the grid's top-left (u0, v0); 1 bottom-left; 2 bottom-right; 3 top-right
  assert.deepEqual(at('front', 0), [2, 3, 0, 0.25, 0.5]); // seen from -z, left of image is +x
  assert.deepEqual(at('front', 2), [0, 0, 0, 0.5, 0.75]);
  assert.deepEqual(at('left', 0), [0, 3, 0, 0.25, 0.5]); // front edge at the left of the grid
  assert.deepEqual(at('left', 3), [0, 3, 4, 0.5, 0.5]);
  assert.deepEqual(at('right', 0), [2, 3, 4, 0.25, 0.5]); // front edge at the right of the grid
  assert.deepEqual(at('right', 3), [2, 3, 0, 0.5, 0.5]);
  assert.deepEqual(at('top', 0), [2, 3, 4, 0.25, 0.5]); // front edge is the bottom row
  assert.deepEqual(at('top', 1), [2, 3, 0, 0.25, 0.75]);
  assert.deepEqual(at('bottom', 0), [2, 0, 0, 0.25, 0.5]); // front edge is the top row
  assert.deepEqual(at('bottom', 1), [2, 0, 4, 0.25, 0.75]);
});

test('faces wind counter-clockwise seen from outside', () => {
  const centre = [1, 1.5, 2];
  const p = (n, k) => {
    const o = verts.indices[n * 6 + k] * 5;
    return [...verts.vertices.slice(o, o + 3)];
  };
  for (let n = 0; n < 6; n++) {
    const [a, b, c] = [p(n, 0), p(n, 1), p(n, 2)];
    const u = b.map((v, i) => v - a[i]), w = c.map((v, i) => v - a[i]);
    const nrm = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    const out = a.map((v, i) => v - centre[i]);
    assert.ok(nrm.reduce((s, v, i) => s + v * out[i], 0) > 0, `face ${n}`);
  }
});

test('missing face names fall back to grey without throwing', () => {
  const atlas = packAtlas(fixture);
  const { vertices } = boxVertices({ lo: [0, 0, 0], size: [1, 1, 1], faces: { front: 'nope.front' } }, atlas.uvOf);
  const g = atlas.uvOf(undefined);
  assert.deepEqual(g, atlas.uvOf('nope.front'));
  assert.equal(vertices[3], g.u0);
  assert.equal(vertices[4], g.v0);
  const r = atlas.rects[FALLBACK];
  assert.equal(g.u0, r.x / atlas.width);
});

const pose = (o) => animalPose({ pos: [1, 2, 3], size: [0.9, 1.4], facing: 0, dying: false, hurt: false, ...o });
const near = (a, b) => a.forEach((v, i) => assert.ok(Math.abs(v - b[i]) < 1e-5, `${[...a]} vs ${b}`));

test('animalPose faces and scales', () => {
  near(pose({}).model, [0.9, 0, 0, 0, 0, 1.4, 0, 0, 0, 0, 0.9, 0, 1, 2, 3, 1]);
  near(pose({ facing: 90 }).model, [0, 0, 0.9, 0, 0, 1.4, 0, 0, -0.9, 0, 0, 0, 1, 2, 3, 1]);
  assert.deepEqual(pose({}).tint, [1, 1, 1]);
});

test('animalPose tips a dying animal and reddens a hurt one', () => {
  near(pose({ dying: true }).model, [0, 0.9, 0, 0, -1.4, 0, 0, 0, 0, 0, 0.9, 0, 1, 2.45, 3, 1]);
  const t = pose({ hurt: true }).tint;
  assert.ok(t[0] > t[1] && t[0] > t[2]);
});

test('every batch-15 animal model is requested by the client', async () => {
  const { readFileSync } = await import('node:fs');
  const b = JSON.parse(readFileSync(new URL('../../art/batches_15.json', import.meta.url)));
  const names = Object.keys(Object.values(b)[0].models).sort();
  assert.deepEqual([...ANIMAL_MODELS].sort(), names);
  assert.equal(animalBoxes('cow')[0].faces.front, 'cow.front');
  assert.deepEqual(animalBoxes('dragon')[0].faces, {});
});
