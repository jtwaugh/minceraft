// Run: node --test client/test/deathui.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { hudScale } from '../hud.js';
import { deathModel, deathScale, mountDeath } from '../deathui.js';

// Just enough DOM for mountDeath: elements with children, listeners and a canvas stub.
function fakeDocument() {
  const make = (tag) => ({
    tag, style: {}, children: [], listeners: {}, hidden: false, id: '', textContent: '',
    append(...kids) { this.children.push(...kids); },
    addEventListener(type, fn) { (this.listeners[type] ??= []).push(fn); },
    click() { (this.listeners.click ?? []).forEach((fn) => fn()); },
    getContext: () => ({ drawImage() {} }),
    toDataURL: () => 'data:,',
  });
  const body = make('body');
  return { body, createElement: make };
}

function mount() {
  const document = fakeDocument();
  let respawns = 0;
  const view = mountDeath(document, { palette: {}, batches: {} }, { respawn: () => respawns++ }, document.body, { innerWidth: 800, innerHeight: 600 });
  const root = document.body.children[1];
  return { root, view, count: () => respawns, ids: () => root.children[0].children.map((e) => e.id) };
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
