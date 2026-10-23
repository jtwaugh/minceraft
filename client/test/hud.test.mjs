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

// Just enough DOM for mountHud: elements with children and a canvas stub.
function fakeDocument() {
  const make = (tag) => ({
    tag, style: {}, children: [], hidden: false, id: '', className: '', textContent: '',
    append(...kids) { this.children.push(...kids); },
    replaceChildren(...kids) { this.children = kids; },
    getContext: () => ({ putImageData() {} }),
  });
  return { body: make('body'), createElement: make };
}

test('mountHud places every element from its hudLayout rectangle', async () => {
  const { mountHud } = await import('../hud.js');
  const { hudLayout } = await import('../hudlayout.js');
  globalThis.ImageData = class { constructor(d, w, h) { Object.assign(this, { data: d, width: w, height: h }); } };
  const grid = (w, h) => Array(h).fill('a'.repeat(w));
  const set = { palette: { a: '#ffffff' }, batches: { ui: { textures: { slot: grid(10, 10), slot_selected: grid(10, 10), sword: grid(8, 8), heart_full: grid(9, 9), heart_half: grid(9, 9), heart_empty: grid(9, 9), ...Object.fromEntries([0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((d) => [`digit_${d}`, grid(4, 6)])) } } } };
  const doc = fakeDocument();
  const view = { innerWidth: 800, innerHeight: 450 };
  const hud = mountHud(doc, set, doc.body, view);
  const m = hudModel({ belt, health: 19, maxHealth: 20, secondsLeft: 42 });
  const px = (r, ox = 0, oy = 0) => ({ left: `${r.x - ox}px`, top: `${r.y - oy}px`, width: `${r.w}px`, height: `${r.h}px` });
  const box = (el) => ({ left: el.style.left, top: el.style.top, width: el.style.width, height: el.style.height });
  for (const size of [[800, 450], [390, 844]]) {
    [view.innerWidth, view.innerHeight] = size;
    hud.update(m);
    const { rects } = hudLayout(m, set, { w: size[0], h: size[1] });
    const [cross, timer, hands, hearts, beltEl] = doc.body.children[1].children;
    assert.deepEqual(box(cross), px(rects.crosshair));
    assert.deepEqual(box(timer), px(rects.timer));
    assert.deepEqual(timer.children.slice(1).map(box), rects.timer.digits.map((r) => px(r, rects.timer.x, rects.timer.y)));
    assert.deepEqual(box(hands), px(rects.hands));
    assert.deepEqual(box(hearts), px(rects.hearts));
    assert.deepEqual(hearts.children.map(box), rects.heartIcons.map((r) => px(r, rects.hearts.x, rects.hearts.y)));
    assert.deepEqual(box(beltEl), px(rects.belt));
    assert.deepEqual(beltEl.children.map(box), rects.slots.map((r) => px(r, rects.belt.x, rects.belt.y)));
  }
});
