import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H } from '../../viewer/mesh.js';
import { createParticles, snowOnFoliage } from '../particles.js';

// 3 x 3 inner columns with pad 1: stone to y 60, then `under` and snow in each listed inner column.
function region(cols) {
  const size = 3, w = 5, blocks = new Uint8Array(w * w * H);
  for (let c = 0; c < w * w; c++) blocks.fill(B.stone, c * H, c * H + 60);
  for (const [x, z, under] of cols) {
    const base = ((z + 1) * w + x + 1) * H;
    blocks[base + 60] = under; blocks[base + 61] = B.snow_layer;
  }
  return { blocks, size };
}

test('snow on leaves and bushes is found at its world cell; snow on stone and in the pad is not', () => {
  const { blocks, size } = region([[0, 0, B.oak_leaves], [2, 1, B.bush], [1, 2, B.stone], [-1, 0, B.oak_leaves]]);
  assert.deepEqual(snowOnFoliage(blocks, size, 1, [100, -50]), [[100, 61, -50], [102, 61, -49]]);
});

// Frames of 1/60 s over `seconds`, the sway phase advancing as in sway.js.
function run(p, cells, eye, seconds) {
  let spawned = 0;
  for (let i = 0; i < seconds * 60; i++) {
    const before = p.count;
    p.spawnSnow(cells, eye, (i / 60) * 1.6 % (2 * Math.PI), 1 / 60);
    spawned += p.count - before;
    p.update(1 / 60);
  }
  return spawned;
}

test('snowy foliage near the eye drops flakes that fall beside it and expire', () => {
  const p = createParticles(), cell = [10, 70, -4];
  const cells = Array.from({ length: 40 }, () => cell);
  let lowest = Infinity;
  for (let i = 0; i < 240; i++) {
    p.spawnSnow(cells, [10, 70, 0], (i / 60) * 1.6, 1 / 60);
    p.update(1 / 60);
    for (const f of p.list) {
      assert.ok(f.snow);
      assert.ok(Math.abs(f.pos[0] - 10.5) < 1.5 && Math.abs(f.pos[2] + 3.5) < 1.5, 'near its cell');
      lowest = Math.min(lowest, f.pos[1]);
    }
  }
  assert.ok(p.count > 0 && p.count <= 256);
  assert.ok(lowest < 70, 'flakes fall');
  run(p, [], [0, 0, 0], 3);
  assert.equal(p.count, 0, 'all expire');
});

test('snow far from the eye drops nothing', () => {
  const p = createParticles();
  assert.equal(run(p, Array.from({ length: 40 }, () => [100, 70, 100]), [0, 70, 0], 4), 0);
});
