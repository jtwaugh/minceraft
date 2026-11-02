import test from 'node:test';
import assert from 'node:assert/strict';
import { sampleSky, easeSky, SKY_EASE_SECONDS } from '../skyblend.js';

const ramp = (sky) => sky; // a flat "ramp": the biome's sky colour whatever the temperature
const biomes = [{ sky: [100, 150, 200] }, { sky: [200, 50, 100] }];

test('easing converges in about 2 s at any frame rate and never overshoots', () => {
  const target = [0.2, 0.9, 0.5];
  for (const fps of [10, 30, 60, 144]) {
    let c = [0.8, 0.1, 0.5];
    const dt = 1 / fps;
    for (let t = 0; t < SKY_EASE_SECONDS - 1e-9; t += dt) {
      const n = easeSky(c, target, dt);
      assert.ok(n[0] >= target[0] && n[0] <= c[0], 'monotonic toward target');
      assert.ok(n[1] <= target[1] && n[1] >= c[1]);
      c = n;
    }
    assert.ok(Math.abs(c[0] - target[0]) < 0.06 * 0.6, `fps ${fps}: ${c}`);
    assert.ok(Math.abs(c[1] - target[1]) < 0.06 * 0.8);
  }
  assert.deepEqual(easeSky([0.3, 0.3, 0.3], [0.3, 0.3, 0.3], 100), [0.3, 0.3, 0.3]);
  assert.deepEqual(easeSky([0, 0, 0], [1, 1, 1], 1e6), [1, 1, 1]);
});

test('a window straddling a border averages between the two biomes', () => {
  const ids = [0, 0, 0, 1, 1, 1, 1, 1, 1];
  const c = sampleSky(ids, new Array(9).fill(0.5), biomes, ramp);
  for (let k = 0; k < 3; k++) {
    const lo = Math.min(biomes[0].sky[k], biomes[1].sky[k]) / 255;
    const hi = Math.max(biomes[0].sky[k], biomes[1].sky[k]) / 255;
    assert.ok(c[k] >= lo && c[k] <= hi);
  }
  assert.ok(c[0] > 100 / 255 && c[0] < 200 / 255);
  assert.deepEqual(sampleSky([1, 1], [0, 0], biomes, ramp), [200 / 255, 50 / 255, 100 / 255]);
});

test('blendSky is the base by day, night at the minimum, and smooth between', async () => {
  const { blendSky } = await import('../skyblend.js');
  const base = [0.4, 0.6, 0.9], night = [10, 14, 38], dusk = [214, 118, 84], min = 0.15;
  assert.deepEqual(blendSky(base, 1, min, night, dusk), base);
  const end = blendSky(base, min, min, night, dusk);
  night.forEach((v, k) => assert.ok(Math.abs(end[k] - v / 255) < 1e-12));
  const mid = blendSky(base, (1 + min) / 2, min, night, dusk);
  dusk.forEach((v, k) => assert.ok(Math.abs(mid[k] - v / 255) < 1e-12));
  let prev = blendSky(base, 1, min, night, dusk);
  for (let i = 1; i <= 1000; i++) {
    const c = blendSky(base, 1 - (i / 1000) * (1 - min), min, night, dusk);
    c.forEach((v, k) => assert.ok(Math.abs(v - prev[k]) < 0.01, `jump at ${i}`));
    prev = c;
  }
});
