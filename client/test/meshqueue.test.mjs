import test from 'node:test';
import assert from 'node:assert/strict';
import { createMeshQueue } from '../meshqueue.js';

const at = (rx, rz) => ({ rx, rz });
const job = (rx, rz, kind) => ({ rx, rz, kind });

// Hands out every job one at a time, reporting each back before the next.
function drain(q, centre) {
  const out = [];
  for (let j = q.next(centre); j; j = q.next(centre)) {
    out.push(j);
    assert.equal(q.done(j.rx, j.rz), true);
  }
  return out;
}

test('edit goes before new, and new before stale', () => {
  const q = createMeshQueue({ limit: 1 });
  q.submit(job(5, 5, 'stale'));
  q.submit(job(0, 0, 'new'));
  q.submit(job(3, 3, 'edit'));
  q.submit(job(6, 6, 'stale'));
  q.submit(job(1, 0, 'new'));
  assert.deepEqual(drain(q, at(0, 0)).map((j) => j.kind), ['edit', 'new', 'new', 'stale', 'stale']);
});

test('new regions go nearest the centre first: Chebyshev, then Euclidean', () => {
  const q = createMeshQueue({ limit: 1 });
  for (const [rx, rz] of [[2, 0], [1, 1], [-1, 0], [0, 0], [2, 2], [0, -2]]) q.submit(job(rx, rz, 'new'));
  const order = drain(q, at(0, 0)).map((j) => [j.rx, j.rz]);
  assert.deepEqual(order, [[0, 0], [-1, 0], [1, 1], [2, 0], [0, -2], [2, 2]]);
});

test('nearest is measured from the centre passed to next', () => {
  const q = createMeshQueue({ limit: 1 });
  q.submit(job(0, 0, 'new'));
  q.submit(job(4, 0, 'new'));
  assert.deepEqual(q.next(at(4, 0)), job(4, 0, 'new'));
});

test('a region queued twice keeps one job with the higher priority', () => {
  const q = createMeshQueue({ limit: 1 });
  q.submit(job(0, 0, 'stale'));
  q.submit(job(0, 0, 'edit'));
  q.submit(job(0, 0, 'new')); // lower than edit: ignored
  q.submit(job(1, 0, 'new'));
  q.submit(job(1, 0, 'stale')); // lower than new: ignored
  assert.equal(q.queued, 2);
  assert.deepEqual(drain(q, at(5, 5)), [job(0, 0, 'edit'), job(1, 0, 'new')]);
});

test('a cancelled in-flight job is reported as discarded and frees its slot only when done', () => {
  const q = createMeshQueue({ limit: 1 });
  q.submit(job(0, 0, 'new'));
  q.submit(job(1, 0, 'new'));
  assert.deepEqual(q.next(at(0, 0)), job(0, 0, 'new'));
  q.cancel(0, 0);
  assert.equal(q.next(at(0, 0)), null); // the worker is still busy
  assert.equal(q.done(0, 0), false);
  assert.deepEqual(q.next(at(0, 0)), job(1, 0, 'new'));
  assert.equal(q.done(1, 0), true);
});

test('a cancelled queued job is never handed out', () => {
  const q = createMeshQueue({ limit: 2 });
  q.submit(job(0, 0, 'new'));
  q.submit(job(1, 0, 'edit'));
  q.cancel(1, 0);
  assert.deepEqual(drain(q, at(0, 0)), [job(0, 0, 'new')]);
});

test('no more than limit jobs are in flight, and results come back in any order', () => {
  const q = createMeshQueue({ limit: 2 });
  for (let rx = 0; rx < 4; rx++) q.submit(job(rx, 0, 'new'));
  const a = q.next(at(0, 0));
  const b = q.next(at(0, 0));
  assert.deepEqual([a, b], [job(0, 0, 'new'), job(1, 0, 'new')]);
  assert.equal(q.next(at(0, 0)), null);
  assert.equal(q.inFlight, 2);
  assert.equal(q.done(1, 0), true); // the second result arrives first
  assert.deepEqual(q.next(at(0, 0)), job(2, 0, 'new'));
  assert.equal(q.next(at(0, 0)), null);
  assert.equal(q.done(0, 0), true);
  assert.equal(q.done(2, 0), true);
  assert.deepEqual(q.next(at(0, 0)), job(3, 0, 'new'));
});

test('a region in flight gets its next job only after its result is back', () => {
  const q = createMeshQueue({ limit: 2 });
  q.submit(job(0, 0, 'new'));
  assert.deepEqual(q.next(at(0, 0)), job(0, 0, 'new'));
  q.submit(job(0, 0, 'edit')); // an edit lands while the region is meshing
  assert.equal(q.next(at(0, 0)), null);
  assert.equal(q.done(0, 0), true);
  assert.deepEqual(q.next(at(0, 0)), job(0, 0, 'edit'));
});

test('an unknown kind is refused', () => {
  assert.throws(() => createMeshQueue().submit(job(0, 0, 'later')), /unknown mesh job kind/);
});
