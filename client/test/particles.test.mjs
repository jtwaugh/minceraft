import test from 'node:test';
import assert from 'node:assert/strict';
import { B, H } from '../../viewer/mesh.js';
import { createParticles, snowOnFoliage, particleTexture, PARTICLE_TEXTURES } from '../particles.js';
import { SCENE_TEXTURES } from '../scenetextures.js';
import { readFileSync } from 'node:fs';

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
const WIND = { dir: [0.857493, 0.514496], strength: 1 };
function run(p, cells, eye, seconds, wind = WIND) {
  let spawned = 0;
  for (let i = 0; i < seconds * 60; i++) {
    const before = p.count;
    p.spawnSnow(cells, eye, { ...wind, phase: (i / 60) * 1.6 % (2 * Math.PI) }, 1 / 60);
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
    p.spawnSnow(cells, [10, 70, 0], { ...WIND, phase: (i / 60) * 1.6 }, 1 / 60);
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

test('calm wind spawns far fewer flakes than a strong one, and none at strength 0', () => {
  const cells = Array.from({ length: 40 }, (_, i) => [10 + (i % 8), 70, -4 - (i >> 3)]);
  const strong = run(createParticles(), cells, [10, 70, 0], 10);
  const calm = run(createParticles(), cells, [10, 70, 0], 10, { ...WIND, strength: 0.1 });
  assert.ok(strong > 20, `strong ${strong}`);
  assert.ok(calm * 4 < strong, `calm ${calm} strong ${strong}`);
  assert.equal(run(createParticles(), cells, [10, 70, 0], 10, { dir: [0, 0], strength: 0 }), 0);
});

test('flakes leave in the wind direction', () => {
  const cells = Array.from({ length: 40 }, () => [10, 70, -4]);
  for (const dir of [[1, 0], [0, -1], [-0.6, 0.8]]) {
    const p = createParticles();
    for (let i = 0; i < 240; i++) p.spawnSnow(cells, [10, 70, 0], { dir, strength: 1, phase: (i / 60) * 1.6 }, 1 / 60);
    assert.ok(p.count > 0);
    for (const f of p.list) {
      const out = (f.pos[0] - 10.5) * dir[0] + (f.pos[2] + 3.5) * dir[1];
      assert.ok(out > 0.3, 'spawned on the downwind side');
      assert.ok(f.vel[0] * dir[0] + f.vel[2] * dir[1] > 0, 'drifts downwind');
    }
  }
});

test('a fish_bite with a bobber out splashes at the bobber; no bobber or another event adds none', () => {
  const p = createParticles(), bobber = [4.5, 63, -2.5];
  p.spawnSplash(['fish_bite player=1 species=salmon'], null);
  p.spawnSplash(['line_cast player=1 position=1,2,3', 'fish_caught player=1'], bobber);
  assert.equal(p.count, 0);
  p.spawnSplash(['fish_bite player=1 species=salmon'], bobber);
  assert.ok(p.count > 0);
  for (const s of p.list) {
    assert.equal(s.sprite, 'splash');
    assert.ok(Math.hypot(s.pos[0] - bobber[0], s.pos[2] - bobber[2]) < 0.2);
  }
});

test('splash particles run frames 0 to 3 and expire after their life', () => {
  const p = createParticles();
  p.spawnSplash(['fish_bite'], [0, 63, 0]);
  const seen = new Set();
  for (let i = 0; i < 20 && p.count; i++) {
    seen.add(p.list[0].frame);
    p.update(1 / 30);
  }
  assert.deepEqual([...seen], [0, 1, 2, 3]);
  assert.equal(p.count, 0);
});

test('a lured fish leaves fishing_wake particles at its position over frames; an unlured one leaves none', () => {
  const p = createParticles();
  const fish = [{ id: 1, pos: [3, 62.8, 5], lured: true }, { id: 2, pos: [-8, 62.8, 1], lured: false }];
  let spawned = 0;
  for (let i = 0; i < 30; i++) {
    fish[0].pos = [3 + i * 0.05, 62.8, 5];
    const before = p.count;
    p.spawnWake(fish, 1 / 60);
    spawned += p.count - before;
    p.update(1 / 60);
  }
  assert.ok(spawned >= 4, `spawned ${spawned}`);
  const xs = new Set();
  for (const w of p.list) {
    assert.equal(w.sprite, 'fishing_wake');
    assert.deepEqual(w.vel, [0, 0, 0]);
    assert.ok(Math.abs(w.pos[0] - 3.75) < 1 && w.pos[2] === 5, 'trails the lured fish, not the other');
    xs.add(w.pos[0]);
  }
  assert.ok(xs.size > 1, 'left along its path');
  const q = createParticles();
  for (let i = 0; i < 60; i++) q.spawnWake([fish[1]], 1 / 60);
  assert.equal(q.count, 0);
});

test('wake particles expire and the live count stays bounded', () => {
  const p = createParticles(), fish = [{ id: 1, pos: [0, 63, 0], lured: true }, { id: 4, pos: [2, 63, 0], lured: true }];
  let most = 0;
  for (let i = 0; i < 600; i++) {
    p.spawnWake(fish, 1 / 60);
    p.update(1 / 60);
    most = Math.max(most, p.count);
  }
  assert.ok(most > 0 && most <= 20, `most ${most}`);
  const calm = fish.map((f) => Object.assign({}, f, { lured: false }));
  for (let i = 0; i < 60; i++) {
    p.spawnWake(calm, 1 / 60);
    p.update(1 / 60);
  }
  assert.equal(p.count, 0, 'all expire once no fish is lured');
});

test('splash particles draw splash_<frame>, wake fishing_wake, snow and misses as boxes', () => {
  const p = createParticles();
  p.spawnSplash(['fish_bite'], [0, 63, 0]);
  p.spawnWake([{ id: 1, pos: [0, 63, 0], lured: true }], 1);
  p.spawnMisses(['forage_missed position=1,64,2']);
  p.spawnSnow([[0, 64, 0]], [0, 64, 0], { dir: [1, 0], strength: 1, phase: Math.PI / 2 }, 1e6);
  const seen = new Set();
  for (let i = 0; i < 5; i++) {
    for (const q of p.list) {
      const tex = particleTexture(q);
      if (q.sprite === 'splash') assert.equal(tex, `splash_${q.frame}`);
      else if (q.sprite === 'fishing_wake') assert.equal(tex, 'fishing_wake');
      else assert.equal(tex, null, 'snow and forage-miss particles keep their boxes');
      seen.add(tex);
    }
    p.update(0.12);
  }
  assert.deepEqual([...seen].sort(), [null, 'fishing_wake', 'splash_0', 'splash_1', 'splash_2', 'splash_3'].sort());
  assert.equal(particleTexture({ sprite: 'nope' }), null);
});

test('particle textures are packed into the scene atlas and exist in the art', () => {
  const art = readFileSync(new URL('../../art/batches_7_12.json', import.meta.url), 'utf8');
  for (const name of PARTICLE_TEXTURES) {
    assert.ok(SCENE_TEXTURES.includes(name), name);
    assert.ok(art.includes(`"${name}"`), name);
  }
});
