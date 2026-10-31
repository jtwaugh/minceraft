import test from 'node:test';
import assert from 'node:assert/strict';
import { layoutChecks, MIN_SCALE, CROSSHAIR_CLEAR_MARGIN, MAX_HANDS_BELT_GAP } from '../hudcheck.js';
import { hudLayout } from '../hudlayout.js';
import { hudModel } from '../hud.js';

const r = (x, y, w, h) => ({ x, y, w, h });
const vp = { w: 100, h: 100 };
const clean = () => ({
  rects: { crosshair: r(42, 42, 16, 16), belt: r(10, 80, 80, 10), hearts: r(10, 66, 80, 9), timer: r(30, 8, 40, 10), hands: r(10, 60, 0, 0) },
});

test('a clean layout has no problems', () => {
  assert.deepEqual(layoutChecks(clean(), vp), []);
});

test('overlapping rectangles name both elements', () => {
  const l = clean();
  l.rects.hearts = r(10, 75, 80, 9);
  assert.deepEqual(layoutChecks(l, vp).map((c) => [c.check, c.elements]), [['overlap', ['belt', 'hearts']]]);
});

test('the crosshair is checked against every other element', () => {
  const l = clean();
  l.rects.timer = r(40, 40, 20, 20);
  assert.deepEqual(layoutChecks(l, vp).filter((c) => c.check === 'overlap').map((c) => c.elements), [['crosshair', 'timer']]);
});

test('touching edges and empty rectangles do not overlap', () => {
  const l = clean();
  l.rects.hearts = r(10, 70, 80, 10);
  l.rects.hands = r(10, 75, 0, 0);
  assert.deepEqual(layoutChecks(l, vp), []);
});

test('a rectangle past any viewport edge is reported', () => {
  for (const [rect, edge] of [[r(-1, 80, 20, 10), 'left'], [r(10, -1, 20, 5), 'top'], [r(90, 80, 20, 10), 'right'], [r(10, 95, 20, 10), 'bottom']]) {
    const l = clean();
    l.rects.belt = rect;
    const found = layoutChecks(l, vp).filter((c) => c.check === 'offscreen');
    assert.equal(found.length, 1);
    assert.deepEqual(found[0].elements, ['belt']);
    assert.match(found[0].detail, new RegExp(edge));
  }
});

const grid = (w, h) => Array(h).fill('a'.repeat(w));
const set = {
  palette: { a: '#ffffff' },
  batches: { ui: { textures: { slot: grid(10, 10), slot_selected: grid(10, 10), heart_full: grid(9, 9), heart_half: grid(9, 9), heart_empty: grid(9, 9), phase_gathering: grid(30, 6), sword: grid(8, 8), label_sword: grid(24, 6), ...Object.fromEntries([0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((d) => [`digit_${d}`, grid(4, 6)])) } } },
};
const model = hudModel({ belt: { slots: [null, 'sword', 'hoe', null, null], selected: 1, hands: 'sword' }, health: 19, maxHealth: 20 });

for (const [w, h] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) {
  test(`real hudLayout at ${w}x${h} gives a stable result`, () => {
    const a = layoutChecks(hudLayout(model, set, { w, h }), { w, h });
    assert.deepEqual(layoutChecks(hudLayout(model, set, { w, h }), { w, h }), a);
  });
}

const only = (l, check) => layoutChecks(l, vp).filter((c) => c.check === check);

test('scale below the minimum is a finding, or advisory on a portrait viewport', () => {
  const l = { ...clean(), scale: MIN_SCALE - 1 };
  assert.equal(only({ ...l, scale: MIN_SCALE }, 'scale').length, 0);
  assert.deepEqual(only(l, 'scale').map((c) => c.severity), ['finding']);
  assert.deepEqual(layoutChecks(l, { w: 100, h: 101 }).filter((c) => c.check === 'scale').map((c) => c.severity), ['advisory']);
});

test('clear zone: at, just inside and just outside the margin', () => {
  const m = CROSSHAIR_CLEAR_MARGIN;
  for (const [x, expected] of [[58 + m, 0], [58 + m - 1, 1], [58 + m + 1, 0]]) {
    const l = clean();
    l.rects.timer = r(x, 42, 10, 10);
    assert.equal(only(l, 'clear-zone').length, expected, `x=${x}`);
  }
});

test('hands to belt distance: just inside, at and just outside the limit', () => {
  const l = { ...clean(), scale: 2 };
  const limit = MAX_HANDS_BELT_GAP * 2;
  for (const [gap, expected] of [[limit - 1, 0], [limit, 0], [limit + 1, 1]]) {
    l.rects.hands = r(10, 80 - gap - 5, 20, 5);
    assert.equal(only(l, 'distance').length, expected, `gap=${gap}`);
  }
});

test('containment: a slot icon inside its slot is fine, one outside it is a finding', () => {
  const l = clean();
  l.rects.slots = [r(10, 80, 10, 10)];
  l.rects.slotNumbers = [r(11, 81, 4, 6)];
  l.rects.slotIcons = [r(11, 81, 8, 8)];
  assert.deepEqual(only(l, 'containment'), []);
  l.rects.slotIcons = [r(20, 81, 8, 8)];
  assert.deepEqual(only(l, 'containment').map((c) => c.elements), [['slotIcons[0]', 'slots[0]']]);
  l.rects.slotIcons = [null];
  assert.deepEqual(layoutChecks(l, vp), []);
});

test('containment: timer, hands and hearts parts must sit inside their parent', () => {
  const l = clean();
  l.rects.timer = { ...r(30, 8, 40, 10), phase: r(31, 9, 8, 8), digits: [r(40, 9, 4, 6), r(68, 9, 4, 6)] };
  l.rects.hearts = r(10, 66, 80, 9);
  l.rects.heartIcons = [r(10, 66, 9, 9), r(85, 66, 9, 9)];
  l.rects.hands = { ...r(10, 50, 10, 7), label: { ...r(10, 50, 10, 7), glyphs: [r(10, 50, 5, 7), r(16, 50, 5, 7)] } };
  assert.deepEqual(only(l, 'containment').map((c) => c.elements), [['timer.digits[1]', 'timer'], ['hands.label.glyphs[1]', 'hands'], ['heartIcons[1]', 'hearts']]);
});
