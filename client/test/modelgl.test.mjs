import test from 'node:test';
import assert from 'node:assert/strict';
import { mergeBoxes } from '../modelgl.js';
import { packAtlas } from '../models.js';

const box = (x) => ({ lo: [x, 0, 0], size: [1, 1, 1], faces: {} });

test('mergeBoxes offsets indices per box', () => {
  const atlas = packAtlas({ palette: {}, batches: {} });
  const m = mergeBoxes([box(0), box(2)], atlas.uvOf);
  assert.equal(m.vertices.length, 2 * 24 * 5);
  assert.equal(m.indices.length, 72);
  assert.equal(Math.min(...m.indices.slice(36)), 24);
  assert.equal(Math.max(...m.indices), 47);
});

test('mergeBoxes with no boxes is empty', () => {
  const m = mergeBoxes([], () => ({ u0: 0, v0: 0, u1: 1, v1: 1 }));
  assert.equal(m.indices.length, 0);
});
