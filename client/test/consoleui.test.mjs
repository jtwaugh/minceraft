// Run: node --test client/test/consoleui.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { OutputBuffer, classify, FADE_DELAY_MS, VISIBLE_LINES } from '../consoleui.js';

test('classify spots the command error messages', () => {
  for (const l of ["unknown command 'x' (try 'help')", 'wrong number of arguments (1); usage: give', "bad value 'q' for <n>; usage: x", "'give' can't be used while paused", 'count must be 1 to 64, got 70', "unknown item 'x'", 'missing name']) {
    assert.equal(classify(l), 'error', l);
  }
  for (const l of ['gave item seeds', 'spawned 2 cow', 'cleared belt', 'usage: give tool <name>']) {
    assert.equal(classify(l), 'result', l);
  }
});

test('only the newest lines show, but all are kept', () => {
  const b = new OutputBuffer();
  b.setOpen(true, 0);
  for (let i = 0; i < 25; i++) b.output(`line ${i}`, 0);
  assert.equal(b.lines.length, 25);
  const v = b.visible(0);
  assert.equal(v.length, VISIBLE_LINES);
  assert.equal(v[0].text, 'line 15');
  assert.equal(v.at(-1).text, 'line 24');
});

test('echo lines keep their kind', () => {
  const b = new OutputBuffer();
  b.echo('give item seeds', 0);
  b.output('gave item seeds', 0);
  assert.deepEqual(b.lines.map((l) => l.kind), ['echo', 'result']);
  assert.equal(b.lines[0].text, '> give item seeds');
});

test('output fades the delay after the console closes, and stays while open', () => {
  const b = new OutputBuffer();
  b.setOpen(true, 0);
  b.output('hi', 100);
  assert.equal(b.visible(10 * FADE_DELAY_MS).length, 1); // open: never fades
  b.setOpen(false, 1000);
  assert.equal(b.visible(1000 + FADE_DELAY_MS - 1).length, 1);
  assert.equal(b.visible(1000 + FADE_DELAY_MS).length, 0);
  assert.equal(b.lines.length, 1); // still kept
});

test('late output restarts the fade', () => {
  const b = new OutputBuffer();
  b.setOpen(true, 0);
  b.setOpen(false, 0);
  b.output('result of the line just submitted', 5000);
  assert.equal(b.visible(5000 + FADE_DELAY_MS - 1).length, 1);
  assert.equal(b.visible(5000 + FADE_DELAY_MS).length, 0);
});
