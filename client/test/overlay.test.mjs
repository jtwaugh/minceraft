// Pure-JS checks for the aim read-out (the targeting and rules are tested in crates/game).
// Run: node --test client/test/overlay.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readAim, formatAim, heldFromQuery } from '../overlay.js';

function fakeWalk({ target = [0, 0, 0, 0, 0], box = [], rules = {}, last = -1 } = {}) {
  return {
    target: () => new Int32Array(target),
    target_box: () => new Float64Array(box),
    preview_rule: (secondary) => rules[secondary ? 'secondary' : 'primary'] ?? -1,
    rule_action: (i) => ['attack', 'till'][i] ?? '',
    last_rule: () => last,
  };
}

test('a block target reads its position, face and box', () => {
  const aim = readAim(fakeWalk({ target: [1, 3, 64, -2, 1], box: [3, 64, -2, 4, 65, -1], rules: { secondary: 1 } }), 0.5);
  assert.equal(aim.kind, 'block');
  assert.deepEqual(aim.pos, [3, 64, -2]);
  assert.equal(aim.face, 'top');
  assert.deepEqual(aim.box, [3, 64, -2, 4, 65, -1]);
  assert.equal(aim.primary, null);
  assert.deepEqual(aim.secondary, { index: 1, action: 'till' });
});

test('an entity target reads its id and no position', () => {
  const aim = readAim(fakeWalk({ target: [2, 7, 0, 0, 0], box: [0, 0, 0, 1, 1, 1], rules: { primary: 0 }, last: 0 }), 1);
  assert.equal(aim.kind, 'entity');
  assert.equal(aim.id, 7);
  assert.equal(aim.pos, null);
  assert.deepEqual(aim.last, { index: 0, action: 'attack' });
});

test('no target has no box and formats as none', () => {
  const aim = readAim(fakeWalk(), 0);
  assert.equal(aim.kind, 'none');
  assert.equal(aim.box, null);
  assert.equal(formatAim(aim), 'target none\nrule primary none | secondary none\nlast press none');
});

test('overlay text names the target and the matched rules', () => {
  const aim = readAim(fakeWalk({ target: [1, 1, 2, 3, 0], box: [1, 2, 3, 2, 3, 4], rules: { primary: 0, secondary: 1 } }), 0);
  assert.equal(formatAim(aim), 'target block 1 2 3 bottom\nrule primary #0 attack | secondary #1 till\nlast press none');
});

test('?tool values map to held strings', () => {
  assert.equal(heldFromQuery(null), 'empty');
  assert.equal(heldFromQuery('empty'), 'empty');
  assert.equal(heldFromQuery('hoe'), 'tool:hoe');
  assert.equal(heldFromQuery('item:seeds'), 'item:seeds');
});
