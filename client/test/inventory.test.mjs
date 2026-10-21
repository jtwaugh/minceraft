import test from 'node:test';
import assert from 'node:assert/strict';
import { inventoryModel } from '../inventory.js';

const set = { batches: { b: { textures: { sword: ['.'], hoe: ['.'] } } } };
const labelOf = (name) => name.toUpperCase();
const belt = { slots: [null, 'sword', 'hoe', 'mystery', null], selected: 1, hands: 'sword', held: 'tool:sword' };

test('lists all five slots with labels, selection and icons', () => {
  const m = inventoryModel(set, belt, labelOf);
  assert.equal(m.slots.length, 5);
  assert.deepEqual(m.slots.map((s) => s.label), [null, 'SWORD', 'HOE', 'MYSTERY', null]);
  assert.deepEqual(m.slots.map((s) => s.selected), [false, true, false, false, false]);
  assert.deepEqual(m.slots.map((s) => s.frame), ['slot', 'slot_selected', 'slot', 'slot', 'slot']);
  assert.deepEqual(m.slots.map((s) => s.icon), [null, 'sword', 'hoe', null, null]);
  assert.deepEqual(m.hands, { name: 'sword', label: 'SWORD' });
});

test('an item with no texture keeps its label and has no icon', () => {
  const s = inventoryModel(set, belt, labelOf).slots[3];
  assert.deepEqual([s.name, s.label, s.icon], ['mystery', 'MYSTERY', null]);
});

test('empty hands have no label', () => {
  const m = inventoryModel(set, { ...belt, hands: null }, labelOf);
  assert.deepEqual(m.hands, { name: null, label: null });
});
