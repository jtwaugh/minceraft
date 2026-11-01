import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hudScale } from '../hud.js';
import { menuModel, menuScale, menuDebug, renderMenu, pixelLabels, PANEL, SEA_TILE, WORDMARK, wordmarkTexels } from '../menu.js';
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
    querySelectorAll: () => [],
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

// A small DOM: enough elements for renderMenu (tags, children, attributes,
// listeners and selectors by tag, class or attribute).
function fakeDoc() {
  const matches = (el, sel) => sel.split(',').some((one) => {
    const s = one.trim();
    if (s === '[data-label]') return 'label' in el.dataset;
    if (s === 'a[data-href]') return el.tag === 'a' && 'href' in el.dataset;
    if (s.startsWith('.')) return el.className === s.slice(1);
    return el.tag === s;
  });
  const all = (el) => el.children.flatMap((c) => [c, ...all(c)]);
  const make = (tag) => {
    const el = {
      tag, children: [], dataset: {}, attrs: {}, listeners: {}, className: '', hidden: false, textContent: '', style: {},
      appendChild(c) { el.children.push(c); return c; },
      append(...cs) { el.children.push(...cs); },
      replaceChildren(...cs) { el.children = cs; },
      setAttribute(k, v) { el.attrs[k] = v; },
      addEventListener(k, f) { el.listeners[k] = f; },
      querySelectorAll: (sel) => all(el).filter((c) => matches(c, sel)),
      querySelector: (sel) => el.querySelectorAll(sel)[0] ?? null,
      focus() {},
    };
    return el;
  };
  return { createElement: make, root: make('main') };
}

test('Test play opens a picker with the seed box and the two scenes', () => {
  const doc = fakeDoc();
  renderMenu(doc, doc.root, menuModel(true, false));
  const [world, play] = doc.root.children;
  assert.equal(world.querySelector('a').dataset.label, 'World gen');
  const toggle = play.querySelector('button');
  const picker = play.querySelector('.picker');
  assert.equal(picker.hidden, true);
  toggle.listeners.click();
  assert.equal(picker.hidden, false);
  assert.equal(toggle.attrs['aria-expanded'], 'true');
  assert.ok(picker.querySelector('input'));
  assert.deepEqual(picker.querySelectorAll('li').map((li) => li.children[0].dataset.label), ['Open world', 'Raft']);
  toggle.listeners.click();
  assert.equal(picker.hidden, true);
});

test('pixelLabels draws every label in the pixel font and keeps the text as aria-label', async () => {
  const { readFile } = await import('node:fs/promises');
  const set = JSON.parse(await readFile(new URL('../../viewer/textures.json', import.meta.url)));
  const doc = fakeDoc();
  renderMenu(doc, doc.root, menuModel(true, false));
  const create = doc.createElement;
  doc.createElement = (tag) => (tag === 'canvas'
    ? { tag, width: 0, height: 0, style: {}, getContext: () => ({ fillRect() {} }), children: [], dataset: {} }
    : create(tag));
  doc.querySelectorAll = doc.root.querySelectorAll;
  pixelLabels(doc, set, 2);
  const labels = doc.root.querySelectorAll('[data-label]');
  assert.deepEqual(labels.map((el) => el.dataset.label), ['World gen', 'Test play', 'alpha', 'Seed', 'Random', 'Open world', 'Raft']);
  for (const el of labels) {
    assert.equal(el.attrs['aria-label'], el.dataset.label);
    assert.deepEqual(el.children.map((c) => c.tag), ['canvas']);
  }
});
