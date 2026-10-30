// Run: node --test client/test/hintui.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { HINT_TEXT, hintLayout, mountHint, wrapText } from '../hintui.js';

const CELL = { w: 4, h: 6 };

test('wrapText keeps words whole and splits overlong ones', () => {
  assert.deepEqual(wrapText('aa bb cc', 5), ['aa bb', 'cc']);
  assert.deepEqual(wrapText('abcdefg', 3), ['abc', 'def', 'g']);
  assert.deepEqual(wrapText('', 5), []);
});

test('the hint text is unchanged', () => {
  assert.equal(HINT_TEXT, 'Click to look around. WASD move, Space jump (fly: up), Shift sneak (fly: down), F walk/fly, Esc releases the mouse.');
});

test('the panel rectangle fits inside the viewport and is centred at the bottom', () => {
  for (const [vw, vh] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440], [200, 100]]) {
    const l = hintLayout(CELL, HINT_TEXT, vw, vh);
    const w = l.w * l.scale, h = l.h * l.scale;
    assert.ok(l.x >= 0 && l.y >= 0 && l.x + w <= vw && l.y + h <= vh, `${vw}x${vh}`);
    assert.ok(Math.abs(l.x - (vw - w - l.x)) <= 1);
    assert.equal(l.lines.join(' '), HINT_TEXT);
  }
});

const SET = { palette: { '#': '#fff' }, batches: { b: { textures: { font_unknown: ['####', '####', '####', '####', '####', '####'] } } } };

function fakeDocument() {
  const make = (tag) => ({
    tag, style: {}, children: [], hidden: false, id: '',
    append(...k) { this.children.push(...k); },
    replaceChildren(...k) { this.children = k; },
    getContext: () => ({ fillRect() {} }),
  });
  return { body: make('body'), createElement: make };
}

test('mountHint draws a panel and one canvas per line, re-lays out on resize, hides', () => {
  const document = fakeDocument();
  const listeners = {};
  const view = { innerWidth: 390, innerHeight: 844, addEventListener: (t, f) => { listeners[t] = f; } };
  const hint = mountHint(document, SET, document.body, view);
  const root = document.body.children[0];
  assert.equal(root.id, 'hint');
  const narrow = root.children.length;
  assert.ok(narrow > 2);
  view.innerWidth = 3440; view.innerHeight = 1440;
  listeners.resize();
  assert.ok(root.children.length < narrow);
  hint.setHidden(true);
  assert.equal(root.hidden, true);
  hint.setHidden(false);
  assert.equal(root.hidden, false);
});
