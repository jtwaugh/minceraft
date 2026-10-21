import test from 'node:test';
import assert from 'node:assert/strict';
import { padRegion } from '../regionpad.js';
import { H } from '../../viewer/mesh.js';

const SIZE = 4;
// a region whose every block is `id`
const flat = (id) => new Uint8Array(SIZE * SIZE * H).fill(id);
const at = (out, x, z) => out[((z + 1) * (SIZE + 2) + x + 1) * H];

test('the border holds the neighbours\' edge columns', () => {
  const regions = { '0,0': flat(1), '1,0': flat(2), '0,-1': flat(3), '-1,1': flat(4) };
  const out = padRegion((rx, rz) => regions[`${rx},${rz}`] ?? new Uint8Array(0), 0, 0, SIZE);
  assert.equal(at(out, 0, 0), 1);
  assert.equal(at(out, SIZE, 0), 2);
  assert.equal(at(out, 1, -1), 3);
});

test('a neighbour that is not loaded is read as the region\'s own edge', () => {
  const own = flat(1);
  own[(0 * SIZE + (SIZE - 1)) * H] = 9; // the east edge of the first row
  const out = padRegion((rx, rz) => (rx === 0 && rz === 0 ? own : new Uint8Array(0)), 0, 0, SIZE);
  assert.equal(at(out, SIZE, 0), 9);
  assert.equal(at(out, -1, -1), 1);
});

test('columns map across the border by world position', () => {
  const east = flat(0);
  for (let z = 0; z < SIZE; z++) east[(z * SIZE) * H] = 10 + z; // the east neighbour's west edge
  const out = padRegion((rx, rz) => (rx === 1 ? east : flat(1)), 0, 0, SIZE);
  for (let z = 0; z < SIZE; z++) assert.equal(at(out, SIZE, z), 10 + z);
});
