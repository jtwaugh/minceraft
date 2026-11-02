// Per-frame wind state on the real wasm Wind. Needs ./build.sh first.
// Run: node --test client/test/windstate.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, Wind } from '../pkg/game_web.js';
import { createWindState, PHASE_RATE_MIN, MAX_FRAME_DT } from '../windstate.js';
import { SWAY_SPEED } from '../sway.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

const near = (a, b, what) => assert.ok(Math.abs(a - b) <= 1e-9, `${what}: ${a} vs ${b}`);

test('frame matches the wasm Wind at tick + alpha', () => {
  for (const [seed, x, z, tick, alpha] of [[12345, 12.5, -4.2, 100, 0.25], [7, -80.1, 33.3, 5000, 0.75]]) {
    const w = new Wind(BigInt(seed));
    const [gx, gz] = w.at(x, z, tick + alpha);
    const [ux, uz] = Wind.direction(gx, gz);
    const f = createWindState(w, Wind, SWAY_SPEED).frame(x, z, tick, alpha, 1 / 60, false);
    near(f.dir[0], ux, 'dir x');
    near(f.dir[1], uz, 'dir z');
    near(f.strength, w.strength(gx, gz), 'strength');
  }
});

// A stub whose strength jumps between calm and full: the phase must stay continuous.
test('phase is continuous and bounded when the strength changes abruptly', () => {
  let s = 0;
  const stub = { at: () => [s * 12, 0], strength: () => s };
  const state = createWindState(stub, Wind, SWAY_SPEED);
  const dt = 1 / 60;
  let prev = state.frame(0, 0, 0, 0, dt, false).phase;
  const maxStep = dt * SWAY_SPEED;
  for (let i = 1; i < 600; i++) {
    s = Math.floor(i / 30) % 2; // flips 0 <-> 1 every half second
    const p = state.frame(0, 0, i, 0, dt, false).phase;
    const d = (p - prev + 2 * Math.PI) % (2 * Math.PI);
    assert.ok(d >= dt * SWAY_SPEED * PHASE_RATE_MIN - 1e-12 && d <= maxStep + 1e-12, `step ${d} at frame ${i}`);
    prev = p;
  }
});

test('a huge dt is clamped', () => {
  const stub = { at: () => [12, 0], strength: () => 1 };
  const state = createWindState(stub, Wind, SWAY_SPEED);
  const p = state.frame(0, 0, 0, 0, 1000, false).phase;
  assert.ok(Math.abs(p - ((MAX_FRAME_DT * SWAY_SPEED) % (2 * Math.PI))) < 1e-12);
});

test('reduced motion: phase 0, no sway, real wind still reported', () => {
  const w = new Wind(12345n);
  const state = createWindState(w, Wind, SWAY_SPEED);
  state.frame(0, 0, 0, 0, 0.1, false);
  const f = state.frame(0, 0, 10, 0, 0.1, true);
  assert.equal(f.phase, 0);
  assert.equal(f.sway, 0);
  const [gx, gz] = w.at(0, 0, 10);
  near(f.strength, w.strength(gx, gz), 'strength');
  assert.ok(Math.hypot(...f.dir) > 0.99);
});
