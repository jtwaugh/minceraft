import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { HELD_TEXTURES, heldTextureName } from '../helditem.js';
import { packAtlas } from '../models.js';
import { ITEM_TEXTURES } from '../itemmodel.js';
import { SCENE_TEXTURES } from '../scenetextures.js';

const itemsToml = readFileSync(new URL('../../data/items.toml', import.meta.url), 'utf8');
const toolNames = [...itemsToml.matchAll(/\[\[tool\]\]\s*\nname = "([^"]+)"/g)].map((m) => m[1]);
const itemNames = [...itemsToml.matchAll(/\[\[item\]\]\s*\nname = "([^"]+)"/g)].map((m) => m[1]);

test('HELD_TEXTURES lists every tool in data/items.toml', () => {
  assert.ok(toolNames.length > 0);
  assert.deepEqual([...HELD_TEXTURES].sort(), [...toolNames].sort());
});

test('heldTextureName maps tools and items to icons', () => {
  for (const n of toolNames) assert.equal(heldTextureName(`tool:${n}`), n);
  assert.equal(heldTextureName('tool:sword'), 'sword');
  assert.equal(heldTextureName('tool:hoe'), 'hoe');
  for (const n of itemNames) assert.equal(heldTextureName(`item:${n}`), n);
});

test('heldTextureName is null for empty and unknown names', () => {
  for (const h of ['empty', '', 'tool:nope', 'item:nope', 'item:sword', 'tool:seeds', 'sword', 'block:stone']) {
    assert.equal(heldTextureName(h), null, h);
  }
  assert.equal(heldTextureName(undefined), null);
});

test('every held icon exists in the tileset export', (t) => {
  const path = new URL('../../viewer/textures.json', import.meta.url);
  if (!existsSync(path)) return t.skip('viewer/textures.json missing; run ./build.sh');
  const tiles = JSON.stringify(JSON.parse(readFileSync(path, 'utf8')));
  for (const n of [...HELD_TEXTURES, ...ITEM_TEXTURES]) assert.ok(tiles.includes(`"${n}"`), `${n} missing from the export`);
});

test('the scene texture list packs into the atlas and includes the held icons', (t) => {
  const path = new URL('../../viewer/textures.json', import.meta.url);
  if (!existsSync(path)) return t.skip('viewer/textures.json missing; run ./build.sh');
  const atlas = packAtlas(JSON.parse(readFileSync(path, 'utf8')), SCENE_TEXTURES);
  for (const n of [...HELD_TEXTURES, ...ITEM_TEXTURES, ...SCENE_TEXTURES]) assert.ok(atlas.rects[n], `${n} missing from the atlas`);
});

test('heldDraw is null for empty hands and unknown things', async () => {
  const { heldDraw } = await import('../helditem.js');
  assert.equal(heldDraw('empty'), null);
  assert.equal(heldDraw('tool:nope'), null);
});

test('heldDraw places tools and items right of and below the view centre', async () => {
  const { heldDraw, heldBoxes } = await import('../helditem.js');
  for (const h of ['tool:sword', 'tool:hoe', 'item:seeds']) {
    const d = heldDraw(h);
    assert.equal(d.key, h.split(':')[1]);
    assert.ok(d.model[12] > 0 && d.model[13] < 0 && d.model[14] < 0, h);
    const boxes = heldBoxes(h);
    assert.equal(boxes.length, 1);
    assert.equal(boxes[0].faces.front, d.key);
    assert.equal(boxes[0].faces.back, d.key);
  }
  assert.deepEqual(heldBoxes('empty'), []);
});

test('held pose tunables match data/held.toml', async () => {
  const h = await import('../helditem.js');
  const toml = readFileSync(new URL('../../data/held.toml', import.meta.url), 'utf8');
  const num = (k) => +toml.match(new RegExp(`^${k} = (\\S+)`, 'm'))[1];
  assert.deepEqual(h.HELD_OFFSET, [num('offset_x'), num('offset_y'), num('offset_z')]);
  assert.equal(h.HELD_SCALE, num('scale'));
  assert.equal(h.HELD_TILT_YAW, num('tilt_yaw'));
  assert.equal(h.HELD_TILT_ROLL, num('tilt_roll'));
});

test('hand pose tunables match data/hands.toml', async () => {
  const h = await import('../helditem.js');
  const toml = readFileSync(new URL('../../data/hands.toml', import.meta.url), 'utf8');
  const num = (k) => +toml.match(new RegExp(`^${k} = (\\S+)`, 'm'))[1];
  assert.deepEqual(h.HELD_SWING_OFFSET, [num('swing_offset_x'), num('swing_offset_y'), num('swing_offset_z')]);
  assert.equal(h.HELD_SWING_PITCH, num('swing_pitch'));
  assert.deepEqual(h.HELD_USE_OFFSET, [num('use_offset_x'), num('use_offset_y'), num('use_offset_z')]);
  assert.equal(h.HELD_USE_PITCH, num('use_pitch'));
});

test('heldDraw at phase 0 is exactly the resting matrix', async () => {
  const { heldDraw } = await import('../helditem.js');
  const rest = heldDraw('tool:sword');
  assert.deepEqual([...heldDraw('tool:sword', 0, 0).model], [...rest.model]);
});

test('heldDraw follows the curve between 0 and 1', async () => {
  const h = await import('../helditem.js');
  const rest = h.heldDraw('tool:sword').model;
  for (const t of [0.25, 0.5, 0.75, 1]) {
    const m = h.heldDraw('tool:sword', t, 0).model;
    for (let i = 0; i < 3; i++) assert.ok(Math.abs(m[12 + i] - (rest[12 + i] + h.HELD_SWING_OFFSET[i] * t)) < 1e-6, `swing ${t} offset ${i}`);
    // pitch: the x column's y/z components rotate by t * pitch
    const a = (h.HELD_SWING_PITCH * t * Math.PI) / 180;
    assert.ok(Math.abs(m[1] - (Math.cos(a) * rest[1] - Math.sin(a) * rest[2])) < 1e-6, `swing ${t} pitch`);
    assert.ok(Math.abs(m[2] - (Math.sin(a) * rest[1] + Math.cos(a) * rest[2])) < 1e-6, `swing ${t} pitch z`);
    const u = h.heldDraw('tool:sword', 0, t).model;
    for (let i = 0; i < 3; i++) assert.ok(Math.abs(u[12 + i] - (rest[12 + i] + h.HELD_USE_OFFSET[i] * t)) < 1e-6, `use ${t} offset ${i}`);
  }
});

test('swing moves the held item further than use', async () => {
  const { heldDraw } = await import('../helditem.js');
  const rest = heldDraw('item:seeds').model;
  const dist = (m) => Math.hypot(...[...m].map((v, i) => v - rest[i]));
  for (const t of [0.25, 0.5, 1]) assert.ok(dist(heldDraw('item:seeds', t, 0).model) > dist(heldDraw('item:seeds', 0, t).model), String(t));
});
