import test from 'node:test';
import assert from 'node:assert/strict';
import { panelRects, drawPanel, PANEL_TILE } from '../panel.js';

const SIZES = [[0, 0], [1, 1], [3, 5], [4, 4], [7, 9], [8, 8], [9, 8], [10, 23], [30, 13]];

test('rects tile the target exactly, with no gaps or overlaps', () => {
  for (const scale of [1, 3, 11]) {
    for (const [w, h] of SIZES) {
      const grid = new Uint8Array(w * scale * h * scale);
      for (const r of panelRects(w, h, scale)) {
        assert.ok(r.w > 0 && r.h > 0 && r.w <= PANEL_TILE * scale && r.h <= PANEL_TILE * scale);
        for (let y = r.y; y < r.y + r.h; y++) for (let x = r.x; x < r.x + r.w; x++) grid[y * w * scale + x]++;
      }
      assert.ok(grid.every((n) => n === 1), `${w}x${h} at ${scale}`);
    }
  }
});

test('corners fixed, edges repeat along one axis, centre along both', () => {
  const r = panelRects(14, 10, 1);
  const count = (n) => r.filter((q) => q.name === n).length;
  assert.deepEqual(['tl', 'tr', 'bl', 'br'].map((n) => count(`panel_${n}`)), [1, 1, 1, 1]);
  assert.equal(count('panel_t'), 2);
  assert.equal(count('panel_b'), 2);
  assert.equal(count('panel_l'), 1);
  assert.equal(count('panel_c'), 2);
  const tl = r.find((q) => q.name === 'panel_tl');
  assert.deepEqual([tl.x, tl.y, tl.w, tl.h], [0, 0, 4, 4]);
  const br = r.find((q) => q.name === 'panel_br');
  assert.deepEqual([br.x, br.y], [10, 6]);
});

test('small sizes split the corners and crop the trailing one', () => {
  const r = panelRects(3, 3, 1);
  assert.deepEqual(r.map((q) => q.name), ['panel_tl', 'panel_tr', 'panel_bl', 'panel_br']);
  const br = r[3];
  assert.deepEqual([br.x, br.y, br.w, br.h, br.sx, br.sy], [1, 1, 2, 2, 2, 2]);
});

test('offset and scale position the rects', () => {
  const r = panelRects(8, 8, 3, 5, 7);
  assert.deepEqual([r[0].x, r[0].y, r[0].w], [5, 7, 12]);
  assert.deepEqual([r[3].x, r[3].y], [17, 19]);
});

test('drawPanel sizes the canvas and paints cropped tiles from the right offset', () => {
  const tile = ['abbb', 'bbbb', 'bbbb', 'bbbb'];
  const set = {
    palette: { a: '#ff0000', b: '#00ff00' },
    batches: { x: { textures: Object.fromEntries(
      ['tl', 't', 'tr', 'l', 'c', 'r', 'bl', 'b', 'br'].map((n) => [`panel_${n}`, tile])) } },
  };
  const fills = [];
  const ctx = { fillRect: (...a) => fills.push([ctx.fillStyle, ...a]) };
  const doc = { createElement: () => ({ style: {}, getContext: () => ctx }) };
  const c = drawPanel(set, 8, 8, 2, doc);
  assert.equal(c.width, 16);
  assert.equal(c.height, 16);
  assert.equal(fills.length, 64);
  assert.deepEqual(fills.filter((f) => f[0] === '#ff0000').map((f) => f.slice(1, 3)), [[0, 0], [8, 0], [0, 8], [8, 8]]);
});

const tile = (c) => Array(4).fill(c.repeat(4));
const setOf = (names) => ({ palette: { a: '#fff' }, batches: { b: { textures: Object.fromEntries(names.map((n) => [n, tile('a')])) } } });
const SUFFIXES = ['tl', 't', 'tr', 'l', 'c', 'r', 'bl', 'b', 'br'];

test('panelNames picks pressed tiles only when the whole set exists', async () => {
  const { panelNames } = await import('../panel.js');
  const raised = SUFFIXES.map((s) => `panel_${s}`), pressed = SUFFIXES.map((s) => `panel_pressed_${s}`);
  const both = setOf([...raised, ...pressed]);
  assert.deepEqual(panelNames(both).flat(), raised);
  assert.deepEqual(panelNames(both, 'raised').flat(), raised);
  assert.deepEqual(panelNames(both, 'pressed').flat(), pressed);
  assert.deepEqual(panelNames(setOf(raised), 'pressed').flat(), raised);
  assert.deepEqual(panelNames(setOf([...raised, ...pressed.slice(1)]), 'pressed').flat(), raised);
});

test('panelImage draws nine tiles and falls back without pressed ones', async () => {
  const { panelImage } = await import('../panel.js');
  globalThis.ImageData = class { constructor(d, w, h) { this.data = d; this.width = w; this.height = h; } };
  const drawn = [];
  const doc = { createElement: () => ({ style: {}, getContext: () => ({ putImageData() {}, drawImage: (_t, x, y) => drawn.push([x, y]) }), toDataURL: () => 'url' }) };
  assert.equal(panelImage(doc, setOf(SUFFIXES.map((s) => `panel_${s}`)), 'pressed'), 'url');
  assert.equal(drawn.length, 9);
  delete globalThis.ImageData;
});
