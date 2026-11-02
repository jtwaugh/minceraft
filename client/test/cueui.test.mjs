// Run: node --test client/test/cueui.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { cueLayout, mountCue } from '../cueui.js';
import { CUE_TEXT } from '../waitingcue.js';

const CELL = { w: 4, h: 6 };

test('the cue panel fits and is centred at the hudsizes viewports', () => {
  for (const [vw, vh] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) {
    const l = cueLayout(CELL, CUE_TEXT, vw, vh);
    const w = l.w * l.scale, h = l.h * l.scale;
    assert.ok(l.x >= 0 && l.y >= 0 && l.x + w <= vw && l.y + h <= vh, `${vw}x${vh}`);
    assert.ok(Math.abs(l.x + w / 2 - vw / 2) <= l.scale, `${vw}x${vh}`);
    assert.ok(Math.abs(l.y + h / 2 - vh / 2) <= l.scale, `${vw}x${vh}`);
  }
});

const SET = { palette: { '#': '#fff' }, batches: { b: { textures: { font_unknown: ['####', '####', '####', '####', '####', '####'] } } } };

test('mountCue follows the waiting flag', () => {
  const make = (tag) => ({
    tag, style: {}, children: [], hidden: false, id: '',
    append(...k) { this.children.push(...k); },
    replaceChildren(...k) { this.children = k; },
    addEventListener() {},
    getContext: () => ({ fillRect() {} }),
  });
  const document = { body: make('body'), createElement: make };
  const cue = mountCue(document, SET, document.body, { innerWidth: 1920, innerHeight: 1080 });
  const root = document.body.children.find((c) => c.id === 'cue');
  assert.equal(root.hidden, true);
  cue.update(true);
  assert.equal(root.hidden, false);
  cue.update(false);
  assert.equal(root.hidden, true);
});
