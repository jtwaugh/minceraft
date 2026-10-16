// Checks the wasm clock, fly camera and view matrix bindings as scene.js
// drives them. The sim code itself is tested in crates/game. Needs ./build.sh first.
// Run: node --test client/test/camera.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, InputSampler, SimClock, FlyCamera, Walk, view_matrix } from '../pkg/game_web.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

test('the clock turns frame time into ticks and an interpolation fraction', () => {
  const clock = new SimClock();
  assert.equal(clock.advance(0.075), 1); // 1.5 ticks: one to run, alpha 0.5
  assert.ok(Math.abs(clock.alpha - 0.5) < 1e-5);
  clock.free();
});

test('a fly tick from a sampled forward key moves the eye along -z', () => {
  const sampler = new InputSampler();
  const fly = new FlyCamera(1, 2, 3);
  sampler.key_down('KeyW');
  const t = sampler.drain();
  fly.tick(t);
  t.free();
  const [x, y, z] = fly.eye(1);
  assert.equal(x, 1);
  assert.equal(y, 2);
  assert.ok(z < 3, `eye z ${z}`);
  fly.teleport(5, 6, 7);
  assert.deepEqual(Array.from(fly.eye(0.5)), [5, 6, 7]);
  fly.free();
  sampler.free();
});

test('a walk tick takes a sampled Tick', () => {
  const sampler = new InputSampler();
  const walk = new Walk(0.5, 70, 0.5);
  for (let i = 0; i < 2; i++) {
    const t = sampler.drain();
    walk.tick(t); // no terrain: the player just falls
    t.free();
  }
  assert.ok(walk.feet()[1] < 70);
  walk.free();
  sampler.free();
});

test('view matrix: an eye at z = 5 looking along -z sees the origin 5 ahead', () => {
  const m = view_matrix(0, 0, 5, 0, 0);
  assert.equal(m.length, 16);
  assert.ok(Math.abs(m[14] + 5) < 1e-6);
});
