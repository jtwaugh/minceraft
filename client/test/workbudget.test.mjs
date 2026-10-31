// Per-frame work budgets and the DSL's overbudget event.
// Run: node --test client/test/workbudget.test.mjs (the DSL tests need ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { overBudget, MAX_REGIONS_PER_FRAME, MAX_CHUNKS_PER_TICK, MAX_VERTS_PER_FRAME } from '../workbudget.js';
import { createSession } from './dsl.mjs';

const BUDGETS = { regions: MAX_REGIONS_PER_FRAME, chunks: MAX_CHUNKS_PER_TICK, verts: MAX_VERTS_PER_FRAME };
const none = { regions: 0, chunks: 0, verts: 0 };

test('the budgets are positive whole numbers', () => {
  for (const v of Object.values(BUDGETS)) assert.ok(Number.isInteger(v) && v > 0);
});

test('no work and work at every budget are within budget', () => {
  assert.deepEqual(overBudget(none), []);
  assert.deepEqual(overBudget(BUDGETS), []);
});

for (const [k, budget] of Object.entries(BUDGETS)) {
  test(`${k}: under and at the budget pass, over it is reported`, () => {
    assert.deepEqual(overBudget({ ...none, [k]: budget - 1 }), []);
    assert.deepEqual(overBudget({ ...none, [k]: budget }), []);
    assert.deepEqual(overBudget({ ...none, [k]: budget + 1 }), [k]);
  });
}

test('every field over its budget is listed, in a fixed order', () => {
  const work = { verts: MAX_VERTS_PER_FRAME + 1, regions: MAX_REGIONS_PER_FRAME + 1, chunks: MAX_CHUNKS_PER_TICK + 1 };
  assert.deepEqual(overBudget(work), ['regions', 'chunks', 'verts']);
});

test('events.overbudget needs a streamed world', () => {
  const s = createSession(['walk']);
  assert.deepEqual(s.exec('expect events.overbudget = 0'), { ok: false, error: 'events.overbudget needs `start ... stream`' });
  s.close();
});

test('streaming at the budget of one region per frame stays within budget', () => {
  const s = createSession(['walk', 'stream']);
  assert.ok(s.exec('frame 50 x8').ok);
  assert.equal(s.read('events.overbudget'), 0);
  assert.ok(s.exec('expect no event overbudget').ok);
  s.close();
});

test('a stream meshing two regions a frame fires overbudget naming the regions', () => {
  const s = createSession(['walk', 'stream'], 'session', { streamBudget: 2 });
  assert.ok(s.exec('frame 50 x8').ok);
  assert.equal(s.read('regions.max_per_frame'), 2);
  assert.ok(s.read('events.overbudget') > 0);
  assert.ok(s.exec('expect event overbudget regions=2').ok);
  assert.match(s.exec('expect no event overbudget').error, /unexpected event overbudget/);
  s.close();
});
