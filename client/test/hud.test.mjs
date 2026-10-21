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

test('timer reports the phase and seconds left', () => {
  const t = (phase, secondsLeft) => hudModel({ belt, health: 20, maxHealth: 20, phase, secondsLeft }).timer;
  assert.deepEqual(t('gathering', 120), { phase: 'gathering', secondsLeft: 120 });
  assert.deepEqual(t('service', 1), { phase: 'service', secondsLeft: 1 });
});

test('slotView picks frame by selection and icon by item texture', async () => {
  const { slotView, hudScale } = await import('../hud.js');
  const { readFileSync } = await import('node:fs');
  const set = JSON.parse(readFileSync(new URL('../../viewer/textures.json', import.meta.url), 'utf8'));
  assert.deepEqual(slotView(set, 'sword', true), { frame: 'slot_selected', icon: 'sword' });
  assert.deepEqual(slotView(set, 'hoe', false), { frame: 'slot', icon: 'hoe' });
  assert.deepEqual(slotView(set, null, false), { frame: 'slot', icon: null });
  assert.deepEqual(slotView(set, 'no_such_item', true), { frame: 'slot_selected', icon: null });
  for (const [w, h] of [[0, 0], [100, 50], [1920, 1080]]) assert.ok(Number.isInteger(hudScale(w, h)) && hudScale(w, h) >= 1);
});

test('digitNames splits a number into digit textures', async () => {
  const { digitNames, heartNames } = await import('../hud.js');
  assert.deepEqual(digitNames(0), ['digit_0']);
  assert.deepEqual(digitNames(7), ['digit_7']);
  assert.deepEqual(digitNames(10), ['digit_1', 'digit_0']);
  assert.deepEqual(digitNames(125), ['digit_1', 'digit_2', 'digit_5']);
  assert.deepEqual(heartNames({ full: 1, half: 1, empty: 1 }), ['heart_full', 'heart_half', 'heart_empty']);
});
