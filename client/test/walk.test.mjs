// Pure-JS checks for the walking-mode wiring (the sim itself is tested in crates/game).
// Run: node --test client/test/walk.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { stepWalk } from '../walk.js';

function fakeWalk({ world = true } = {}) {
  const w = {
    ticks: [], alphas: [], world,
    has_world: () => w.world,
    tick: (t) => w.ticks.push(t),
    eye: (a) => { w.alphas.push(a); return new Float64Array([1, 2.62, 3]); },
  };
  return w;
}

test('each clock tick feeds one sampled input and the eye is read at alpha', () => {
  const w = fakeWalk();
  let n = 0;
  const eye = stepWalk(w, 3, 0.25, () => ({ n: n++ }));
  assert.deepEqual(w.ticks, [{ n: 0 }, { n: 1 }, { n: 2 }]);
  assert.deepEqual(w.alphas, [0.25]);
  assert.deepEqual(eye, [1, 2.62, 3]);
});

test('no ticks run, and no input is drained, before terrain is loaded', () => {
  const w = fakeWalk({ world: false });
  let drained = 0;
  stepWalk(w, 4, 0, () => { drained++; });
  assert.equal(w.ticks.length, 0);
  assert.equal(drained, 0);
});
