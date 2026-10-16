// Pure-JS checks for the fly camera and frame statistics.
// Run: node --test client/test/fly.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { stepFly, interpolate, TickClock, TICK_MS, FLY_SPEED, lookVectors } from '../fly.js';
import { FrameTimes } from '../frametimes.js';

const idle = { move_forward: 0, move_strafe: 0, yaw: 0, pitch: 0, jump: false, sneak: false };
const near = (a, b) => a.every((v, i) => Math.abs(v - b[i]) < 1e-9);

test('forward at yaw 0 moves +z by one tick of speed', () => {
  const p = stepFly([0, 0, 0], { ...idle, move_forward: 1 });
  assert.ok(near(p, [0, 0, FLY_SPEED / 20]));
});

test('strafe right at yaw 0 is -x; a quarter turn right faces -x', () => {
  assert.ok(near(stepFly([0, 0, 0], { ...idle, move_strafe: 1 }), [-FLY_SPEED / 20, 0, 0]));
  assert.ok(near(lookVectors(Math.PI / 2, 0).forward, [-1, 0, 0]));
});

test('pitch up flies upward; jump and sneak rise and sink', () => {
  assert.ok(stepFly([0, 0, 0], { ...idle, move_forward: 1, pitch: Math.PI / 2 })[1] > 0.5);
  assert.ok(stepFly([0, 0, 0], { ...idle, jump: true })[1] > 0);
  assert.ok(stepFly([0, 0, 0], { ...idle, sneak: true })[1] < 0);
});

test('interpolate blends the last two ticks', () => {
  assert.deepEqual(interpolate([0, 0, 0], [2, 4, 6], 0.5), [1, 2, 3]);
});

test('clock yields 20 ticks per second at any frame rate and caps stalls', () => {
  for (const frame of [1000 / 144, 1000 / 60, 1000 / 30]) {
    const c = new TickClock();
    let ticks = 0;
    for (let t = 0; t <= 10000; t += frame) ticks += c.advance(t).ticks;
    assert.ok(Math.abs(ticks - 200) <= 1, `${ticks} ticks at ${frame} ms frames`);
  }
  const c = new TickClock();
  c.advance(0);
  assert.equal(c.advance(10 * TICK_MS).ticks, 5);
});

test('frame times report nearest-rank percentiles over the window', () => {
  const f = new FrameTimes(100);
  for (let i = 1; i <= 100; i++) f.push(i);
  assert.equal(f.percentile(50), 50);
  assert.equal(f.percentile(99), 99);
  f.push(1000); // overwrites the oldest
  assert.equal(f.count, 100);
  assert.equal(f.percentile(100), 1000);
  assert.equal(new FrameTimes().percentile(99), 0);
});
