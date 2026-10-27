import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { HELD_TEXTURES, heldTextureName } from '../helditem.js';
import { ITEM_TEXTURES } from '../itemmodel.js';

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
