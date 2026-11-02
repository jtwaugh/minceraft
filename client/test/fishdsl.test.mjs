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
