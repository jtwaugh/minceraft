// The line-at-a-time session API in dsl.mjs. Needs ./build.sh first.
// Run: node --test client/test/session.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { createSession } from './dsl.mjs';

test('steps lines one at a time and reads state', () => {
  const s = createSession(['walk']);
  try {
    assert.equal(s.read('mode'), 'walk');
    // like scripts/modes.input: keys only count once the pointer is locked and the player has settled
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
