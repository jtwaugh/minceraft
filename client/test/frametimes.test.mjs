// Pure-JS checks for the frame statistics.
// Run: node --test client/test/frametimes.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { FrameTimes, FRAME_BUDGET_MS, HITCH_MS, HITCH_RECORDS } from '../frametimes.js';

test('frame times report nearest-rank percentiles over the window', () => {
  const f = new FrameTimes(100);
  for (let i = 1; i <= 100; i++) f.push(i);
  assert.equal(f.percentile(50), 50);
  assert.equal(f.percentile(99), 99);
  f.push(1000); // overwrites the oldest
  assert.equal(f.count, 100);
  assert.equal(f.percentile(100), 1000);
  assert.equal(new FrameTimes().percentile(99), 0);
});

test('one stall shows in max and hitches but not in p99', () => {
  const f = new FrameTimes(300);
  for (let i = 0; i < 299; i++) f.push(10);
  f.push(150);
  const s = f.summary();
  assert.equal(s.max, 150);
  assert.equal(s.hitches, 1);
  assert.ok(s.p99 < FRAME_BUDGET_MS);
  assert.match(f.format(), /max 150\.0 ms {2}1 hitches/);
});

test('an empty window has max 0 and no hitches', () => {
  const s = new FrameTimes().summary();
  assert.deepEqual(s, { frames: 0, p50: 0, p99: 0, max: 0, hitches: 0 });
});

test('max and hitches drop frames the ring buffer overwrote', () => {
  const f = new FrameTimes(4);
  f.push(150);
  f.push(HITCH_MS + 1);
  for (let i = 0; i < 3; i++) f.push(10); // wraps: the 150 ms frame is gone
  assert.equal(f.max, HITCH_MS + 1);
  assert.equal(f.hitches, 1);
  f.push(HITCH_MS); // overwrites the other hitch; exactly HITCH_MS is not one
  assert.equal(f.max, HITCH_MS);
  assert.equal(f.hitches, 0);
});

test('the hitch threshold is two frame budgets', () => {
  assert.equal(HITCH_MS, 2 * FRAME_BUDGET_MS);
});

test('a hitch frame keeps a record of its work; other frames add none', () => {
  const f = new FrameTimes();
  const work = { meshed: 2, remeshed: 1, verts: 4096, ticks: 3, chunks: 5 };
  f.push(10, work);
  f.push(HITCH_MS, work); // exactly the threshold is not a hitch
  f.push(50, work);
  f.push(60); // a hitch with no work counted records zeros
  assert.deepEqual(f.hitchRecords(), [
    { ms: 50, ...work },
    { ms: 60, meshed: 0, remeshed: 0, verts: 0, ticks: 0, chunks: 0 },
  ]);
  assert.match(f.formatLastHitch(), /^last hitch 60\.0 ms: 0 meshed/);
});

test('hitch records are bounded and evict the oldest first', () => {
  const f = new FrameTimes();
  for (let i = 0; i < HITCH_RECORDS + 3; i++) f.push(100 + i, { meshed: i });
  const r = f.hitchRecords();
  assert.equal(r.length, HITCH_RECORDS);
  assert.equal(r[0].meshed, 3);
  assert.equal(r.at(-1).meshed, HITCH_RECORDS + 2);
});

test('a series with no hitches keeps no records', () => {
  const f = new FrameTimes(10);
  for (let i = 0; i < 50; i++) f.push(16, { meshed: 1, verts: 100 });
  assert.deepEqual(f.hitchRecords(), []);
  assert.equal(f.formatLastHitch(), '');
});
