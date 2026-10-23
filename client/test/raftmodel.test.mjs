import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { raftBoxes, raftMatrix, RAFT_FACES, RAFT_PLANKS } from '../raftmodel.js';
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

const apply = (m, [x, y, z]) => [0, 1, 2].map((r) => m[r] * x + m[4 + r] * y + m[8 + r] * z + m[12 + r]);
const near = (a, b) => a.forEach((v, i) => assert.ok(Math.abs(v - b[i]) < 1e-5, `${a} vs ${b}`));

test('heading 0 is the translation-only matrix', () => {
  const m = raftMatrix([1.5, -0.25, 2], 0, [10, 20], 8);
  assert.deepEqual([...m], [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 9.5, -0.25, 10, 1]);
});

test('a quarter turn carries -z to +x about a fixed pivot', () => {
  const origin = 8;
  const pivot = [12, 15]; // world; region-local (4, 7)
  const m = raftMatrix([0, 0, 0], Math.PI / 2, pivot, origin);
  near(apply(m, [4, 3, 7]), [pivot[0], 3, pivot[1]]);
  near(apply(m, [4, 3, 6]), [pivot[0] + 1, 3, pivot[1]]);
});

test('the offset moves the turned raft', () => {
  const m = raftMatrix([1, 2, 3], Math.PI / 2, [12, 15], 8);
  near(apply(m, [4, 0, 7]), [13, 2, 18]);
});
