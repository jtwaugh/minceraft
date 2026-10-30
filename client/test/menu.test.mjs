import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hudScale } from '../hud.js';
import { menuModel, menuScale, menuDebug, PANEL, SEA_TILE, WORDMARK, wordmarkTexels } from '../menu.js';
import { SCENES } from '../scenes.js';

test('menu links are relative and cover every scene', () => {
  const [world, play] = menuModel(true, false);
  assert.equal(world.href, 'viewer/');
  assert.equal(play.href, 'client/');
  assert.equal(play.alpha, true);
  assert.equal(play.scenes.length, SCENES.length);
  assert.deepEqual(play.scenes.map((s) => s.href), ['client/', 'client/?scene=raft']);
  for (const e of [world, play, ...play.scenes]) assert.doesNotMatch(e.href, /^(\/|[a-z]+:)/);
});

test('alpha off hides Test play unless debug', () => {
  assert.equal(menuModel(false, false).length, 1);
  const m = menuModel(false, true);
  assert.equal(m[1].alpha, false);
});

test('menuDebug reads ?debug=1', () => {
  assert.equal(menuDebug('?debug=1'), true);
  assert.equal(menuDebug(''), false);
});

test('model carries a label without the alpha suffix', () => {
  for (const e of menuModel(true, false)) assert.doesNotMatch(e.label, /\(alpha\)/);
  assert.equal(menuModel(true, false)[1].alpha, true);
});

test('panel and sea tiles exist in the tileset with the expected sizes', async () => {
  const { readFile } = await import('node:fs/promises');
  const { textureSize } = await import('../uipx.js');
  const set = JSON.parse(await readFile(new URL('../../viewer/textures.json', import.meta.url)));
  for (const name of PANEL.flat()) assert.deepEqual(textureSize(set, name), { w: 4, h: 4 }, name);
  assert.deepEqual(textureSize(set, SEA_TILE), { w: 8, h: 8 });
});

test('menuScale follows hudScale and never overflows the 160x120 layout', () => {
  for (const [w, h] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) {
    const s = menuScale(w, h);
    assert.ok(s <= hudScale(w, h));
    assert.ok(160 * s <= w && 120 * s <= h, `${w}x${h} -> ${s}`);
  }
  assert.equal(menuScale(3440, 1440), 9);
});

test('wordmark is used when the tileset has it, text h1 otherwise', async () => {
  const { readFile } = await import('node:fs/promises');
  const set = JSON.parse(await readFile(new URL('../../viewer/textures.json', import.meta.url)));
  const size = wordmarkTexels(set);
  assert.ok(size && size.w > 0 && size.h > 0);
  for (const [w, h] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) {
    const s = menuScale(w, h);
    assert.ok(size.w * s <= w, `${w}x${h}: wordmark ${size.w * s} wide`);
  }
  assert.equal(wordmarkTexels({ batches: {} }), null);
  assert.equal(WORDMARK, 'wordmark');
});

test('styleMenu sets the button variables and generates raised and pressed CSS', async () => {
  const { readFile } = await import('node:fs/promises');
  const { styleMenu, BUTTON_SELECTOR } = await import('../menu.js');
  const set = JSON.parse(await readFile(new URL('../../viewer/textures.json', import.meta.url)));
  const vars = {}, sheets = {};
  globalThis.ImageData ??= class { constructor(data, w, h) { this.data = data; this.width = w; this.height = h; } };
  const canvas = () => ({
    width: 0, height: 0, style: {},
    getContext: () => ({ drawImage() {}, fillRect() {}, putImageData() {}, createImageData: (w, h) => ({ data: new Uint8ClampedArray(w * h * 4) }) }),
    toDataURL: () => 'data:x',
  });
  const doc = {
    documentElement: { style: { setProperty: (k, v) => { vars[k] = v; } } },
    body: { classList: { add() {} } },
    head: { appendChild: (el) => { sheets[el.id] = el; } },
    getElementById: (id) => sheets[id] ?? null,
    querySelector: () => null,
    createElement: (tag) => (tag === 'canvas' ? canvas() : { id: '', textContent: '' }),
  };
  assert.equal(styleMenu(doc, { innerWidth: 1366, innerHeight: 768 }, set), true);
  assert.match(vars['--btn-raised'], /^url\(/);
  assert.match(vars['--btn-pressed'], /^url\(/);
  const css = sheets['menu-buttons'].textContent;
  assert.ok(css.includes(`${BUTTON_SELECTOR}:active`));
  assert.match(css, /border-image-source/);
  styleMenu(doc, { innerWidth: 1920, innerHeight: 1080 }, set);
  assert.equal(Object.keys(sheets).length, 1);
});
