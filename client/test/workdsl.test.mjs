// The DSL's work-per-frame paths (scripts proper: stream.input).
// Run: node --test client/test/workdsl.test.mjs (needs ./build.sh first)
import test from 'node:test';
import assert from 'node:assert/strict';
import { createSession } from './dsl.mjs';
import { verifyFinding } from '../tools/verify.mjs';
import { VERTEX_BYTES } from '../../viewer/mesh.js';

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

test('render.faults needs a streamed world and is 0 on the start regions', () => {
  const s = createSession(['walk']);
  assert.deepEqual(s.exec('expect render.faults = 0'), { ok: false, error: 'render.faults needs `start ... stream`' });
  s.close();
  const t = createSession(['walk', 'stream']);
  assert.ok(t.exec('frame 50 x8').ok);
  assert.equal(t.read('render.faults'), 0);
  t.close();
});

// The opaque part with its middle face left out, as meshcheck.test.mjs drops one.
const dropFace = ({ verts, faces }) => {
  const q = faces >> 1, bytes = 4 * VERTEX_BYTES, all = new Uint8Array(verts);
  const out = new Uint8Array(verts.byteLength - bytes);
  out.set(all.subarray(0, q * bytes));
  out.set(all.subarray((q + 1) * bytes), q * bytes);
  return { verts: out.buffer, faces: faces - 1 };
};

test('a seeded missing face makes render.faults nonzero', () => {
  const s = createSession(['walk', 'stream'], 'seeded', { damage: dropFace });
  assert.ok(s.exec('frame 50 x8').ok);
  assert.ok(s.read('render.faults') > 0);
  assert.match(s.exec('expect render.faults = 0').error, /expected render.faults = 0, got [1-9]/);
  s.close();
});

test('a render finding asserting render.faults replays under the verify pass', () => {
  const repro = 'case a\nstart fly stream\nclick\nlock\ndown KeyW\nuntil regions.z = -1\nframe 50 x10\nexpect render.faults = 0\n';
  const finding = { kind: 'render', severity: 'low', goal: 'g', observation: 'o', suggestion: 's', repro };
  assert.equal(verifyFinding(finding).reproduced, true);
  assert.equal(verifyFinding({ ...finding, repro: repro.replace('= 0\n', '> 0\n') }).reproduced, false);
});
