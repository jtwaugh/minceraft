// client/tools/play.mjs: the JSON-lines player tool. Needs ./build.sh first.
// Run: node --test client/test/play.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { handle, handleLine, newState } from '../tools/play.mjs';
import { parse, run } from './dsl.mjs';

const started = () => {
  const state = newState();
  assert.equal(handle(state, { start: ['walk', 'raft'] }).ok, true);
  return state;
};

test('start, exec and observe answer with the player observation', () => {
  const state = started();
  try {
    const r = handle(state, { exec: 'frame 50' });
    assert.equal(r.ok, true);
    assert.equal(r.observation.mode, 'walk');
    assert.deepEqual(handle(state, { observe: 'player' }).observation, r.observation);
    assert.equal(handle(state, { start: ['fly'] }).ok, false);
  } finally { state.session.close(); }
});

test('a bad line or failing expect is ok:false and the session keeps working', () => {
  const state = started();
  try {
    const bad = handle(state, { exec: 'bogus' });
    assert.equal(bad.ok, false);
    assert.ok(bad.error);
    const exp = handle(state, { exec: 'expect mode = fly' });
    assert.equal(exp.ok, false);
    assert.match(exp.error, /expected mode = fly/);
    assert.equal(handle(state, { exec: 'frame 50' }).ok, true);
  } finally { state.session.close(); }
});

test('malformed JSON, unknown requests and use before start are errors', () => {
  const state = newState();
  for (const text of ['{nope', '[]', '3', '{"dance": 1}', '{"exec": "frame"}', '{"transcript": true}']) {
    const r = handleLine(state, text);
    assert.equal(r.ok, false, text);
    assert.ok(r.error, text);
  }
  assert.equal(handle(state, { start: 'walk' }).ok, false);
  const s = started();
  try {
    assert.equal(handle(s, { observe: 'debug' }).ok, false);
    assert.equal(handle(s, { exec: 5 }).ok, false);
  } finally { s.session.close(); }
});

test('player mode refuses debug reads without changing the session', () => {
  const state = started();
  try {
    handle(state, { exec: 'frame 50' });
    const before = handle(state, { transcript: true }).transcript;
    for (const exec of ['expect rule.primary = none', 'until sim_tick > 5', 'expect event x', 'expect no event x', 'mark', 'expect eye.y > 0', 'frame 50\nexpect moved < 1', 'expect\nrule.primary = none', 'until\nsim_tick > 5', 'expect\nevent x']) {
      const r = handle(state, { exec });
      assert.equal(r.ok, false, exec);
      assert.match(r.error, /debug/, exec);
    }
    assert.equal(handle(state, { observe: 'debug' }).ok, false);
    assert.equal(handle(state, { transcript: true }).transcript, before);
    for (const exec of ['expect mode = walk', 'expect locked = false', 'expect hud.selected = 0']) {
      assert.equal(handle(state, { exec }).ok, true, exec);
    }
  } finally { state.session.close(); }
});

test('player mode returns layout results and still refuses debug paths', () => {
  const state = started();
  try {
    const r = handle(state, { exec: 'layout 1366 768' });
    assert.equal(r.ok, true, r.error);
    assert.equal(typeof r.layout.scale, 'number');
    assert.ok(r.layout.rects && Array.isArray(r.layout.problems));
    assert.equal('value' in r, false);
    assert.ok(r.observation);
    assert.equal(handle(state, { exec: 'layout 0 5' }).ok, false);
    assert.equal('layout' in handle(state, { exec: 'frame 1' }), false);
    assert.equal(handle(state, { exec: 'layout 800 600\nexpect rule.primary = none' }).ok, false);
    assert.match(handle(state, { exec: 'expect rule.primary = none' }).error, /debug/);
    assert.equal(handle(state, { observe: 'debug' }).ok, false);
  } finally { state.session.close(); }
});

test('player mode can expect on the last layout, so layout findings can be verified', () => {
  const state = started();
  try {
    const { layout } = handle(state, { exec: 'layout 1366 768' });
    const r = handle(state, { exec: `expect layout.scale = ${layout.scale}` });
    assert.equal(r.ok, true, r.error);
    assert.equal(handle(state, { exec: `expect layout.problems = ${layout.problems.length}` }).ok, true);
  } finally { state.session.close(); }
});

test('verify mode runs debug reads and serves the debug observation', () => {
  const state = newState();
  assert.equal(handle(state, { start: ['walk', 'raft'], debug: true }).ok, true);
  try {
    for (const exec of ['expect rule.primary = none', 'until sim_tick > 5', 'expect no event x', 'mark', 'expect moved < 1']) {
      const r = handle(state, { exec });
      assert.equal(r.ok, true, `${exec}: ${r.error}`);
    }
    const r = handle(state, { observe: 'debug' });
    assert.equal(r.ok, true);
    assert.ok('sim_tick' in r.observation);
    assert.equal(handle(newState(), { start: ['walk'], debug: 'yes' }).ok, false);
  } finally { state.session.close(); }
});

test('the transcript is a script that replays through parse and run', () => {
  const state = started();
  try {
    for (const exec of ['click', 'lock', 'frame 50 x3', 'bogus', 'expect mode = walk']) handle(state, { exec });
    const { ok, transcript } = handle(state, { transcript: true });
    assert.equal(ok, true);
    assert.doesNotMatch(transcript, /bogus/);
    const cases = parse(transcript);
    assert.equal(cases.length, 1);
    run(cases[0]);
  } finally { state.session.close(); }
});

test('the real process answers one JSON line per request', async () => {
  const child = spawn(process.execPath, [new URL('../tools/play.mjs', import.meta.url).pathname]);
  let out = '';
  child.stdout.on('data', (d) => { out += d; });
  const done = new Promise((resolve) => child.on('close', resolve));
  const requests = [
    '{"start": ["walk"]}', '{"exec": "frame 50"}', '{"exec": "expect mode = fly"}', 'not json',
    '{"observe": "player"}', '{"transcript": true}',
  ];
  child.stdin.end(`${requests.join('\n')}\n`);
  await done;
  const responses = out.trim().split('\n').map((l) => JSON.parse(l));
  assert.equal(responses.length, requests.length);
  assert.deepEqual(responses.map((r) => r.ok), [true, true, false, false, true, true]);
  assert.equal(responses[4].observation.mode, 'walk');
  run(parse(responses[5].transcript)[0]);
});
