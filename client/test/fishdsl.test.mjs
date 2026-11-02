// The DSL's `spawn fish` statement and the `fish` and `fish.near` paths.
// Run: node --test client/test/fishdsl.test.mjs (needs ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { parse, run } from './dsl.mjs';

const lake = (body) => parse(`case fish dsl\nstart walk stream at 20.5 122.5\nframe 50 x40\n${body}`)[0];

test('spawn fish with an unknown species fails with a clear message', () => {
  assert.throws(() => run(lake('spawn fish nope')), /unknown species 'nope'/);
});

test('spawn fish on dry land fails with a clear message', () => {
  const c = parse('case dry\nstart walk stream\nframe 50 x40\nspawn fish salmon')[0];
  assert.throws(() => run(c), /no water at/);
});

test('fish.near counts only fish within the radius, and rejects bad arguments', () => {
  assert.throws(() => run(lake('spawn fish salmon\nexpect fish.near 90 65 40 3 = 1')), /fish\.near/);
  assert.throws(() => run(lake('expect fish.near 1 2 = 0')), /fish\.near <x> <y> <z> <r>/);
});

test('bobber.near rejects bad arguments and counts zero with no line out', () => {
  assert.throws(() => run(lake('expect bobber.near 1 2 = 0')), /bobber\.near <x> <y> <z> <r>/);
  run(lake('expect bobber = none\nexpect bobber.near 20 64 122 50 = 0'));
});

test('readBobber and readRodTip read the game-web rows', async () => {
  const { readBobber, readRodTip } = await import('../overlay.js');
  const rows = (b) => ({ bobber: () => b, rod_tip: () => [1, 2, 3] });
  assert.equal(readBobber(rows([])), null);
  assert.deepEqual(readBobber(rows([1, 0, 4, 5, 6])), { state: 'floating', dipping: false, pos: [4, 5, 6] });
  assert.equal(readBobber(rows([0, 1, 0, 0, 0])).dipping, true);
  assert.equal(readBobber(rows([2, 0, 0, 0, 0])).state, 'resting');
  assert.deepEqual(readRodTip(rows([])), [1, 2, 3]);
});

test('readFish adds lured from the fish_lured rows', async () => {
  const { readFish } = await import('../overlay.js');
  const walk = {
    fish: () => [1, 0, 0, 0, 0, 1, 1, 2, 0, 0, 0, 0, 1, 1],
    fish_species: () => ['salmon', 'salmon'],
    fish_lured: () => [0, 1],
  };
  assert.deepEqual(readFish(walk).map((f) => f.lured), [false, true]);
});
