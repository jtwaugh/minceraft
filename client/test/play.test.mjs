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
