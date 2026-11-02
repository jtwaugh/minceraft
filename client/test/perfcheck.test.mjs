// Pure-JS checks for the perf-pass verdict.
// Run: node --test client/test/perfcheck.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { HITCH_MS } from '../frametimes.js';
import { PERF_REPLAYS, PERF_MAX_MS, median, perfVerdict } from '../perfcheck.js';

const replay = (max, hitches = 0, records = []) => ({ frames: 300, p50: 10, p99: 12, max, hitches, records });

test('threshold and replay count', () => {
  assert.equal(PERF_MAX_MS, HITCH_MS);
  assert.equal(PERF_REPLAYS, 5);
});

test('median handles odd, even and empty input', () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
  assert.equal(median([]), 0);
});

test('one replay over is not over; three of five is', () => {
  const one = [replay(10), replay(12), replay(200), replay(11), replay(13)];
  assert.equal(perfVerdict(one).verdict, 'ok');
  const three = [replay(10), replay(50), replay(200), replay(40), replay(13)];
  assert.equal(perfVerdict(three).verdict, 'over');
  assert.equal(perfVerdict([replay(HITCH_MS)]).verdict, 'ok');
});

test('empty input gives no verdict', () => {
  const v = perfVerdict([]);
  assert.equal(v.verdict, null);
  assert.equal(v.text, '');
});

test('text lists the hitch records of the slowest replay', () => {
  const rec = { ms: 90, meshed: 2, remeshed: 1, verts: 500, ticks: 3, chunks: 4 };
  const v = perfVerdict([replay(10, 0, [{ ...rec, ms: 1 }]), replay(90, 1, [rec]), replay(20)]);
  assert.equal(v.medianHitches, 0);
  assert.match(v.text, /hitch 90\.0 ms: 2 meshed  1 remeshed  500 verts  3 ticks  4 chunks/);
  assert.doesNotMatch(v.text, /hitch 1\.0 ms/);
});
