// Run: node --test client/test/deathui.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { hudScale } from '../hud.js';
import { deathModel, deathScale, mountDeath } from '../deathui.js';

// Just enough DOM for mountDeath: elements with children, listeners and a canvas stub.
function fakeDocument() {
  const make = (tag) => ({
    tag, style: {}, children: [], listeners: {}, hidden: false, id: '', textContent: '',
    attrs: {},
    setAttribute(k, v) { this.attrs[k] = v; },
    replaceChildren(...kids) { this.children = kids; },
    append(...kids) { this.children.push(...kids); },
    addEventListener(type, fn) { (this.listeners[type] ??= []).push(fn); },
    click() { (this.listeners.click ?? []).forEach((fn) => fn()); },
    getContext: () => ({ drawImage() {}, fillRect() {} }),
    toDataURL: () => 'data:,',
  });
  const body = make('body');
  return { body, createElement: make };
}

const cell = Array.from({ length: 6 }, () => '.aa.');
const FONT_SET = {
  palette: { a: '#ffffff' },
  batches: { one: { textures: { font_unknown: cell, font_y: cell, font_r: cell } } },
};

function mount() {
  const document = fakeDocument();
  let respawns = 0;
  const view = mountDeath(document, FONT_SET, { respawn: () => respawns++ }, document.body, { innerWidth: 800, innerHeight: 600 });
  const root = document.body.children[2];
  return { document, root, view, count: () => respawns, ids: () => root.children[0].children.map((e) => e.id) };
}

test('deathScale is an integer >= 1', () => {
  for (const [w, h] of [[0, 0], [NaN, 5], [100, 60], [1920, 1080]]) {
    const s = deathScale(w, h);
    assert.ok(Number.isInteger(s) && s >= 1);
  }
});

test('the model is exactly a title and a respawn button', () => {
  assert.deepEqual(deathModel().map((m) => [m.id, m.kind]), [['death-title', 'title'], ['death-respawn', 'button']]);
  assert.equal(deathModel()[0].text, 'You died');
});

test('hidden while alive, shown while dead', () => {
  const { root, view } = mount();
  assert.equal(root.hidden, true);
  view.update(true);
  assert.equal(root.hidden, false);
  view.update(false);
  assert.equal(root.hidden, true);
});

test('the DOM holds only the model elements by id', () => {
  const { ids } = mount();
  assert.deepEqual(ids(), deathModel().map((m) => m.id));
});

test('clicking respawn calls controller.respawn once', () => {
  const { root, view, count } = mount();
  view.update(true);
  root.children[0].children[1].click();
  assert.equal(count(), 1);
});

test('deathScale agrees with hudScale at the standard sizes', () => {
  for (const [w, h] of [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]]) assert.equal(deathScale(w, h), hudScale(w, h));
});

test('the button gets raised and pressed CSS, and the text is drawn from the font', () => {
  const { document, root, view } = mount();
  view.update(true);
  const css = document.body.children[1].textContent;
  assert.match(css, /#death-respawn \{[^}]*border-image-source/);
  assert.match(css, /#death-respawn:active \{ border-image-source/);
  const [title, button] = root.children[0].children;
  assert.equal(title.children[0].tag, 'canvas');
  assert.equal(button.children[0].tag, 'canvas');
  assert.equal(button.attrs['aria-label'], 'Respawn');
});

test('no monospace font is left in the death screen source', async () => {
  const { readFileSync } = await import('node:fs');
  assert.ok(!readFileSync(new URL('../deathui.js', import.meta.url), 'utf8').includes('ui-monospace'));
});
