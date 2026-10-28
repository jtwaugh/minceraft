// Pure-JS checks for the aim read-out (the targeting and rules are tested in crates/game).
// Run: node --test client/test/overlay.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readAim, formatAim, readBelt, formatBelt, readItems } from '../overlay.js';

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

test('the belt reads slots, selection and hands and formats as text', () => {
  const slots = ['', 'sword', 'hoe', '', ''];
  const belt = readBelt({
    belt_len: () => 5, belt_slot: (i) => slots[i], selected_slot: () => 1, hands: () => '', held: () => 'tool:sword', swing_phase: () => 0, use_phase: () => 0,
  });
  assert.deepEqual(belt, { slots: [null, 'sword', 'hoe', null, null], selected: 1, hands: null, held: 'tool:sword', swing: 0, use: 0 });
  assert.equal(formatBelt(belt), 'belt  1:- >2:sword  3:hoe  4:-  5:-\nhands empty | held tool:sword');
});

test('readItems reads nothing from an empty world', () => {
  assert.deepEqual(readItems({ items: () => new Float64Array(), item_names: () => [] }), []);
});

test('readItems reads one item per loose item, in order', () => {
  const walk = {
    items: () => new Float64Array([4, 1, 6, 2, 0.25]),
    item_names: () => ['stick'],
  };
  assert.deepEqual(readItems(walk), [{ id: 4, name: 'stick', pos: [1, 6, 2], size: 0.25 }]);
  const many = {
    items: () => new Float64Array([4, 1, 6, 2, 0.25, 9, 3.5, 7, -4, 0.25]),
    item_names: () => ['stick', 'wheat'],
  };
  assert.deepEqual(readItems(many), [
    { id: 4, name: 'stick', pos: [1, 6, 2], size: 0.25 },
    { id: 9, name: 'wheat', pos: [3.5, 7, -4], size: 0.25 },
  ]);
});
