// client/tools/finding.mjs: finding schema. Run: node --test client/test/finding.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { validateFinding, parseFindings, SEVERITIES, KINDS } from '../tools/finding.mjs';

const good = () => ({
  kind: 'control-flow',
  severity: 'low',
  goal: 'open the console',
  observation: 'stuck',
  suggestion: 'fix it',
  repro: 'frame 1\n',
});

const with_ = (field, value) => Object.assign(good(), { [field]: value });

test('a valid finding passes', () => {
  assert.deepEqual(validateFinding(good()), { ok: true, errors: [] });
  assert.deepEqual(SEVERITIES, ['low', 'medium', 'high']);
});

test('each field is required', () => {
  for (const f of Object.keys(good())) {
    const o = good();
    delete o[f];
    const r = validateFinding(o);
    assert.equal(r.ok, false, f);
    assert.match(r.errors[0], new RegExp(f));
  }
});

test('non-string fields are rejected', () => {
  for (const f of Object.keys(good())) {
    assert.equal(validateFinding(with_(f, 3)).ok, false, f);
  }
  assert.equal(validateFinding(null).ok, false);
  assert.equal(validateFinding([]).ok, false);
});

test('unknown kind, unknown severity and empty repro are rejected', () => {
  assert.match(validateFinding(with_('kind', 'bug')).errors[0], /kind/);
  assert.match(validateFinding(with_('severity', 'urgent')).errors[0], /severity/);
  assert.match(validateFinding(with_('repro', '  \n')).errors[0], /repro/);
  assert.equal(validateFinding(with_('kind', 'layout')).ok, true);
});

test('perf and render kinds are accepted; other kinds still are not', () => {
  assert.deepEqual(KINDS, ['control-flow', 'layout', 'perf', 'render']);
  for (const kind of ['perf', 'render']) {
    assert.deepEqual(validateFinding(with_('kind', kind)), { ok: true, errors: [] }, kind);
  }
  for (const kind of ['Perf', 'rendering', 'advisory', '']) {
    assert.match(validateFinding(with_('kind', kind)).errors[0], /unknown kind/, kind);
  }
  const r = parseFindings(JSON.stringify([with_('kind', 'perf'), with_('kind', 'render'), with_('kind', 'bug')]));
  assert.deepEqual(r.valid.map((f) => f.kind), ['perf', 'render']);
  assert.equal(r.rejected.length, 1);
});

test('parseFindings splits valid from rejected', () => {
  const bad = with_('severity', 'x');
  const r = parseFindings(JSON.stringify([good(), bad]));
  assert.equal(r.valid.length, 1);
  assert.equal(r.rejected.length, 1);
  assert.deepEqual(r.rejected[0].finding, bad);
  assert.match(r.rejected[0].errors[0], /severity/);
  assert.equal(r.error, null);
});

test('parseFindings does not throw on malformed input', () => {
  assert.match(parseFindings('not json').error, /malformed/);
  assert.match(parseFindings('42').error, /array/);
  assert.deepEqual(parseFindings('[]'), { valid: [], rejected: [], error: null });
});

test('advisory may be absent or a boolean, nothing else', () => {
  assert.equal(validateFinding(with_('advisory', true)).ok, true);
  assert.equal(validateFinding(with_('advisory', false)).ok, true);
  for (const v of ['true', 1, null, {}]) assert.equal(validateFinding(with_('advisory', v)).ok, false);
});
