import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import {
  ITEM_TEXTURES, itemTextureName, itemBoxes, itemPose, itemDraws, BOB_SPEED, BOB_AMPLITUDE,
} from '../itemmodel.js';
import { packAtlas } from '../models.js';

const itemsToml = readFileSync(new URL('../../data/items.toml', import.meta.url), 'utf8');
const itemNames = [...itemsToml.matchAll(/\[\[item\]\]\s*\nname = "([^"]+)"/g)].map((m) => m[1]);

test('ITEM_TEXTURES lists every item in data/items.toml', () => {
  assert.ok(itemNames.length > 0);
  assert.deepEqual([...ITEM_TEXTURES].sort(), [...itemNames].sort());
});

test('itemTextureName maps items to icons and unknown names to null', () => {
  for (const n of itemNames) assert.equal(itemTextureName(n), n);
  assert.equal(itemTextureName('no_such_item'), null);
  assert.equal(itemTextureName(''), null);
});

test('every item icon exists in the tileset export and packs into the atlas', (t) => {
  const path = new URL('../../viewer/textures.json', import.meta.url);
  if (!existsSync(path)) return t.skip('viewer/textures.json missing; run ./build.sh');
  const atlas = packAtlas(JSON.parse(readFileSync(path, 'utf8')), ITEM_TEXTURES);
  for (const n of ITEM_TEXTURES) assert.ok(atlas.rects[n], `${n} missing from the export`);
});

const camera = { yaw: 0.7 };
const item = (id, size = 0.25, pos = [3, 64, -2]) => ({ id, name: 'seeds', pos, size });

test('BOB tunables match data/item_motion.toml', () => {
  const toml = readFileSync(new URL('../../data/item_motion.toml', import.meta.url), 'utf8');
  assert.equal(+toml.match(/^bob_speed = (\S+)/m)[1], BOB_SPEED);
  assert.equal(+toml.match(/^bob_amplitude = (\S+)/m)[1], BOB_AMPLITUDE);
});

test('itemBoxes textures both big faces with the icon, unknown items grey', () => {
  const [b] = itemBoxes('seeds');
  assert.equal(b.faces.front, 'seeds');
  assert.equal(b.faces.back, 'seeds');
  assert.deepEqual(itemBoxes('nope')[0].faces, {});
});

test('itemDraws yields one draw per item at its position', () => {
  const items = [item(1, 0.25, [1, 2, 3]), item(2, 0.25, [4, 5, 6]), item(9, 0.25, [-7, 8, 9])];
  const draws = itemDraws(items, 0, camera);
  assert.equal(draws.length, items.length);
  draws.forEach((d, i) => {
    assert.equal(d.key, 'seeds');
    assert.equal(d.model[12], items[i].pos[0]);
    assert.equal(d.model[14], items[i].pos[2]);
    assert.ok(Math.abs(d.model[13] - items[i].pos[1]) <= BOB_AMPLITUDE * 0.25 + 1e-6);
  });
  assert.deepEqual(itemDraws([], 0, camera), []);
});

test('the bob is deterministic and different ids are out of phase', () => {
  assert.deepEqual(itemPose(item(5), 1.3, camera), itemPose(item(5), 1.3, camera));
  const y = (id, t) => itemPose(item(id), t, camera)[13];
  assert.notEqual(y(1, 0.4), y(2, 0.4));
  assert.notEqual(y(2, 0.4), y(3, 0.4));
  // one full bob later it is back where it started
  assert.ok(Math.abs(y(4, 0.4) - y(4, 0.4 + 1 / BOB_SPEED)) < 1e-4);
});

test('the pose scales to item.size and faces the camera upright', () => {
  for (const size of [0.25, 1, 2]) {
    const m = itemPose(item(1, size), 0, camera);
    assert.ok(Math.abs(Math.hypot(m[0], m[1], m[2]) - size) < 1e-6);
    assert.ok(Math.abs(Math.hypot(m[4], m[5], m[6]) - size) < 1e-6);
    assert.ok(Math.abs(Math.hypot(m[8], m[9], m[10]) - size) < 1e-6);
    assert.equal(m[5], size);
    assert.equal(m[1], 0);
  }
});
