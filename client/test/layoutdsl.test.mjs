// The DSL's `layout` statement and the `layout.*` expect paths.
// Run: node --test client/test/layoutdsl.test.mjs (needs ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { parse, run, createSession } from './dsl.mjs';
import { readFileSync } from 'node:fs';
import { initSync } from '../pkg/game_web.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

const script = (body) => parse(`case layout dsl\nstart walk\n${body}`)[0];

test('layout.* paths match the value layout returns', () => {
  const s = createSession(['walk']);
  const { value } = s.execStep({ op: 'layout', args: ['1366', '768'] });
  s.close();
  const w = value.rects.belt.w;
  run(script([
    'layout 1366 768',
    `expect layout.scale = ${value.scale}`,
    `expect layout.problems = ${value.problems.length}`,
    `expect layout.belt.w = ${w}`,
    `expect layout.belt.fraction ~ ${w / 1366}`,
  ].join('\n')));
});

test('a layout.* expectation can fail', () => {
  assert.throws(() => run(script('layout 1366 768\nexpect layout.scale = 999')), /layout\.scale/);
});

test('layout.* before any layout is an error', () => {
  assert.throws(() => run(script('expect layout.scale = 1')), /earlier `layout/);
});

test('an unknown layout path is an error', () => {
  assert.throws(() => run(script('layout 1366 768\nexpect layout.bogus = 1')), /unknown path/);
});
