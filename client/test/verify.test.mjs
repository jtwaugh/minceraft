// client/tools/verify.mjs: the verify pass. Run: node --test client/test/verify.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyFinding, verifyAll } from '../tools/verify.mjs';

const finding = (repro) => ({
  kind: 'control-flow', severity: 'low', goal: 'g', observation: 'o', suggestion: 's', repro,
});
const good = finding('case a\nstart walk\npress KeyW\nframe 50 x4\nexpect mode = walk\n');
const badExpect = finding('case a\nstart walk\nexpect mode = fly\n');
const badLine = finding('case a\nstart walk\nbogus thing\nexpect mode = walk\n');
const noExpect = finding('case a\nstart walk\nframe 50\n');

test('a repro whose expects hold is kept', () => {
  assert.equal(verifyFinding(good).reproduced, true);
});

test('a failed expect, a bad line and a missing expect are dropped with a reason', () => {
  for (const f of [badExpect, badLine, noExpect, finding('press KeyW\n')]) {
    const r = verifyFinding(f);
    assert.equal(r.reproduced, false);
    assert.ok(r.reason.length > 0);
  }
  assert.match(verifyFinding(noExpect).reason, /no expect/);
  assert.match(verifyFinding(badLine).reason, /line 3/);
});

test('replay runs in debug mode', () => {
  assert.equal(verifyFinding(finding('case a\nstart walk\nexpect cheated = false\n')).reproduced, true);
});

test('replaying twice gives the same result', () => {
  for (const f of [good, badExpect, badLine, noExpect]) {
    assert.deepEqual(verifyFinding(f), verifyFinding(f));
  }
});

test('verifyAll splits kept and dropped', () => {
  const { kept, dropped } = verifyAll([good, badExpect, noExpect]);
  assert.deepEqual(kept, [good]);
  assert.deepEqual(dropped.map((d) => d.finding), [badExpect, noExpect]);
  assert.ok(dropped.every((d) => d.reason));
});
