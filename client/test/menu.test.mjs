import { test } from 'node:test';
import assert from 'node:assert/strict';
import { menuModel, menuDebug } from '../menu.js';
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
