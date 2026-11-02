import test from 'node:test';
import assert from 'node:assert/strict';
import { hudLayout, BOTTOM_MARGIN, SLOT_GAP, HEART_GAP, HEARTS_BELT_GAP, TOP_MARGIN, TIMER_PAD_X, TIMER_PAD_Y, LABEL_GAP, HANDS_GAP } from '../hudlayout.js';
import { hudModel } from '../hud.js';
import { readFileSync } from 'node:fs';
import { layoutChecks } from '../hudcheck.js';
import { BELT_PAD, BELT_MAX_WIDTH_FRACTION } from '../uipx.js';

const grid = (w, h) => Array(h).fill('a'.repeat(w));
const set = {
  palette: { a: '#ffffff' },
  batches: { ui: { textures: { slot: grid(10, 10), slot_selected: grid(10, 10), heart_full: grid(9, 9), heart_half: grid(9, 9), heart_empty: grid(9, 9), crosshair: grid(7, 7), sun: grid(8, 8), moon: grid(8, 8), sword: grid(8, 8), panel_c: grid(4, 4), font_unknown: grid(5, 7), font_s: grid(5, 7), font_w: grid(5, 7), font_o: grid(5, 7), font_r: grid(5, 7), font_d: grid(5, 7), ...Object.fromEntries([0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((d) => [`digit_${d}`, grid(4, 6)])) } } },
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
    assert.deepEqual(rects.crosshair, { x: Math.floor((w - 7 * scale) / 2), y: Math.floor((h - 7 * scale) / 2), w: 7 * scale, h: 7 * scale });
    const bp = rects.beltPanel;
    const hp = rects.handsPanel;
    const rowEnd = hp.x + hp.w;
    if (Math.floor((w - bp.w) / 2) + (rowEnd - bp.x) <= w) {
      assert.equal(bp.x, Math.floor((w - bp.w) / 2));
      if (w >= 390) assert.ok(Math.abs(bp.x + bp.w / 2 - w / 2) <= 1);
    } else {
      assert.equal(bp.x, Math.floor((w - Math.max(rowEnd - bp.x, rects.hearts.w)) / 2));
    }
    if (bp.x + rects.hearts.w <= w) assert.equal(rects.hearts.x, bp.x);
    else assert.equal(rects.hearts.x, Math.max(0, w - rects.hearts.w));
    assert.equal(bp.y + bp.h, h - BOTTOM_MARGIN);
    assert.equal(rects.hearts.y + rects.hearts.h + HEARTS_BELT_GAP * scale, bp.y);
    // The strip encloses the slot row by BELT_PAD texels.
    assert.deepEqual(bp, { x: rects.belt.x - BELT_PAD * scale, y: rects.belt.y - BELT_PAD * scale, w: rects.belt.w + 2 * BELT_PAD * scale, h: rects.belt.h + 2 * BELT_PAD * scale });
    // Slots tile the belt left to right with the gap between them.
    assert.equal(rects.slots[0].x, rects.belt.x);
    rects.slots.slice(1).forEach((s, i) => assert.equal(s.x, rects.slots[i].x + rects.slots[i].w + SLOT_GAP * scale));
  });
}

for (const n of [5, 3, 9]) {
  for (const [w, h] of [[80, 45], [1366, 768]]) {
    test(`belt strip and slot numbers for ${n} slots at ${w}x${h}`, () => {
      const m = hudModel({ belt: { slots: Array(n).fill(null), selected: 0, hands: null }, health: 20, maxHealth: 20 });
      const { scale, rects } = hudLayout(m, set, { w, h });
      assert.equal(rects.slots.length, n);
      assert.equal(rects.slotNumbers.length, n);
      rects.slots.forEach((s, i) => {
        const r = rects.slotNumbers[i];
        assert.equal(r.n, i + 1);
        assert.equal(r.glyph, `digit_${i + 1}`);
        assert.ok(r.x >= s.x && r.y >= s.y && r.x + r.w <= s.x + s.w && r.y + r.h <= s.y + s.h, `number ${i + 1} inside its slot`);
        assert.ok(s.x >= rects.beltPanel.x && s.x + s.w <= rects.beltPanel.x + rects.beltPanel.w);
        assert.ok(s.y >= rects.beltPanel.y && s.y + s.h <= rects.beltPanel.y + rects.beltPanel.h);
      });
      if (n === 5) {
        assert.equal(scale, w === 80 ? 1 : 4);
        assert.ok(rects.beltPanel.w <= w * BELT_MAX_WIDTH_FRACTION || scale === 1);
      }
    });
  }
}

