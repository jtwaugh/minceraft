import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { B } from '../../viewer/mesh.js';

const root = new URL('../../', import.meta.url);
const read = (p) => readFileSync(new URL(p, root), 'utf8');

function tomlIds() {
  const out = {};
  for (const m of read('data/blocks.toml').matchAll(/id = (\d+)\nname = "(\w+)"/g)) out[m[2]] = +m[1];
  return out;
}

test('mesh.js B matches blocks.toml', () => {
  assert.deepEqual(B, tomlIds());
});

test('index.html takes B from mesh.js', () => {
  assert.match(read('viewer/index.html'), /import \{ B\b[^}]*\} from '\.\/mesh\.js'/);
  assert.doesNotMatch(read('viewer/index.html'), /const B = \{/);
});
