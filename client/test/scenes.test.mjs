import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ALPHA, SCENES, sceneById, menuEntries } from '../scenes.js';

const testPlay = (entries) => entries.find((e) => e.key === 'testplay');

test('scene list has the single chunk (no id) and raft', () => {
  assert.equal(SCENES[0].id, null);
  assert.ok(SCENES.some((s) => s.id === 'raft'));
  assert.equal(typeof ALPHA, 'boolean');
});

test('alpha on: Test play is listed with an alpha label', () => {
  const tp = testPlay(menuEntries(true, false));
  assert.ok(tp);
  assert.match(tp.name, /alpha/);
  assert.deepEqual(tp.scenes.map((s) => s.href), ['client/', 'client/?scene=raft']);
});

test('alpha off, no debug: Test play is absent, World gen stays', () => {
  const entries = menuEntries(false, false);
  assert.equal(testPlay(entries), undefined);
  assert.ok(entries.some((e) => e.key === 'worldgen'));
});

test('alpha off, debug on: Test play is present', () => {
  const tp = testPlay(menuEntries(false, true));
  assert.ok(tp);
  assert.doesNotMatch(tp.name, /alpha/);
});

test('unknown scene id resolves to no scene and entries do not throw', () => {
  assert.equal(sceneById('nope'), undefined);
  assert.equal(sceneById(null), undefined);
  assert.equal(sceneById('raft').id, 'raft');
  assert.doesNotThrow(() => menuEntries(true, false));
});
