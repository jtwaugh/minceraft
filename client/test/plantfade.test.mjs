import test from 'node:test';
import assert from 'node:assert/strict';
import { plantScale, PLANT_FADE_RANGE } from '../plantfade.js';
import { B, VERTEX_BYTES, meshBlocks } from '../../viewer/mesh.js';

test('plant fade: full inside the start distance, half mid-range, gone at the end', () => {
  assert.equal(plantScale(0), 1);
  assert.equal(plantScale(48), 1);
  assert.equal(plantScale(48 + PLANT_FADE_RANGE / 2), 0.5);
  assert.equal(plantScale(48 + PLANT_FADE_RANGE), 0);
  assert.equal(plantScale(500), 0);
  assert.equal(plantScale(24, 20), 0.5);
});

test('mesh byte 14: 255 on cross plant tops, 0 at the base and on other faces', () => {
  const size = 2, blocks = new Uint8Array(size * size * 128);
  blocks[10] = B.stone;
  blocks[11] = B.dandelion;
  const ctx = { faceLayer: new Uint8Array(256 * 3 * 2), COLORS: [], NAMES: [] };
  const one = [1, 1, 1];
  const tints = { grass: [one, one, one, one], leaf: [one, one, one, one] };
  const m = meshBlocks(ctx, blocks, tints, size, 0);
  const plant = new Uint8Array(m.cutout.verts), i16 = new Int16Array(m.cutout.verts);
  assert.equal(plant.length, 16 * VERTEX_BYTES);
  let tops = 0, bases = 0;
  for (let v = 0; v < 16; v++) {
    const w = plant[v * VERTEX_BYTES + 14], y = i16[v * 8 + 1];
    if (y === 12 * 16) { assert.equal(w, 255); tops++; } else { assert.equal(y, 11 * 16); assert.equal(w, 0); bases++; }
  }
  assert.equal(tops, 8); assert.equal(bases, 8);
  const solid = new Uint8Array(m.opaque.verts);
  assert.ok(solid.length > 0);
  for (let v = 0; v < solid.length / VERTEX_BYTES; v++) assert.equal(solid[v * VERTEX_BYTES + 14], 0);
});
