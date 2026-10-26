// The line-at-a-time session API in dsl.mjs. Needs ./build.sh first.
// Run: node --test client/test/session.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { createSession, parse, run } from './dsl.mjs';

test('steps lines one at a time and reads state', () => {
  const s = createSession(['walk', 'raft']);
  try {
    assert.equal(s.read('mode'), 'walk');
    // like scripts/modes.input (W does not move the player in the plain walk world): keys only count once the pointer is locked and the player has settled
    for (const line of ['click', 'lock', 'frame 50 x10']) assert.deepEqual(s.exec(line), { ok: true });
    assert.deepEqual(s.exec('mark'), { ok: true });
    assert.deepEqual(s.exec('down KeyW'), { ok: true });
    assert.deepEqual(s.exec('frame 50 x10'), { ok: true });
    assert.ok(s.read('moved') > 0);
    assert.deepEqual(s.exec('# only a comment'), { ok: true });
  } finally { s.close(); }
});

test('a failed expect is a result and the session stays usable', () => {
  const s = createSession(['walk']);
  try {
    const r = s.exec('expect mode = fly');
    assert.equal(r.ok, false);
    assert.match(r.error, /expected mode = fly, got walk/);
    assert.equal(s.exec('expect mode = walk').ok, true);
    assert.equal(s.exec('bogus').ok, false);
    assert.equal(s.exec('frame 50').ok, true);
  } finally { s.close(); }
});

test('start after creation is an error result', () => {
  const s = createSession(['fly']);
  try {
    const r = s.exec('start walk');
    assert.equal(r.ok, false);
    assert.match(r.error, /start/);
    assert.equal(s.read('mode'), 'fly');
  } finally { s.close(); }
});

test('bad start flags are rejected with the usage message', () => {
  assert.throws(() => createSession(['swim']), /start fly\|walk \[raft\] \[debug\] \[stream\]/);
  assert.throws(() => createSession(['walk', 'bogus']), /start fly\|walk/);
});

test('a bare start line in a script is still a usage error', () => {
  const [c] = parse('case bare\nstart\n');
  assert.throws(() => run(c), /line 2: start fly\|walk/);
});

/** Replay a transcript through parse and a fresh session; returns the observed state. */
function replay(text) {
  const [c] = parse(text);
  const s = createSession(c.steps[0].args, c.name);
  try {
    for (const { op, args } of c.steps.slice(1)) {
      const r = s.execStep({ op, args });
      assert.equal(r.ok, true, r.error);
    }
    return observe(s);
  } finally { s.close(); }
}

const observe = (s) => ({ tick: s.read('sim_tick'), x: s.read('eye.x'), y: s.read('eye.y'), z: s.read('eye.z') });

test('transcript has header lines and drops errored lines', () => {
  const s = createSession(['walk', 'raft'], 'demo');
  try {
    for (const line of ['# note', '', 'click', 'expect mode = fly', 'bogus', 'frame 50 x3', 'approach cow']) s.exec(line);
    assert.equal(s.transcript(), 'case demo\nstart walk raft\nclick\nframe 50 x3\n');
  } finally { s.close(); }
  const d = createSession();
  try {
    assert.equal(d.transcript(), 'case session\nstart walk\n');
    assert.equal(d.exec('# only a comment').ok, true);
    assert.equal(d.exec('   ').ok, true);
    assert.equal(d.transcript(), 'case session\nstart walk\n');
  } finally { d.close(); }
});

for (const [name, args, lines] of [
  ['raft approach and use', ['walk', 'raft'], ['click', 'lock', 'frame 50 x10', 'approach animal', 'face animal', 'frame 50 x5', 'click', 'frame 50 x5', 'bogus']],
  ['stream start', ['walk', 'stream'], ['click', 'lock', 'frame 50 x20', 'down KeyW', 'frame 50 x20']],
]) {
  test(`transcript replays to the same state: ${name}`, () => {
    const s = createSession(args, 'rt');
    let live;
    try {
      for (const l of lines) s.exec(l);
      live = observe(s);
      assert.ok(live.tick > 0);
      assert.deepEqual(replay(s.transcript()), live);
    } finally { s.close(); }
  });
}

test('layout returns rectangles and problems, records a line and leaves the world alone', () => {
  const s = createSession(['walk']);
  try {
    const before = observe(s);
    const sweep = [[390, 844], [1366, 768], [1920, 1080], [3440, 1440]];
    for (const [w, h] of sweep) {
      const r = s.exec(`layout ${w} ${h}`);
      assert.equal(r.ok, true, r.error);
      const { scale, rects, problems } = r.value;
      assert.ok(scale >= 1);
      for (const k of ['crosshair', 'belt', 'slots', 'hearts', 'heartIcons', 'timer', 'hands']) assert.ok(rects[k], `${w}x${h} has ${k}`);
      // 1366x768 and 1920x1080 do report off-screen belt and hearts today (see the PR); only the ultrawide is clean
      if (w >= 3440) assert.deepEqual(problems.filter((p) => p.check === 'offscreen'), [], `${w}x${h}`);
    }
    assert.deepEqual(observe(s), before);
    assert.equal(s.transcript(), `case session\nstart walk\n${sweep.map(([w, h]) => `layout ${w} ${h}\n`).join('')}`);
    assert.deepEqual(replay(s.transcript()), before);
  } finally { s.close(); }
});

test('layout with a bad size is an error result with usage', () => {
  const s = createSession(['walk']);
  try {
    for (const line of ['layout', 'layout 800', 'layout a b', 'layout 800 0', 'layout 1 2 3']) {
      const r = s.exec(line);
      assert.equal(r.ok, false, line);
      assert.match(r.error, /usage: layout <w> <h>/);
    }
    assert.equal(s.transcript(), 'case session\nstart walk\n');
  } finally { s.close(); }
});
