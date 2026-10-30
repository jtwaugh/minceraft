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

test('hands and slot labels are drawn through the pixel font', async () => {
  const { handsText, mountInventory } = await import('../inventoryui.js');
  assert.equal(handsText(''), 'Hands: empty');
  assert.equal(handsText('Hoe'), 'Hands: Hoe');
  const cell = ['....', '.aa.', '....'];
  const set = { palette: { a: '#fff' }, batches: { b: { textures: { font_unknown: cell, slot: ['..'], panel_c: ['.'] } } } };
  const make = (tag) => ({
    tag, style: {}, children: [], hidden: false, textContent: '', width: 0, height: 0,
    append(...k) { this.children.push(...k); },
    replaceChildren(...k) { this.children = k; },
    querySelector: () => make('div'),
    getContext: () => ({ drawImage() {}, fillRect() {}, putImageData() {} }),
    toDataURL: () => 'data:,',
  });
  globalThis.ImageData ??= class { constructor(d, w, h) { Object.assign(this, { data: d, width: w, height: h }); } };
  const strings = [];
  const doc = { body: make('body'), createElement: (t) => { const e = make(t); Object.defineProperty(e, 'innerHTML', { set() {} }); return e; } };
  const origCreate = doc.createElement;
  doc.createElement = (t) => { const e = origCreate(t); if (t === 'canvas') strings.push(e); return e; };
  const k = inventoryScale(400, 300);
  const view = mountInventory(doc, set, doc.body, { innerWidth: 400, innerHeight: 300 });
  view.update(true, { selected: 0, slots: [{ name: 'hoe', count: 1 }] });
  assert.ok(strings.some((c) => c.width === 'Hands: empty'.length * 4 * k && c.height === 3 * k), strings.map((c) => c.width).join());
});
