import test from 'node:test';
import assert from 'node:assert/strict';
import { hudLayout, CROSSHAIR_SIZE, BOTTOM_MARGIN, SLOT_GAP, HEART_GAP, HEARTS_BELT_GAP, TOP_MARGIN, TIMER_PAD_X, TIMER_PAD_Y, LABEL_GAP } from '../hudlayout.js';
import { hudModel } from '../hud.js';
import { readFileSync } from 'node:fs';
import { layoutChecks } from '../hudcheck.js';

const grid = (w, h) => Array(h).fill('a'.repeat(w));
const set = {
  palette: { a: '#ffffff' },
  batches: { ui: { textures: { slot: grid(10, 10), slot_selected: grid(10, 10), heart_full: grid(9, 9), heart_half: grid(9, 9), heart_empty: grid(9, 9), sun: grid(8, 8), moon: grid(8, 8), sword: grid(8, 8), font_unknown: grid(5, 7), font_s: grid(5, 7), font_w: grid(5, 7), font_o: grid(5, 7), font_r: grid(5, 7), font_d: grid(5, 7), ...Object.fromEntries([0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((d) => [`digit_${d}`, grid(4, 6)])) } } },
};
const belt = { slots: [null, 'sword', 'hoe', null, null], selected: 1, hands: null };
const model = hudModel({ belt, health: 19, maxHealth: 20 });

const sweep = [
  [390, 844, 'phone portrait'],
  [1366, 768, 'laptop'],
  [1920, 1080, 'full hd'],
  [3440, 1440, 'ultrawide'],
  [80, 45, 'minimum, scale 1'],
];

for (const [w, h, name] of sweep) {
  test(`hudLayout at ${w}x${h} (${name})`, () => {
    const { scale, rects } = hudLayout(model, set, { w, h });
    assert.ok(Number.isInteger(scale) && scale >= 1);
    const all = [rects.crosshair, rects.belt, rects.hearts, ...rects.slots, ...rects.heartIcons];
    for (const r of all) for (const v of Object.values(r)) assert.ok(Number.isInteger(v));
    // Sizes are exact multiples of the texture sizes.
    assert.equal(rects.slots.length, 5);
    for (const s of rects.slots) assert.deepEqual([s.w, s.h], [10 * scale, 10 * scale]);
    assert.equal(rects.heartIcons.length, 10);
    for (const s of rects.heartIcons) assert.deepEqual([s.w, s.h], [9 * scale, 9 * scale]);
    assert.equal(rects.belt.w, (5 * 10 + 4 * SLOT_GAP) * scale);
    assert.equal(rects.hearts.w, (10 * 9 + 9 * HEART_GAP) * scale);
    // Crosshair centred; belt centred at the bottom margin; hearts above the belt.
    assert.deepEqual(rects.crosshair, { x: Math.floor((w - CROSSHAIR_SIZE) / 2), y: Math.floor((h - CROSSHAIR_SIZE) / 2), w: CROSSHAIR_SIZE, h: CROSSHAIR_SIZE });
    assert.equal(rects.belt.x, Math.floor((w - Math.max(rects.belt.w, rects.hearts.w)) / 2));
    assert.equal(rects.hearts.x, rects.belt.x);
    assert.equal(rects.belt.y + rects.belt.h, h - BOTTOM_MARGIN);
    assert.equal(rects.hearts.y + rects.hearts.h + HEARTS_BELT_GAP * scale, rects.belt.y);
    // Slots tile the belt left to right with the gap between them.
    assert.equal(rects.slots[0].x, rects.belt.x);
    rects.slots.slice(1).forEach((s, i) => assert.equal(s.x, rects.slots[i].x + rects.slots[i].w + SLOT_GAP * scale));
  });
}

test('minimum viewport has hud scale 1', () => {
  assert.equal(hudLayout(model, set, { w: 80, h: 45 }).scale, 1);
  assert.equal(hudLayout(model, set, { w: 10, h: 10 }).scale, 1);
});

test('layout depends only on model, tileset and viewport', () => {
  const a = hudLayout(model, set, { w: 1366, h: 768 });
  assert.deepEqual(hudLayout(model, set, { w: 1366, h: 768 }), a);
});

const withTimer = (secondsLeft, hands = null) => hudModel({ belt: { ...belt, hands }, health: 19, maxHealth: 20, phase: 'gathering', secondsLeft });

for (const [w, h, name] of sweep) {
  for (const phase of ['gathering', 'service']) {
    for (const secs of [7, 42, 125]) {
      test(`${phase} timer with ${String(secs).length} digit(s) at ${w}x${h} (${name})`, () => {
        const m = hudModel({ belt, health: 19, maxHealth: 20, phase, secondsLeft: secs });
        const { scale, rects: { timer } } = hudLayout(m, set, { w, h });
        const n = String(secs).length;
        assert.equal(timer.y, TOP_MARGIN);
        assert.equal(timer.x, Math.floor((w - timer.w) / 2));
        assert.equal(timer.w, (8 + n * 4 + n * LABEL_GAP + 2 * TIMER_PAD_X) * scale);
        assert.equal(timer.h, (8 + 2 * TIMER_PAD_Y) * scale);
        assert.deepEqual(timer.phase, { x: timer.x + TIMER_PAD_X * scale, y: timer.y + TIMER_PAD_Y * scale, w: 8 * scale, h: 8 * scale });
        assert.equal(timer.digits.length, n);
        assert.equal(timer.digits[0].x, timer.phase.x + timer.phase.w + LABEL_GAP * scale);
        timer.digits.forEach((d) => assert.deepEqual([d.w, d.h, d.y], [4 * scale, 6 * scale, timer.phase.y + scale]));
        const last = timer.digits.at(-1);
        assert.equal(timer.x + timer.w - (last.x + last.w), TIMER_PAD_X * scale);
      });
    }
  }

  test(`hands at ${w}x${h} (${name})`, () => {
    const empty = hudLayout(withTimer(7), set, { w, h });
    assert.deepEqual([empty.rects.hands.w, empty.rects.hands.h, empty.rects.hands.icon, empty.rects.hands.label], [0, 0, null, null]);
    const { scale, rects } = hudLayout(withTimer(7, 'sword'), set, { w, h });
    const hd = rects.hands;
    // The row shrinks in whole texels only when it would reach the crosshair's clear zone.
    const k = hd.icon.h / 8;
    assert.ok(Number.isInteger(k) && k >= 1 && k <= scale && (k === scale || h >= 1000));
    const labelW = 'sword'.length * 5 * k;
    assert.equal(hd.x + hd.w, rects.belt.x + rects.belt.w);
    assert.deepEqual([hd.w, hd.h], [8 * k + LABEL_GAP * k + labelW, 8 * k]);
    // Beside the hearts when it fits, otherwise stacked above them; never overlapping.
    const stacked = hd.y + hd.h === rects.hearts.y;
    assert.ok(stacked || hd.y + hd.h === rects.hearts.y + rects.hearts.h);
    assert.ok(stacked || hd.x >= rects.hearts.x + rects.hearts.w);
    assert.deepEqual(hd.icon, { x: hd.x, y: hd.y, w: 8 * k, h: 8 * k });
    assert.deepEqual([hd.label.x, hd.label.y, hd.label.w, hd.label.h], [hd.x + (8 + LABEL_GAP) * k, hd.y, labelW, 7 * k]);
    assert.equal(hd.label.glyphs.length, 5);
    hd.label.glyphs.forEach((g, i) => assert.deepEqual([g.x, g.w], [hd.label.x + i * 5 * k, 5 * k]));
    assert.equal(hd.label.glyphs.at(-1).x + 5 * k, hd.label.x + hd.label.w);
    // The belt does not move when the hands fill.
    assert.deepEqual(rects.belt, empty.rects.belt);
  });
}

test('hands stack above the hearts when a long name would meet them', () => {
  const { scale, rects } = hudLayout(withTimer(7, 'sword_sword_sword'), set, { w: 390, h: 844 });
  assert.equal(rects.hands.y + rects.hands.h, rects.hearts.y);
  assert.deepEqual(layoutChecks({ scale, rects }, { w: 390, h: 844 }).filter((c) => c.check === 'overlap'), []);
});

test('an unknown character is measured with font_unknown', () => {
  const { scale, rects } = hudLayout(withTimer(7, 'sw?'), set, { w: 800, h: 450 });
  assert.equal(rects.hands.label.w, 3 * 5 * scale);
  assert.equal(rects.hands.label.glyphs[2].glyph, 'font_unknown');
});

test('a missing phase icon leaves only the digits', () => {
  const { rects } = hudLayout(withTimer(7), { ...set, batches: { ui: { textures: { ...set.batches.ui.textures, sun: undefined } } } }, { w: 800, h: 450 });
  assert.equal(rects.timer.phase, null);
  assert.equal(rects.timer.digits.length, 1);
});

test('the real tileset with an item in hands has no layout problems across the sweep', () => {
  const real = JSON.parse(readFileSync(new URL('../../viewer/textures.json', import.meta.url), 'utf8'));
  for (const [w, h] of sweep.slice(0, 4)) {
    for (const hands of ['sword', 'wooden_pickaxe']) {
      const layout = hudLayout(withTimer(42, hands), real, { w, h });
      assert.deepEqual(layoutChecks(layout, { w, h }), [], `${hands} at ${w}x${h}`);
      assert.equal(layout.rects.hands.x + layout.rects.hands.w, layout.rects.belt.x + layout.rects.belt.w);
    }
  }
});