test('belt strip is within the width share and the layout has no problems at the design sizes', () => {
  for (const [w, h] of sweep.slice(0, 4)) {
    const layout = hudLayout(model, set, { w, h });
    assert.ok(layout.rects.beltPanel.w <= w * BELT_MAX_WIDTH_FRACTION, `${w}x${h}`);
    assert.deepEqual(layoutChecks(layout, { w, h }), []);
  }
});

test('slot icons are centred in their slots on whole texels, null when empty or without a texture', () => {
  const odd = { ...set, batches: { ui: { textures: { ...set.batches.ui.textures, hoe: grid(5, 7) } } } };
  for (const [w, h] of [[80, 45], [1366, 768]]) {
    const { scale, rects } = hudLayout(model, odd, { w, h });
    assert.equal(rects.slotIcons.length, rects.slots.length);
    assert.deepEqual([rects.slotIcons[0], rects.slotIcons[3], rects.slotIcons[4]], [null, null, null]);
    const [s1, s2] = [rects.slots[1], rects.slots[2]];
    assert.deepEqual(rects.slotIcons[1], { x: s1.x + scale, y: s1.y + scale, w: 8 * scale, h: 8 * scale });
    assert.deepEqual(rects.slotIcons[2], { x: s2.x + 2 * scale, y: s2.y + scale, w: 5 * scale, h: 7 * scale });
    // Without a hoe texture its slot shows the frame only.
    assert.equal(hudLayout(model, set, { w, h }).rects.slotIcons[2], null);
  }
});

test('the hands strip falls back to a centred column when it would run off the viewport', () => {
  const view = { w: 80, h: 45 };
  const { rects } = hudLayout(model, set, view);
  const hp = rects.handsPanel, bp = rects.beltPanel;
  assert.ok(Math.floor((view.w - bp.w) / 2) + (hp.x + hp.w - bp.x) > view.w, 'centring the belt would overflow');
  assert.equal(bp.x, Math.floor((view.w - Math.max(hp.x + hp.w - bp.x, rects.hearts.w)) / 2));
  assert.ok(hp.x >= 0 && hp.x + hp.w <= view.w, "hands strip inside the viewport");
  assert.ok(rects.hearts.x >= 0);
});

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
    assert.deepEqual([empty.rects.hands.w, empty.rects.hands.h, empty.rects.hands.label, empty.rects.handsSlot.icon], [0, 0, null, null]);
    const { scale, rects } = hudLayout(withTimer(7, 'sword'), set, { w, h });
    // The hands slot is always shown: a slot frame on its own strip, right of the belt's and level with it.
    const bp = rects.beltPanel, hp = rects.handsPanel, hs = rects.handsSlot;
    assert.deepEqual(empty.rects.handsPanel, hp);
    assert.deepEqual(hp, { x: bp.x + bp.w + HANDS_GAP * scale, y: bp.y, w: (10 + 2 * BELT_PAD) * scale, h: bp.h });
    assert.deepEqual([hs.x, hs.y, hs.w, hs.h], [hp.x + BELT_PAD * scale, hp.y + BELT_PAD * scale, 10 * scale, 10 * scale]);
    // The carried item's icon sits inside the frame, centred on whole texels.
    assert.deepEqual(hs.icon, { x: hs.x + scale, y: hs.y + scale, w: 8 * scale, h: 8 * scale });
    // The name row shrinks in whole texels only when it would reach the crosshair's clear zone.
    const hd = rects.hands, k = hd.scale;
    assert.ok(Number.isInteger(k) && k >= 1 && k <= scale);
    assert.ok(k === 1 || hd.y >= rects.crosshair.y + rects.crosshair.h + 8, 'clear of the crosshair zone');
    const labelW = 'sword'.length * 5 * k;
    assert.equal(hd.x + hd.w, hp.x + hp.w);
    assert.deepEqual([hd.w, hd.h], [labelW, 7 * k]);
    // Beside the hearts when it fits, otherwise stacked above them; never overlapping.
    const stacked = hd.y + hd.h === rects.hearts.y;
    assert.ok(stacked || hd.y + hd.h === rects.hearts.y + rects.hearts.h);
    assert.ok(stacked || hd.x >= rects.hearts.x + rects.hearts.w);
    assert.deepEqual([hd.label.x, hd.label.y, hd.label.w, hd.label.h], [hd.x, hd.y, labelW, 7 * k]);
    assert.equal(hd.label.glyphs.length, 5);
    hd.label.glyphs.forEach((g, i) => assert.deepEqual([g.x, g.w], [hd.label.x + i * 5 * k, 5 * k]));
    assert.equal(hd.label.glyphs.at(-1).x + 5 * k, hd.label.x + hd.label.w);
    // The belt does not move when the hands fill.
    assert.deepEqual(rects.beltPanel, empty.rects.beltPanel);
  });
}

