// The DSL's `sprinting` path and the Tick field behind it (scripts proper: sprint.input).
// Run: node --test client/test/sprint_dsl.test.mjs (needs ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { parse, run } from './dsl.mjs';
import { readFileSync } from 'node:fs';
import { initSync, InputSampler } from '../pkg/game_web.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

// On the field: the raft scene starts the player in the water, where sprinting
// can't start (#894).
const script = (body) => parse(`case sprint dsl\nstart walk raft\nlock\nstand field\n${body}`)[0];

test('Tick.sprint is set on the drain after a double-tap of forward', () => {
  const s = new InputSampler();
  s.key_down('KeyW');
  s.key_up('KeyW');
  s.drain();
  s.drain();
  s.key_down('KeyW');
  const t = s.drain();
  assert.equal(t.sprint, true);
  t.free();
  assert.equal(s.drain().sprint, false);
  s.free();
});

test('a double-tap makes expect sprinting = true hold', () => {
  run(script('runway 12\nexpect sprinting = false\npress KeyW\nframe 100\ndown KeyW\nframe 100\nexpect sprinting = true'));
});

test('a single press does not sprint, and the expectation can fail', () => {
  run(script('down KeyW\nframe 50\nexpect sprinting = false'));
  assert.throws(() => run(script('down KeyW\nframe 100\nexpect sprinting = true')), /sprinting/);
});
