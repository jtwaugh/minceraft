import test from 'node:test';
import assert from 'node:assert/strict';
import { hudScale } from '../hud.js';
import { inventoryScale, itemLabel, panelSize } from '../inventoryui.js';

test('inventoryScale is an integer >= 1 for any viewport', () => {
  for (const [w, h] of [[0, 0], [-1, 5], [NaN, 5], [99, 59], [100, 60], [1920, 1080], [1920, 300]]) {
    const s = inventoryScale(w, h);
    assert.ok(Number.isInteger(s) && s >= 1, `${w}x${h} -> ${s}`);
  }
  assert.equal(inventoryScale(1920, 1080), 15);
  assert.equal(inventoryScale(1920, 300), 4);
});

test('itemLabel turns names into display text', () => {
  assert.equal(itemLabel('raw_beef'), 'Raw beef');
  assert.equal(itemLabel('hoe'), 'Hoe');
  assert.equal(itemLabel(null), '');
});

test('panelSize adds padding around and between slots', () => {
  assert.deepEqual(panelSize(5, 10, 10, 3), { w: 68, h: 16 });
});

test('inventoryScale agrees with hudScale at the standard sizes', () => {
  for (const [w, h] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) assert.equal(inventoryScale(w, h), hudScale(w, h));
});
