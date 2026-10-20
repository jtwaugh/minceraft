import test from 'node:test';
import assert from 'node:assert/strict';
import { buildMips, buildMips1 } from '../mips.js';

const SIZE = 8;

// Leaf-like layer: one colour, roughly 60% of texels opaque, the rest holes.
function leaf() {
  const px = new Uint8Array(SIZE * SIZE * 4);
  for (let i = 0; i < SIZE * SIZE; i++) {
    const solid = (i * 7 + (i >> 3)) % 5 < 3;
    px.set(solid ? [40, 160, 30, 255] : [0, 0, 0, 0], i * 4);
  }
  return px;
}

function share(level) {
  let n = 0;
  for (let i = 3; i < level.length; i += 4) if (level[i] >= 128) n++;
  return { n, total: level.length / 4 };
}

test('chain runs down to 1x1', () => {
  const mips = buildMips(leaf(), SIZE);
  assert.deepEqual(mips.map((m) => m.length / 4), [64, 16, 4, 1]);
});

test('one colour with holes keeps that colour at every level', () => {
  for (const m of buildMips(leaf(), SIZE).slice(1)) {
    for (let i = 0; i < m.length; i += 4) {
      if (m[i + 3] === 0) continue;
      assert.deepEqual([...m.subarray(i, i + 3)], [40, 160, 30]);
    }
  }
});

test('coverage stays within one texel of level 0', () => {
  const mips = buildMips(leaf(), SIZE);
  const base = share(mips[0]);
  const frac = base.n / base.total;
  for (const m of mips) {
    const { n, total } = share(m);
    assert.ok(Math.abs(n - frac * total) <= 1, `${n}/${total} vs ${frac}`);
  }
});

test('opaque and transparent layers stay so', () => {
  const opaque = new Uint8Array(SIZE * SIZE * 4).fill(255);
  for (const m of buildMips(opaque, SIZE)) assert.ok(m.every((v) => v === 255));
  const clear = new Uint8Array(SIZE * SIZE * 4);
  for (const m of buildMips(clear, SIZE)) assert.ok(m.every((v) => v === 0));
});

test('premultiply scales colour with alpha, colour / alpha unchanged', () => {
  const straight = buildMips(leaf(), SIZE);
  const pre = buildMips(leaf(), SIZE, { premultiply: true });
  pre.forEach((m, l) => {
    for (let i = 0; i < m.length; i += 4) {
      const a = m[i + 3];
      assert.equal(a, straight[l][i + 3]);
      for (let c = 0; c < 3; c++) {
        assert.ok(Math.abs(m[i + c] - (straight[l][i + c] * a) / 255) <= 1);
      }
    }
  });
});

test('single-channel helper averages', () => {
  const data = Uint8Array.from([0, 100, 200, 200, 0, 100, 200, 200, 50, 50, 255, 255, 50, 50, 255, 255]);
  const mips = buildMips1(data, 4);
  assert.deepEqual(mips.map((m) => m.length), [16, 4, 1]);
  assert.deepEqual([...mips[1]], [50, 200, 50, 255]);
  assert.equal(mips[2][0], Math.round((50 + 200 + 50 + 255) / 4));
});

test('rejects non power of two', () => {
  assert.throws(() => buildMips(new Uint8Array(36), 3));
});
