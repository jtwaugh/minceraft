// Pure-JS checks for the frame statistics.
// Run: node --test client/test/frametimes.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { FrameTimes, FRAME_BUDGET_MS, HITCH_MS } from '../frametimes.js';

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
