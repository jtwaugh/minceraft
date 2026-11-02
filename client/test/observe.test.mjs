import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createSession } from './dsl.mjs';
import { PATHS, playerObservation, debugObservation } from '../tools/observe.mjs';

/** The paths named in the `Paths:` paragraph of dsl.mjs's header comment, groups expanded. */
function documentedPaths() {
  const header = readFileSync(new URL('./dsl.mjs', import.meta.url), 'utf8').split('\nimport ')[0];
  const text = header.split('// Paths:')[1].replace(/\n\/\/ ?/g, ' ').replace(/\([^)]*\)/g, '');
  const out = [];
  for (const item of text.split(',')) {
    const token = item.trim().split(/\s+/)[0].replace(/\.$/, '');
    if (!/^[a-z_.<>|0-9-]+$/.test(token)) continue;
    const m = token.match(/^(.*)\.([a-z]+(?:\|[a-z]+)+)$/);
    if (m) out.push(...m[2].split('|').map((a) => `${m[1]}.${a}`));
    else out.push(token);
  }
  return out;
}

test('every documented path is classified', () => {
  const paths = documentedPaths().flatMap((p) => (p.includes('<1-5>') ? [1, 2, 3, 4, 5].map((n) => p.replace('<1-5>', n)) : [p]));
  assert.ok(paths.length > 40, `parsed ${paths.length} paths`);
  assert.deepEqual(paths.filter((p) => !(p in PATHS)), []);
});

test('the table only names paths read() serves', () => {
  const s = createSession(['walk', 'raft']);
  for (const p of Object.keys(PATHS)) {
    try { s.read(p); } catch (e) { assert.doesNotMatch(e.message, /unknown path/, p); }
  }
  s.close();
});

test('fresh session: player data holds no debug path, debug data holds them', () => {
  const s = createSession(['walk', 'raft']);
  const p = playerObservation(s);
  assert.equal(p.mode, 'walk');
  assert.equal(p.locked, false);
  assert.deepEqual(p.hud.hearts, { full: 10, half: 0, empty: 0 });
  assert.equal(p.hud.belt.slots.length, 5);
  assert.equal(p.hud.ready, 'none');
  assert.equal(p.console, null);
  const keys = JSON.stringify(p);
  for (const [path, kind] of Object.entries(PATHS)) {
    if (kind === 'debug') assert.ok(!keys.includes(`"${path}"`), path);
  }
  const d = debugObservation(s);
  for (const path of ['rule.primary', 'events', 'sim_tick', 'field.crop', 'eye.x', 'animals']) assert.ok(path in d, path);
  assert.ok(!('mode' in d) && !('hud.selected' in d));
  s.close();
});

test('after steps: slot, aim and console show up', () => {
  const s = createSession(['walk', 'raft', 'debug']);
  s.exec('lock');
  s.exec('press Digit3');
  s.exec('frame 50 x2');
  const p = playerObservation(s);
  assert.equal(p.hud.belt.selected, 2);
  assert.equal(typeof p.target.outlined, 'boolean');
  assert.equal(p.target.kind, s.read('target.kind'));
  assert.ok(!('id' in p.target) && !('face' in p.target));
  s.exec('stand field');
  assert.equal(playerObservation(s).target.outlined, true);
  assert.equal(debugObservation(s)['field.ground'], s.read('field.ground'));
  s.exec('press Backquote');
  assert.equal(playerObservation(s).mode, 'console');
  assert.deepEqual(Object.keys(playerObservation(s).console), ['text', 'last']);
  s.close();
});

test('the work and render counters are diagnostic, and no other path moved class', () => {
  const diagnostic = ['regions.max_per_frame', 'chunks.max_per_tick', 'verts.max_per_frame', 'events.overbudget', 'render.faults'];
  assert.deepEqual(Object.keys(PATHS).filter((p) => PATHS[p] === 'diagnostic').sort(), diagnostic.sort());
  assert.equal(PATHS['regions.loaded'], 'debug');
  const s = createSession(['walk', 'stream']);
  try {
    const d = debugObservation(s);
    for (const p of diagnostic) assert.ok(p in d, p);
    assert.ok(!JSON.stringify(playerObservation(s)).includes('max_per_frame'));
  } finally { s.close(); }
});
