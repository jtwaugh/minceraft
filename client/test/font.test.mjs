import test from 'node:test';
import assert from 'node:assert/strict';
import { measureString, stringRects, drawString } from '../font.js';

// Small fixture: 4x6 cells, capitals only, no punctuation.
const cell = (fill) => Array.from({ length: 6 }, (_, y) => (y === 0 ? `.${fill}${fill}.` : '....'));
const set = {
  palette: { a: '#ffffff', b: '#ff0000' },
  batches: {
    one: { textures: { font_a: cell('a'), font_b: cell('a'), font_unknown: cell('b'), digit_7: cell('a') } },
  },
};

test('measureString is length times cell width, linear in scale', () => {
  for (const s of [1, 2, 3]) {
    assert.deepEqual(measureString(set, 'AB7', s), { w: 12 * s, h: 6 * s });
    assert.deepEqual(measureString(set, 'A B', s), { w: 12 * s, h: 6 * s });
    assert.deepEqual(measureString(set, 'A!', s), { w: 8 * s, h: 6 * s });
    assert.deepEqual(measureString(set, '', s), { w: 0, h: 6 * s });
  }
});

test('stringRects names glyphs, spaces draw nothing, missing characters are unknown', () => {
  const r = stringRects(set, 'a 7!z', 2, 10, 5);
  assert.deepEqual(r.map((q) => q.glyph), ['font_a', null, 'digit_7', 'font_unknown', 'font_unknown']);
  assert.deepEqual(r.map((q) => q.x), [10, 18, 26, 34, 42]);
  assert.ok(r.every((q) => q.y === 5 && q.w === 8 && q.h === 12));
  assert.equal(r[0].ch, 'a');
});

test('lower case uses the capital glyph', () => {
  assert.deepEqual(stringRects(set, 'ab', 1).map((q) => q.glyph), stringRects(set, 'AB', 1).map((q) => q.glyph));
  assert.equal(stringRects(set, 'b', 1)[0].glyph, 'font_b');
});

test('drawString sizes the canvas and paints unknown glyphs in their own colour', () => {
  const fills = [];
  const ctx = { fillRect: (...a) => fills.push([ctx.fillStyle, ...a]) };
  const doc = { createElement: () => ({ style: {}, getContext: () => ctx }) };
  const c = drawString(set, 'A?', 3, doc);
  assert.equal(c.width, 24);
  assert.equal(c.height, 18);
  assert.equal(c.style.imageRendering, 'pixelated');
  assert.deepEqual(fills, [['#ffffff', 3, 0, 3, 3], ['#ffffff', 6, 0, 3, 3], ['#ff0000', 15, 0, 3, 3], ['#ff0000', 18, 0, 3, 3]]);
});
