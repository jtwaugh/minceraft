import test from 'node:test';
import assert from 'node:assert/strict';
import { createStream, regionOf } from '../stream.js';

const load = (...rs) => rs.flatMap(([rx, rz]) => [1, rx, rz]);
const unload = (...rs) => rs.flatMap(([rx, rz]) => [0, rx, rz]);
const at = (rx, rz) => ({ rx, rz });

test('regionOf floors, so negative columns land in negative regions', () => {
  assert.deepEqual(regionOf(0, 127.9), at(0, 0));
  assert.deepEqual(regionOf(-0.5, 128), at(-1, 1));
});

test('regions mesh nearest the viewer first', () => {
  const s = createStream({ budget: 1 });
  const order = [];
  let events = load([2, 0], [-1, 0], [0, 0], [1, 1]);
  for (let i = 0; i < 4; i++) {
    order.push(...s.frame(events, at(0, 0)).mesh.map((r) => [r.rx, r.rz]));
    events = [];
  }
  assert.deepEqual(order, [[0, 0], [-1, 0], [1, 1], [2, 0]]);
});

test('the per-frame budget holds and the rest waits', () => {
  const s = createStream({ budget: 2 });
  const first = s.frame(load([0, 0], [1, 0], [2, 0], [3, 0], [4, 0]), at(0, 0));
  assert.equal(first.mesh.length, 2);
  assert.equal(s.pending, 3);
  assert.equal(s.frame([], at(0, 0)).mesh.length, 2);
  assert.equal(s.frame([], at(0, 0)).mesh.length, 1);
  assert.equal(s.frame([], at(0, 0)).mesh.length, 0);
  assert.equal(s.meshed, 5);
});

test('an unloaded region is freed, or dropped quietly if never meshed', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0], [1, 0]), at(0, 0)); // meshes (0,0)
  const out = s.frame(unload([0, 0], [1, 0]), at(0, 0));
  assert.deepEqual(out.free, [at(0, 0)]);
  assert.deepEqual(out.mesh, []);
  assert.equal(s.pending, 0);
  assert.equal(s.meshed, 0);
});

test('a region loaded twice is meshed once', () => {
  const s = createStream({ budget: 4 });
  assert.equal(s.frame(load([0, 0], [0, 0]), at(0, 0)).mesh.length, 1);
  assert.equal(s.frame(load([0, 0]), at(0, 0)).mesh.length, 0);
  assert.equal(s.meshed, 1);
});

test('a region unloaded then loaded again is meshed again', () => {
  const s = createStream({ budget: 4 });
  s.frame(load([0, 0]), at(0, 0));
  const out = s.frame([...unload([0, 0]), ...load([0, 0])], at(0, 0));
  assert.deepEqual(out.free, [at(0, 0)]);
  assert.deepEqual(out.mesh, [at(0, 0)]);
});

test('edited keeps only the meshed regions among the edited ones', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0], [1, 0]), at(0, 0)); // (0, 0) meshed, (1, 0) still queued
  assert.deepEqual(s.edited([0, 0, 1, 0, 5, 5]), [at(0, 0)]);
  assert.deepEqual(s.edited([]), []);
});

test('meshing a region makes its meshed neighbours stale', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0], [1, 0], [5, 5]), at(0, 0)); // meshes (0, 0)
  assert.equal(s.stale, 0);
  s.frame([], at(0, 0)); // meshes (1, 0): (0, 0) is now stale
  assert.equal(s.stale, 1);
  s.frame([], at(0, 0)); // meshes (5, 5): no meshed neighbours
  assert.equal(s.stale, 1);
});

test('one stale region is returned on a frame with no new mesh, none on a frame that meshes', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0]), at(0, 0));
  const meshing = s.frame(load([1, 0], [-1, 0]), at(0, 0)); // (0, 0) goes stale
  assert.equal(meshing.mesh.length, 1);
  assert.deepEqual(meshing.stale, []);
  assert.deepEqual(s.frame([], at(0, 0)).stale, []); // meshes the other side; (0, 0) still stale
  const spare = s.frame([], at(0, 0));
  assert.deepEqual(spare.mesh, []);
  assert.deepEqual(spare.stale, [at(0, 0)]);
  assert.deepEqual(s.frame([], at(0, 0)).stale, []);
});

test('at most one stale region is returned per frame', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0], [2, 0]), at(0, 0)); // (0, 0)
  s.frame([], at(0, 0)); // (2, 0)
  s.frame(load([1, 0]), at(0, 0)); // (1, 0): (0, 0) and (2, 0) stale
  assert.equal(s.stale, 2);
  assert.equal(s.frame([], at(0, 0)).stale.length, 1);
  assert.equal(s.frame([], at(0, 0)).stale.length, 1);
  assert.equal(s.frame([], at(0, 0)).stale.length, 0);
});

test('a freed region is never returned as stale', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0], [1, 0]), at(0, 0));
  s.frame([], at(0, 0)); // (0, 0) stale
  const out = s.frame(unload([0, 0]), at(0, 0));
  assert.deepEqual(out.free, [at(0, 0)]);
  assert.deepEqual(out.stale, []);
  assert.equal(s.stale, 0);
});

test('an edited region is meshed again, so no longer stale', () => {
  const s = createStream({ budget: 1 });
  s.frame(load([0, 0], [1, 0]), at(0, 0));
  s.frame([], at(0, 0)); // (0, 0) stale
  assert.deepEqual(s.edited([0, 0]), [at(0, 0)]);
  assert.deepEqual(s.frame([], at(0, 0)).stale, []);
});
