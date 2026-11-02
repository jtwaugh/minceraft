import test from 'node:test';
import assert from 'node:assert/strict';
import { createSpans, SPAN_NAMES } from '../perfspans.js';

function fakePerf() {
  let t = 0;
  const marks = new Map();
  return {
    marks,
    measures: 0,
    advance: (ms) => { t += ms; },
    mark(n) { marks.set(n, t); },
    measure(n, a, b) { this.measures++; return { duration: marks.get(b) - marks.get(a) }; },
    clearMarks(n) { marks.delete(n); },
    clearMeasures() { this.measures = 0; },
  };
}

test('take sums count, total and max per name, then resets', () => {
  const perf = fakePerf();
  const { span, take } = createSpans(perf);
  span('mesh', () => perf.advance(3));
  assert.equal(span('mesh', () => { perf.advance(5); return 7; }), 7);
  span('tick', () => perf.advance(1));
  const a = take();
  assert.deepEqual(Object.keys(a), SPAN_NAMES);
  assert.deepEqual(a.mesh, { count: 2, total: 8, max: 5 });
  assert.deepEqual(a.tick, { count: 1, total: 1, max: 1 });
  assert.deepEqual(a.upload, { count: 0, total: 0, max: 0 });
  assert.deepEqual(take().mesh, { count: 0, total: 0, max: 0 });
});

test('marks and measures are cleared, even when the body throws', () => {
  const perf = fakePerf();
  const { span, take } = createSpans(perf);
  assert.throws(() => span('upload', () => { throw new Error('x'); }));
  assert.equal(perf.marks.size, 0);
  assert.equal(perf.measures, 0);
  assert.equal(take().upload.count, 1);
});
