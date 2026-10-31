// The DSL's work-per-frame paths (scripts proper: stream.input).
// Run: node --test client/test/workdsl.test.mjs (needs ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { createSession } from './dsl.mjs';

const PATHS = ['regions.max_per_frame', 'chunks.max_per_tick', 'verts.max_per_frame'];

test('the work paths need a streamed world', () => {
  const s = createSession(['walk']);
  for (const p of PATHS) assert.deepEqual(s.exec(`expect ${p} >= 0`), { ok: false, error: `${p} needs \`start ... stream\`` });
  s.close();
});

test('the work paths count the start region and can fail', () => {
  const s = createSession(['walk', 'stream']);
  assert.ok(s.exec('frame 50 x8').ok);
  assert.equal(s.read('regions.max_per_frame'), 1);
  assert.ok(s.read('chunks.max_per_tick') > 0);
  assert.ok(s.read('verts.max_per_frame') > 0);
  assert.match(s.exec('expect regions.max_per_frame > 1').error, /expected regions.max_per_frame > 1, got 1/);
  s.close();
});

test('the work paths reset with each case', () => {
  const a = createSession(['walk', 'stream']);
  a.exec('frame 50 x8');
  const verts = a.read('verts.max_per_frame');
  a.close();
  const b = createSession(['walk', 'stream']);
  assert.equal(b.read('regions.max_per_frame'), 1); // the first frame, in createSession, meshes region 0 0
  assert.ok(b.read('verts.max_per_frame') <= verts);
  b.close();
});
