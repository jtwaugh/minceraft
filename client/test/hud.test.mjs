import test from 'node:test';
import assert from 'node:assert/strict';
import { hudModel } from '../hud.js';

const belt = { slots: [null, 'sword', 'hoe', null, null], selected: 1, hands: null, held: 'tool:sword' };

test('hearts for health 0, 1, 19 and 20', () => {
  const hearts = (health) => hudModel({ belt, health, maxHealth: 20 }).hearts;
  assert.deepEqual(hearts(0), { full: 0, half: 0, empty: 10 });
  assert.deepEqual(hearts(1), { full: 0, half: 1, empty: 9 });
  assert.deepEqual(hearts(19), { full: 9, half: 1, empty: 0 });
  assert.deepEqual(hearts(20), { full: 10, half: 0, empty: 0 });
});

test('health outside 0..max is clamped', () => {
  assert.deepEqual(hudModel({ belt, health: 99, maxHealth: 20 }).hearts, { full: 10, half: 0, empty: 0 });
});

test('belt slots, selection and carried item', () => {
  const m = hudModel({ belt: { ...belt, hands: 'seeds' }, health: 20, maxHealth: 20 });
  assert.equal(m.crosshair, true);
  assert.equal(m.selected, 1);
  assert.deepEqual(m.slots.map((s) => s.name), [null, 'sword', 'hoe', null, null]);
  assert.deepEqual(m.slots.map((s) => s.selected), [false, true, false, false, false]);
  assert.equal(m.hands, 'seeds');
});
