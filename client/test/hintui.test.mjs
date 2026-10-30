// Run: node --test client/test/hintui.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { HINT_TEXT, hintLayout, hintScale, mountHint, wrapText } from '../hintui.js';
import { CORNER_MARGIN } from '../cornerbutton.js';

const CELL = { w: 4, h: 6 };

test('wrapText keeps words whole and splits overlong ones', () => {
  assert.deepEqual(wrapText('aa bb cc', 5), ['aa bb', 'cc']);
  assert.deepEqual(wrapText('abcdefg', 3), ['abc', 'def', 'g']);
  assert.deepEqual(wrapText('', 5), []);
});

test('the hint text is unchanged', () => {
  assert.equal(HINT_TEXT, 'Click to look around. WASD move, Space jump (fly: up), Shift sneak (fly: down), F walk/fly, Esc releases the mouse.');
});

test('the panel fits inside the viewport, right-aligned under the corner button', () => {
  for (const [vw, vh] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) {
    const l = hintLayout(CELL, HINT_TEXT, vw, vh);
    const w = l.w * l.scale, h = l.h * l.scale;
    assert.ok(l.x >= 0 && l.y >= 0 && l.x + w <= vw && l.y + h <= vh, `${vw}x${vh}`);
    assert.equal(l.x + w, vw - CORNER_MARGIN, `${vw}x${vh}`);
    assert.ok(l.y > CORNER_MARGIN && l.y < vh / 2, `${vw}x${vh}`);
    assert.ok(l.scale <= hintScale(vw, vh));
    assert.equal(l.lines.join(' '), HINT_TEXT);
  }
});

test('the hint is drawn at half the interface scale, rounded up', () => {
  assert.equal(hintScale(1920, 1080), 3);
  assert.equal(hintScale(1366, 768), 2);
});

const SET = { palette: { '#': '#fff' }, batches: { b: { textures: { font_unknown: ['####', '####', '####', '####', '####', '####'] } } } };

function fakeDocument() {
  const make = (tag) => ({
    tag, style: {}, children: [], hidden: false, id: '', listeners: {},
    append(...k) { this.children.push(...k); },
    replaceChildren(...k) { this.children = k; },
    addEventListener(t, f) { this.listeners[t] = f; },
    getContext: () => ({ fillRect() {} }),
  });
  return { body: make('body'), createElement: make };
}

test('mountHint draws a panel and one canvas per line and re-lays out on resize', () => {
  const document = fakeDocument();
  const listeners = {};
  const view = { innerWidth: 390, innerHeight: 844, addEventListener: (t, f) => { listeners[t] = f; } };
  mountHint(document, SET, document.body, view);
  const root = document.body.children.find((c) => c.id === 'hint');
  const narrow = root.children.length;
  assert.ok(narrow > 2);
  view.innerWidth = 3440; view.innerHeight = 1440;
  listeners.resize();
  assert.ok(root.children.length <= narrow);
});

test('the hint shows at start, folds on the first lock, and the corner button reopens it', () => {
  const document = fakeDocument();
  const view = { innerWidth: 1920, innerHeight: 1080, addEventListener() {} };
  const hint = mountHint(document, SET, document.body, view);
  const root = document.body.children.find((c) => c.id === 'hint');
  const button = document.body.children.find((c) => c.id === 'hint-button');
  const click = () => button.listeners.click({ stopPropagation() {} });
  hint.update(false);
  assert.equal(root.hidden, false);
  hint.update(true);
  assert.equal(root.hidden, true);
  hint.update(false); // Esc: stays folded
  assert.equal(root.hidden, true);
  click();
  assert.equal(root.hidden, false);
  hint.update(false);
  assert.equal(root.hidden, false);
  click();
  assert.equal(root.hidden, true);
  click();
  hint.update(true); // clicking back into the game folds it again
  assert.equal(root.hidden, true);
});
