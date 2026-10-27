import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { ITEM_TEXTURES, itemTextureName } from '../itemmodel.js';
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
