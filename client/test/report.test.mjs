// client/tools/report.mjs: grouped idea report. Run: node --test client/test/report.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { renderReport } from '../tools/report.mjs';

const finding = (n, repro = `frame ${n}\n`) => ({
  kind: n % 2 ? 'control-flow' : 'layout',
  severity: 'medium',
  goal: `goal ${n}`,
  observation: `saw ${n}`,
  suggestion: `try ${n}`,
  repro,
});

test('no findings gives null', () => {
  assert.equal(renderReport([], { runDate: '2026-10-27' }), null);
});

test('starts with the heading and has Status and Scope lines', () => {
  const r = renderReport([finding(1)], { runDate: '2026-10-27' });
  assert.ok(r.body.startsWith(`# Mince Raft: ${r.title}\n`));
  assert.match(r.title, /2026-10-27/);
  assert.match(r.body, /^Status: idea$/m);
  assert.match(r.body, /^Scope: game$/m);
});

test('every field and repro appears, in the given order', () => {
  const r = renderReport([finding(1), finding(2), finding(3)], { runDate: 'd' });
  let at = 0;
  for (const n of [1, 2, 3]) {
    for (const s of [`goal ${n}`, `saw ${n}`, `try ${n}`, '```input\nframe ' + n + '\n```']) {
      const i = r.body.indexOf(s, at);
      assert.ok(i >= at, s);
      at = i;
    }
  }
  assert.match(r.body, /Kind: layout/);
  assert.match(r.body, /Severity: medium/);
});

test('a repro containing a fence does not close the block early', () => {
  const r = renderReport([finding(1, 'a\n```\nb\n')], { runDate: 'd' });
  assert.match(r.body, /````input\na\n```\nb\n````\n/);
});

test('perf and render findings render like the other kinds', () => {
  const perf = { ...finding(1), kind: 'perf' };
  const render = { ...finding(2, 'start walk stream\nframe 60\n'), kind: 'render' };
  const r = renderReport([perf, render], { runDate: 'd' });
  assert.match(r.body, /## 1\. goal 1\n\n- Kind: perf\n- Severity: medium\n- Goal: goal 1\n/);
  assert.match(r.body, /## 2\. goal 2\n\n- Kind: render\n- Severity: medium\n- Goal: goal 2\n/);
  assert.match(r.body, /\*\*Repro:\*\*\n\n```input\nstart walk stream\nframe 60\n```\n/);
});
