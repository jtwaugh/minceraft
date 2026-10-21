import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { raftBoxes, RAFT_FACES, RAFT_PLANKS } from '../raftmodel.js';
import { packAtlas } from '../models.js';

const SIZE = 4;
const cell = (x, y, z) => (z * SIZE + x) * 128 + y;
// Two plank cells side by side along x and a crate (id 28) on top of one.
const blocks = new Uint8Array(SIZE * SIZE * 128);
blocks[cell(1, 3, 2)] = RAFT_PLANKS;
blocks[cell(2, 3, 2)] = RAFT_PLANKS;
blocks[cell(2, 4, 2)] = 28;

test('raft boxes lie inside raft cells', () => {
  const boxes = raftBoxes(blocks, SIZE);
  assert.equal(boxes.length, 2 * (1 + 4) + 1);
  for (const b of boxes) {
    const [x, y, z] = b.lo.map(Math.floor);
    assert.notEqual(blocks[cell(x, y, z)], 0, `box at ${b.lo} is not in a raft cell`);
    b.lo.forEach((v, a) => {
      assert.ok(v >= [x, y, z][a] - 1e-9 && v + b.size[a] <= [x, y, z][a] + 1 + 1e-9, `box at ${b.lo} leaves its cell`);
    });
  }
});

test('outer sides get edge faces, shared sides plank faces', () => {
  const [deck] = raftBoxes(blocks, SIZE);
  assert.equal(deck.faces.left, 'raft_edge');
  assert.equal(deck.faces.right, 'raft_planks');
  assert.equal(deck.faces.bottom, 'raft_bottom');
});

test('an empty raft has no boxes', () => {
  assert.equal(raftBoxes(new Uint8Array(SIZE * SIZE * 128), SIZE).length, 0);
});

test('every requested face exists in the art export', (t) => {
  const path = new URL('../../viewer/textures.json', import.meta.url);
  if (!existsSync(path)) return t.skip('viewer/textures.json missing; run ./build.sh');
  const atlas = packAtlas(JSON.parse(readFileSync(path, 'utf8')), RAFT_FACES);
  const used = new Set(raftBoxes(blocks, SIZE).flatMap((b) => Object.values(b.faces)));
  for (const n of [...used, ...RAFT_FACES]) assert.ok(atlas.rects[n], `${n} missing from the export`);
});
