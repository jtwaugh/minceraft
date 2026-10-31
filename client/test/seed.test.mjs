import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseSeed, randomSeed, withSeed, DEFAULT_SEED } from '../seed.js';
import { menuModel } from '../menu.js';

test('integer seeds are taken as numbers, negatives wrap to u64', () => {
  assert.equal(parseSeed('12345'), 12345n);
  assert.equal(parseSeed(' 42 '), 42n);
  assert.equal(parseSeed('-1'), 2n ** 64n - 1n);
  assert.equal(parseSeed('18446744073709551616'), 0n);
});

test('blank or missing text is the default seed', () => {
  for (const t of [null, undefined, '', '   ']) assert.equal(parseSeed(t), BigInt(DEFAULT_SEED));
});

test('text seeds hash with FNV-1a 64, like the viewer', () => {
  assert.equal(parseSeed('a'), 0xaf63dc4c8601ec8cn); // published FNV-1a 64 test vector
  assert.notEqual(parseSeed('raft'), parseSeed('Raft'));
  const h = parseSeed('mince raft');
  assert.ok(h >= 0n && h < 2n ** 64n);
});

test('random seeds are decimal integers that parse back to themselves', () => {
  for (const r of [() => 0, () => 0.999999, Math.random]) {
    const s = randomSeed(r);
    assert.match(s, /^\d+$/);
    assert.ok(Number.isSafeInteger(+s));
    assert.equal(parseSeed(s), BigInt(s));
  }
  assert.notEqual(randomSeed(() => 0.1), randomSeed(() => 0.2));
});

test('withSeed adds or replaces ?seed= and keeps other params', () => {
  assert.equal(withSeed('client/', '99'), 'client/?seed=99');
  assert.equal(withSeed('client/?scene=raft', 'my world'), 'client/?scene=raft&seed=my+world');
  assert.equal(withSeed('client/?seed=1', '2'), 'client/?seed=2');
  assert.equal(withSeed('client/', '  '), 'client/');
  assert.equal(new URLSearchParams(withSeed('client/', 'a&b=c').split('?')[1]).get('seed'), 'a&b=c');
});

test('only Test play has a seed box', () => {
  const [world, play] = menuModel(true, false);
  assert.equal(world.seedBox, false);
  assert.equal(play.seedBox, true);
});