test('hands stack above the hearts when a long name would meet them', () => {
  const { scale, rects } = hudLayout(withTimer(7, 'sword_sword_sword'), set, { w: 390, h: 844 });
  assert.equal(rects.hands.y + rects.hands.h, rects.hearts.y);
  assert.deepEqual(layoutChecks({ scale, rects }, { w: 390, h: 844 }).filter((c) => c.check === 'overlap'), []);
});

test('an unknown character is measured with font_unknown', () => {
  const { rects } = hudLayout(withTimer(7, 'sw?'), set, { w: 800, h: 450 });
  assert.equal(rects.hands.label.w, 3 * 5 * rects.hands.scale);
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
      assert.equal(layout.rects.hands.x + layout.rects.hands.w, layout.rects.handsPanel.x + layout.rects.handsPanel.w);
    }
  }
});

const withHoe = { ...set, batches: { ui: { textures: { ...set.batches.ui.textures, hoe: grid(6, 8) } } } };

test('the ready tool fills the hands frame when the hands are empty, without a label', () => {
  const { rects } = hudLayout({ ...withTimer(7), ready: 'hoe' }, withHoe, { w: 1366, h: 768 });
  const hs = rects.handsSlot, ic = hs.icon;
  assert.equal(ic.w % (ic.h / 8), 0);
  const k = ic.h / 8;
  assert.deepEqual(ic, { x: hs.x + Math.floor((10 - 6) / 2) * k, y: hs.y + k, w: 6 * k, h: 8 * k });
  assert.deepEqual([rects.hands.w, rects.hands.h, rects.hands.label], [0, 0, null]);
});

test('a carried item wins over the ready tool', () => {
  const held = hudLayout(withTimer(7, 'sword'), set, { w: 1366, h: 768 }).rects.handsSlot.icon;
  const both = hudLayout({ ...withTimer(7, 'sword'), ready: 'hoe' }, withHoe, { w: 1366, h: 768 }).rects.handsSlot.icon;
  assert.deepEqual(both, held);
  assert.equal(held.w, held.h);
});

test('the belt panel is centred within 1px at 1366x768 and 390x844 and the hearts stay on screen', () => {
  for (const [w, h] of [[1366, 768], [390, 844]]) {
    const { rects } = hudLayout(model, set, { w, h });
    const bp = rects.beltPanel;
    assert.ok(Math.abs(bp.x + bp.w / 2 - w / 2) <= 1, `${w}x${h}`);
    assert.ok(rects.hearts.x >= 0 && rects.hearts.x + rects.hearts.w <= w, `${w}x${h}`);
    assert.deepEqual(layoutChecks(hudLayout(model, set, { w, h }), { w, h }), []);
  }
  const wide = hudLayout(model, set, { w: 1366, h: 768 }).rects;
  assert.equal(wide.hearts.x, wide.beltPanel.x);
});
