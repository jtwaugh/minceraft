// Pure-JS checks for the debug mode gate.
// Run: node --test client/test/debug.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { debugEnabled } from '../debug.js';

test('debug=1 or a scene turns debug mode on', () => {
  assert.equal(debugEnabled('?debug=1'), true);
  assert.equal(debugEnabled('?scene=raft'), true);
  assert.equal(debugEnabled('?seed=7&debug=1&mode=fly'), true);
});

test('anything else leaves it off', () => {
  assert.equal(debugEnabled(''), false);
  assert.equal(debugEnabled('?debug=0'), false);
  assert.equal(debugEnabled('?mode=fly'), false);
  assert.equal(debugEnabled('?scene='), false);
});
