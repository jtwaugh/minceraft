// client/tools/critic.mjs with a canned model: no network. Run: node --test client/test/critic.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { runCritic, main, criticPrompt } from '../tools/critic.mjs';

const finding = (goal, repro) => ({
  kind: 'control-flow', severity: 'low', goal, observation: 'o', suggestion: 's', repro,
});
const good = finding('reproduces', 'case a\nstart walk\nexpect mode = walk\n');
const bad = finding('does not reproduce', 'case b\nstart walk\nexpect mode = fly\n');
const sink = () => ({ text: '', write(s) { this.text += s; } });
const run = async (output) => {
  const out = sink();
  const err = sink();
  const code = await main(out, err, { model: async () => output, runDate: '2026-01-01' });
  return { out: out.text, err: err.text, code };
};

test('the report holds only the finding that reproduces', async () => {
  const { out, code } = await run(JSON.stringify([good, bad]));
  assert.equal(code, 0);
  assert.match(out, /Play critic findings, 2026-01-01/);
  assert.match(out, /reproduces/);
  assert.doesNotMatch(out, /does not reproduce/);
});

test('findings wrapped in prose and a code fence are still read', async () => {
  const { out } = await run('Here you go:\n```json\n' + JSON.stringify([good]) + '\n```\n');
  assert.match(out, /reproduces/);
});

test('malformed, empty and non-reproducing output print "no findings" and exit 0', async () => {
  for (const output of ['not json', '', '[]', '{"a":1}', JSON.stringify([bad])]) {
    const { out, code } = await run(output);
    assert.equal(out, 'no findings\n', output);
    assert.equal(code, 0);
  }
});

test('a model error exits non-zero', async () => {
  const out = sink();
  const err = sink();
  const code = await main(out, err, { model: async () => { throw new Error('boom'); } });
  assert.equal(code, 1);
  assert.match(err.text, /boom/);
  assert.equal(out.text, '');
});

test('the prompt sent to the model is the critic prompt', async () => {
  let seen;
  await runCritic({ model: async (p) => { seen = p; return '[]'; } });
  assert.equal(seen, criticPrompt());
  assert.match(seen, /play critic/);
});
