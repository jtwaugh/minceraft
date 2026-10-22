// The controller's eased sprint FOV (SPEC Sprinting, Camera) through the DSL's `fov` path.
// look.fov_degrees is 70, sprint.fov_extra_degrees 10 and sprint.fov_ease_seconds 0.15 in data/input.toml.
// Run: node --test client/test/sprint_fov.test.mjs (needs ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { parse, run } from './dsl.mjs';

// Streamed terrain is open ground, so the sprint is not cut short by a wall.
const script = (body) => parse(`case fov\nstart walk stream\nlock\nframe 100 x40\n${body}`)[0];
const sprint = 'down KeyW\nframe 50\nup KeyW\nframe 50\ndown KeyW\nframe 50\n';

test('fov stays at look.fov_degrees while not sprinting', () => {
  run(script('expect fov = 70\ndown KeyW\nframe 50 x10\nexpect sprinting = false\nexpect fov = 70'));
});

test('sprinting widens fov by a step proportional to frame time, never past the target', () => {
  run(script(`${sprint}expect sprinting = true\nexpect fov > 70\nexpect fov < 80\nframe 1000\nexpect fov = 80\nframe 50\nexpect fov = 80`));
});

test('the full widening takes fov_ease_seconds', () => {
  run(script(`${sprint}expect sprinting = true\nframe 150\nexpect fov = 80`));
});

test('fov eases back to the base when the sprint ends, and stops there', () => {
  run(script(`${sprint}frame 1000\nexpect fov = 80\nup KeyW\nframe 20\nexpect sprinting = false\nexpect fov < 80\nexpect fov > 70\nframe 150\nexpect fov = 70\nframe 50\nexpect fov = 70`));
});

test('a mid-way reading is eased linearly', () => {
  const lines = `${sprint}frame 1000\nup KeyW\nframe 75\nexpect fov ~ 75`;
  run(script(lines));
});

test('the expectation can fail', () => {
  assert.throws(() => run(script('expect fov = 80')), /fov/);
});
