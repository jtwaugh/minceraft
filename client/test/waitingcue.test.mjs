// Run: node --test client/test/waitingcue.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { CUE_TEXT, waitingForClick } from '../waitingcue.js';
import { createHintState } from '../hintui.js';

test('the cue text', () => {
  assert.equal(CUE_TEXT, 'Click to play');
});

test('waitingForClick is true only for unlocked, no console, no inventory, alive', () => {
  for (let bits = 0; bits < 16; bits++) {
    const s = { locked: !!(bits & 1), console: !!(bits & 2), inventory: !!(bits & 4), dead: !!(bits & 8) };
    assert.equal(waitingForClick(s), bits === 0, JSON.stringify(s));
  }
});

test('hint state folds on first lock, reopens on toggle, folds on the next lock', () => {
  const h = createHintState();
  assert.equal(h.open, true);
  h.update(false);
  assert.equal(h.open, true);
  h.update(true);
  assert.equal(h.open, false);
  h.update(false);
  assert.equal(h.open, false);
  h.toggle();
  assert.equal(h.open, true);
  h.update(false);
  assert.equal(h.open, true);
  h.update(true);
  assert.equal(h.open, false);
});
