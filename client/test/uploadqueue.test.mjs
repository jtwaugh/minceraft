// Upload queue: ready meshes nearest first, edits first, up to a vertex budget.
// Run: node --test client/test/uploadqueue.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { createUploadQueue } from '../uploadqueue.js';
import { UPLOAD_VERTS_PER_FRAME } from '../workbudget.js';

const at = (rx, rz) => ({ rx, rz });
const entry = (rx, rz, kind, verts) => ({ rx, rz, kind, verts });
const where = (list) => list.map((e) => [e.rx, e.rz]);

test('the upload budget is a positive whole number above one peak region', () => {
  assert.ok(Number.isInteger(UPLOAD_VERTS_PER_FRAME) && UPLOAD_VERTS_PER_FRAME >= 805_000);
});

test('a fixed list comes out edits first, then nearest: Chebyshev, then Euclidean', () => {
  const q = createUploadQueue({ budget: 1000 });
  q.add(entry(2, 0, 'new', 10));
  q.add(entry(1, 1, 'stale', 10));
  q.add(entry(3, 3, 'edit', 10));
  q.add(entry(-1, 0, 'new', 10));
  q.add(entry(0, 0, 'new', 10));
  q.add(entry(2, 1, 'stale', 10));
  q.add(entry(1, 0, 'edit', 10));
  assert.deepEqual(where(q.take(at(0, 0))), [[1, 0], [3, 3], [0, 0], [-1, 0], [1, 1], [2, 0], [2, 1]]);
  assert.equal(q.pending, 0);
  assert.deepEqual(q.take(at(0, 0)), []);
});

test('takes entries while the vertex total stays within the budget, leftovers come later', () => {
  const q = createUploadQueue({ budget: 100 });
  q.add(entry(0, 0, 'new', 40));
  q.add(entry(1, 0, 'new', 60));
  q.add(entry(2, 0, 'new', 30));
  q.add(entry(3, 0, 'new', 80));
  q.add(entry(4, 0, 'new', 10));
  const c = at(0, 0);
  assert.deepEqual(where(q.take(c)), [[0, 0], [1, 0]]); // 100: at the budget is within it
  assert.equal(q.pending, 3);
  assert.deepEqual(where(q.take(c)), [[2, 0]]); // 30 + 80 is over: stop, nearest first
  assert.deepEqual(where(q.take(c)), [[3, 0], [4, 0]]);
  assert.equal(q.pending, 0);
});

test('an entry over the budget is still taken, alone, and the rest follow', () => {
  const q = createUploadQueue({ budget: 100 });
  q.add(entry(0, 0, 'new', 500));
  q.add(entry(1, 0, 'new', 5));
  q.add(entry(2, 0, 'new', 5));
  assert.deepEqual(where(q.take(at(0, 0))), [[0, 0]]);
  assert.deepEqual(where(q.take(at(0, 0))), [[1, 0], [2, 0]]);
});

test('edits count against the budget too', () => {
  const q = createUploadQueue({ budget: 100 });
  q.add(entry(0, 0, 'new', 50));
  q.add(entry(5, 5, 'edit', 70));
  assert.deepEqual(where(q.take(at(0, 0))), [[5, 5]]);
  assert.deepEqual(where(q.take(at(0, 0))), [[0, 0]]);
});

test('order follows the centre given to each take', () => {
  const q = createUploadQueue({ budget: 10 });
  for (let rx = 0; rx < 4; rx++) q.add(entry(rx, 0, 'new', 10));
  assert.deepEqual(where(q.take(at(3, 0))), [[3, 0]]);
  assert.deepEqual(where(q.take(at(0, 0))), [[0, 0]]);
});

test('a re-add replaces the older entry: one entry per region', () => {
  const q = createUploadQueue({ budget: 1000 });
  q.add({ ...entry(0, 0, 'new', 10), mesh: 'old' });
  q.add({ ...entry(0, 0, 'stale', 20), mesh: 'new' });
  assert.equal(q.pending, 1);
  assert.deepEqual(q.take(at(0, 0)), [{ ...entry(0, 0, 'stale', 20), mesh: 'new' }]);
});

test('cancel drops the region entry, and a cancel then re-add leaves one', () => {
  const q = createUploadQueue({ budget: 1000 });
  q.add(entry(0, 0, 'new', 10));
  q.add(entry(1, 0, 'new', 10));
  q.cancel(0, 0);
  q.cancel(7, 7); // nothing queued there: no effect
  assert.equal(q.pending, 1);
  q.add(entry(1, 0, 'edit', 10));
  q.cancel(1, 0);
  assert.equal(q.pending, 0);
  q.add(entry(1, 0, 'new', 10));
  assert.deepEqual(where(q.take(at(0, 0))), [[1, 0]]);
  assert.equal(q.pending, 0);
});
