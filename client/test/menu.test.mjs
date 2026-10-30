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
  assert.equal(menuScale(3440, 1440), 12);
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
