// client/tools/file.mjs and critic --file with a fake gh: no network. Run: node --test client/test/file.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileReport, repros } from '../tools/file.mjs';
import { main } from '../tools/critic.mjs';
import { renderReport } from '../tools/report.mjs';

const repro = 'case a\nstart walk\nexpect mode = walk\n';
const finding = { kind: 'control-flow', severity: 'low', goal: 'g', observation: 'o', suggestion: 's', repro };
const report = renderReport([finding], { runDate: '2026-01-01' });

// A fake gh: `existing` is what the idea search returns; every call and created body is recorded.
const fakeGh = (existing = []) => {
  const calls = [];
  const gh = async (args) => {
    calls.push(args);
    if (args[1] === 'list') return JSON.stringify(existing);
    gh.body = readFileSync(args[args.indexOf('--body-file') + 1], 'utf8');
    return 'https://github.com/o/r/issues/9\n';
  };
  gh.calls = calls;
  return gh;
};

test('a new report creates exactly one issue, without the idea label', async () => {
  const gh = fakeGh([{ number: 1, title: 'other', body: 'nothing' }]);
  const r = await fileReport(report, gh);
  assert.deepEqual(r, { filed: true, url: 'https://github.com/o/r/issues/9' });
  const creates = gh.calls.filter((a) => a[1] === 'create');
  assert.equal(creates.length, 1);
  assert.ok(!creates[0].includes('--label'));
  assert.equal(creates[0][creates[0].indexOf('--title') + 1], report.title);
  assert.equal(gh.body, report.body);
  assert.deepEqual(gh.calls[0].slice(0, 5), ['issue', 'list', '--label', 'idea', '--state']);
  assert.equal(gh.calls[0][5], 'all');
});

test('a report with the same title is skipped and names the match', async () => {
  const gh = fakeGh([{ number: 7, title: report.title, body: '' }]);
  const r = await fileReport(report, gh);
  assert.deepEqual(r, { filed: false, duplicateOf: { number: 7, title: report.title } });
  assert.equal(gh.calls.filter((a) => a[1] === 'create').length, 0);
});

test('a report sharing a repro with an earlier issue is skipped', async () => {
  const gh = fakeGh([{ number: 3, title: 'Play critic findings, 2025-12-01', body: renderReport([finding], { runDate: 'x' }).body }]);
  const r = await fileReport(report, gh);
  assert.equal(r.filed, false);
  assert.equal(r.duplicateOf.number, 3);
  assert.equal(gh.calls.filter((a) => a[1] === 'create').length, 0);
});

test('repros reads fenced input blocks of any fence length', () => {
  assert.deepEqual(repros(report.body), [repro.trim()]);
  assert.deepEqual(repros('x\n````input\na\n```\nb\n````\n'), ['a\n```\nb']);
});

const sink = () => ({ text: '', write(s) { this.text += s; } });
const runMain = async (argv, gh) => {
  const out = sink();
  const code = await main(out, sink(), { model: async () => JSON.stringify([finding]), runDate: '2026-01-01', gh }, argv);
  return { out: out.text, code };
};

test('without --file, gh is never called', async () => {
  const gh = fakeGh();
  const { code } = await runMain([], gh);
  assert.equal(code, 0);
  assert.equal(gh.calls.length, 0);
});

test('with --file, the report is filed, or a duplicate is named', async () => {
  const gh = fakeGh();
  assert.match((await runMain(['--file'], gh)).out, /filed https:\/\/github\.com\/o\/r\/issues\/9/);
  const dup = fakeGh([{ number: 5, title: 'Play critic findings, 2026-01-01', body: '' }]);
  assert.match((await runMain(['--file'], dup)).out, /duplicate of #5/);
});
