// Pure-JS checks for the perf-pass runner's arguments and report (no browser).
// Run: node --test client/test/perfpass.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { PERF_REPLAYS } from '../perfcheck.js';
import { parseArgs, buildReport } from '../tools/perfpass.mjs';

const run = (max, hitches = 0, records = [], spans = []) => ({ stats: { frames: 300, p50: 10, p99: 12, max, hitches, records }, spans });

test('parseArgs defaults and overrides', () => {
  assert.deepEqual(parseArgs([], {}), { seed: '12345', chrome: 'google-chrome', replays: PERF_REPLAYS });
  assert.deepEqual(parseArgs(['abc'], { CHROME: '/x/chrome', PERF_REPLAYS: '3' }), { seed: 'abc', chrome: '/x/chrome', replays: 3 });
});

test('parseArgs rejects bad input', () => {
  assert.match(parseArgs(['1', '2'], {}).error, /extra argument/);
  assert.match(parseArgs([' '], {}).error, /blank/);
  assert.match(parseArgs([], { PERF_REPLAYS: '0' }).error, /PERF_REPLAYS/);
  assert.match(parseArgs([], { PERF_REPLAYS: 'x' }).error, /PERF_REPLAYS/);
});

test('buildReport prints records, spans and a verdict line', () => {
  const rec = { ms: 80, meshed: 1, remeshed: 0, verts: 900, ticks: 2, chunks: 4 };
  const r = buildReport([run(10), run(95, 1, [rec], [{ name: 'mesh', ms: 40 }]), run(90)]);
  assert.equal(r.verdict, 'over');
  assert.match(r.text, /hitch 80\.0 ms: 1 meshed/);
  assert.match(r.text, /mesh: 40\.0 ms/);
  assert.match(r.text, /^verdict: over \(advisory\)$/m);
});

test('buildReport copes with no spans and no replays', () => {
  assert.match(buildReport([run(10)]).text, /none reported/);
  assert.equal(buildReport([]).text, 'perf: no replays ran');
});
