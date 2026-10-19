// The controller's console mode with a fake walk and input: typing F in the
// console must not toggle fly, and submit/close restore the mode it came from.
// Run: node --test client/test/console.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { createController } from '../controller.js';

function setup() {
  const document = new EventTarget();
  const sent = [];
  const input = {
    locked: true,
    console: false,
    closed: [],
    closeConsole(o) {
      this.console = false;
      this.closed.push(o ?? {});
    },
  };
  const at = [1, 2, 3];
  const walk = {
    eye: () => at, feet: () => [1, 1, 3], teleport() {},
    submit_command: (l) => sent.push(l),
  };
  const fly = { eye: () => at, teleport() {} };
  const c = createController({ input, walk, fly, clock: {}, document });
  const keyF = () => document.dispatchEvent(Object.assign(new Event('keydown'), { code: 'KeyF', repeat: false }));
  return { c, input, sent, keyF };
}

test('F does not toggle fly in console mode', () => {
  const s = setup();
  s.input.console = true;
  s.keyF();
  assert.equal(s.c.state.flying, false);
  s.input.console = false;
  s.keyF();
  assert.equal(s.c.state.flying, true);
});

test('submit queues the line and relocks; the flying mode is kept', () => {
  const s = setup();
  s.keyF();
  s.input.console = true;
  s.c.submitConsole('give tool hoe');
  assert.deepEqual(s.sent, ['give tool hoe']);
  assert.deepEqual(s.input.closed, [{ submitted: true }]);
  assert.equal(s.c.state.flying, true);
});

test('escape closes without submitting; submit outside the console is ignored', () => {
  const s = setup();
  s.c.submitConsole('help');
  assert.deepEqual(s.sent, []);
  s.input.console = true;
  s.c.closeConsole();
  assert.deepEqual(s.sent, []);
  assert.deepEqual(s.input.closed, [{}]);
});
