import test from 'node:test';
import assert from 'node:assert/strict';
import { padRegion } from '../regionpad.js';
import { H } from '../../viewer/mesh.js';

const SIZE = 4;
// a region whose every block is `id`
const flat = (id) => new Uint8Array(SIZE * SIZE * H).fill(id);
const at = (out, x, z) => out[((z + 1) * (SIZE + 2) + x + 1) * H];

// The strip of a full region that faces its neighbour at (-dx, -dz), as Walk.region_border gives it.
const borderOf = (get) => (rx, rz, dx, dz) => {
  const b = get(rx, rz);
  if (!b.length) return b;
  const span = (d) => (d < 0 ? [SIZE - 1] : d > 0 ? [0] : [...Array(SIZE).keys()]);
  const out = [];
  for (const z of span(dz)) for (const x of span(dx)) out.push(...b.subarray((z * SIZE + x) * H, (z * SIZE + x + 1) * H));
  return Uint8Array.from(out);
};
const pad = (get, rx, rz) => padRegion(get, borderOf(get), rx, rz, SIZE);

// The padding built from whole neighbours, as padRegion did before it read border strips.
function padFull(get, rx, rz) {
  const own = get(rx, rz), w = SIZE + 2, out = new Uint8Array(w * w * H);
  for (let z = -1; z <= SIZE; z++) {
    for (let x = -1; x <= SIZE; x++) {
      const dx = x < 0 ? -1 : x >= SIZE ? 1 : 0, dz = z < 0 ? -1 : z >= SIZE ? 1 : 0;
      const nb = get(rx + dx, rz + dz);
      const [sx, sz] = nb.length ? [(x + SIZE) % SIZE, (z + SIZE) % SIZE] : [Math.min(Math.max(x, 0), SIZE - 1), Math.min(Math.max(z, 0), SIZE - 1)];
      const src = (nb.length ? nb : own).subarray((sz * SIZE + sx) * H, (sz * SIZE + sx + 1) * H);
      out.set(src, ((z + 1) * w + x + 1) * H);
    }
  }
  return out;
}

test('the border holds the neighbours\' edge columns', () => {
  const regions = { '0,0': flat(1), '1,0': flat(2), '0,-1': flat(3), '-1,1': flat(4) };
  const out = pad((rx, rz) => regions[`${rx},${rz}`] ?? new Uint8Array(0), 0, 0);
  assert.equal(at(out, 0, 0), 1);
  assert.equal(at(out, SIZE, 0), 2);
  assert.equal(at(out, 1, -1), 3);
  assert.equal(at(out, -1, SIZE), 4);
});

test('a neighbour that is not loaded is read as the region\'s own edge', () => {
  const own = flat(1);
  own[(0 * SIZE + (SIZE - 1)) * H] = 9; // the east edge of the first row
  const out = pad((rx, rz) => (rx === 0 && rz === 0 ? own : new Uint8Array(0)), 0, 0);
  assert.equal(at(out, SIZE, 0), 9);
  assert.equal(at(out, -1, -1), 1);
});

test('columns map across the border by world position', () => {
  const east = flat(0);
  for (let z = 0; z < SIZE; z++) east[(z * SIZE) * H] = 10 + z; // the east neighbour's west edge
  const out = pad((rx, rz) => (rx === 1 ? east : flat(1)), 0, 0);
  for (let z = 0; z < SIZE; z++) assert.equal(at(out, SIZE, z), 10 + z);
});

test('padding from border strips equals padding from whole neighbours, with border edits', () => {
  // every block distinct-ish, so a column taken from the wrong place shows
  const region = (rx, rz) => Uint8Array.from({ length: SIZE * SIZE * H }, (_, i) => (i * 31 + rx * 7 + rz * 13 + 1000) % 251);
  const regions = new Map();
  for (let rz = -1; rz <= 1; rz++) for (let rx = -1; rx <= 1; rx++) regions.set(`${rx},${rz}`, region(rx, rz));
  // edits on neighbours' border columns: the east edge's middle, the north edge, a corner
  regions.get('1,0')[(2 * SIZE + 0) * H + 5] = 250;
  regions.get('0,-1')[((SIZE - 1) * SIZE + 1) * H + 7] = 249;
  regions.get('-1,1')[(0 * SIZE + SIZE - 1) * H + 3] = 248;
  const all = (rx, rz) => regions.get(`${rx},${rz}`) ?? new Uint8Array(0);
  assert.deepEqual(pad(all, 0, 0), padFull(all, 0, 0));
  // and with some neighbours not loaded
  const some = (rx, rz) => (rx === 1 || rz === 1 ? new Uint8Array(0) : all(rx, rz));
  assert.deepEqual(pad(some, 0, 0), padFull(some, 0, 0));
});

test('only the region itself is read in full', () => {
  const calls = [];
  const get = (rx, rz) => { calls.push(`${rx},${rz}`); return flat(1); };
  padRegion(get, borderOf(() => flat(2)), 0, 0, SIZE);
  assert.deepEqual(calls, ['0,0']);
});
