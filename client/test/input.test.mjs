// Feeds scripted DOM-style events through client/input.js into the real wasm
// sampler and checks the resulting TickInputs. Needs ./build.sh first.
// Run: node --test client/test/input.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { initSync, InputSampler } from '../pkg/game_web.js';
import { attachInput } from '../input.js';

initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });

// Minimal EventTarget stand-ins for the canvas, document and window.
function target(extra = {}) {
  const t = new EventTarget();
  return Object.assign(t, extra);
}

function setup() {
  const canvas = target({
    locks: 0,
    requestPointerLock() {
      this.locks++;
    },
  });
  const document = target({ pointerLockElement: null });
  const window = target();
  const input = attachInput({ sampler: new InputSampler(), canvas, document, window });
  const fire = (t, type, props = {}) => {
    const e = Object.assign(new Event(type, { cancelable: true }), props);
    t.dispatchEvent(e);
    return e;
  };
  const lock = () => {
    fire(canvas, 'click');
    document.pointerLockElement = canvas;
    fire(document, 'pointerlockchange');
  };
  const unlock = () => {
    document.pointerLockElement = null;
    fire(document, 'pointerlockchange');
  };
  return { canvas, document, window, input, fire, lock, unlock };
}

test('click requests pointer lock; nothing is sampled before it', () => {
  const s = setup();
  s.fire(s.document, 'keydown', { code: 'KeyW', repeat: false });
  s.fire(s.document, 'mousedown', { button: 0 });
  assert.equal(s.input.tick().move_forward, 0);
  assert.equal(s.input.tick().primary, false);
  s.lock();
  assert.equal(s.canvas.locks, 1);
  assert.equal(s.input.locked, true);
});

test('keys, buttons and wheel become TickInput fields', () => {
  const s = setup();
  s.lock();
  s.fire(s.document, 'keydown', { code: 'KeyW', repeat: false });
  s.fire(s.document, 'keydown', { code: 'KeyW', repeat: true });
  s.fire(s.document, 'keydown', { code: 'KeyA', repeat: false });
  s.fire(s.document, 'keydown', { code: 'Space', repeat: false });
  s.fire(s.document, 'keydown', { code: 'ShiftLeft', repeat: false });
  s.fire(s.document, 'keydown', { code: 'KeyE', repeat: false });
  s.fire(s.document, 'keydown', { code: 'KeyQ', repeat: false });
  s.fire(s.document, 'mousedown', { button: 2 });
  s.fire(s.document, 'keydown', { code: 'Digit2', repeat: false });
  let t = s.input.tick();
  assert.deepEqual(
    [t.move_forward, t.move_strafe, t.jump, t.sneak, t.interact, t.drop],
    [1, -1, true, true, true, true],
  );
  assert.deepEqual([t.secondary, t.secondary_held, t.primary, t.tool_slot], [true, true, false, 1]);
  s.fire(s.document, 'mouseup', { button: 2 });
  s.fire(s.document, 'keyup', { code: 'KeyW' });
  s.fire(s.document, 'wheel', { deltaY: 100 });
  t = s.input.tick();
  assert.deepEqual([t.move_forward, t.secondary, t.secondary_held, t.tool_slot], [0, false, false, 2]);
  assert.equal(s.input.tick().tool_slot, null);
});

test('a click between two ticks is not lost and acts once', () => {
  const s = setup();
  s.lock();
  s.fire(s.document, 'mousedown', { button: 0 });
  s.fire(s.document, 'mouseup', { button: 0 });
  assert.equal(s.input.tick().primary, true);
  assert.equal(s.input.tick().primary, false);
});

test('look is live every frame and read at the tick boundary', () => {
  const s = setup();
  s.lock();
  s.fire(s.document, 'mousemove', { movementX: 100, movementY: 0 });
  const a = s.input.look();
  s.fire(s.document, 'mousemove', { movementX: 100, movementY: -1e6 });
  const b = s.input.look();
  assert.ok(b.yaw > a.yaw);
  assert.ok(Math.abs(b.pitch - Math.PI / 2) < 1e-6);
  const t = s.input.tick();
  assert.equal(t.yaw, b.yaw);
  assert.equal(t.pitch, b.pitch);
});

test('releasing the pointer or blurring clears held inputs', () => {
  const s = setup();
  s.lock();
  s.fire(s.document, 'keydown', { code: 'KeyW', repeat: false });
  s.fire(s.document, 'keydown', { code: 'Space', repeat: false });
  s.fire(s.document, 'mousedown', { button: 2 });
  s.unlock();
  let t = s.input.tick();
  assert.deepEqual([t.move_forward, t.jump, t.secondary_held], [0, false, false]);
  s.lock();
  s.fire(s.document, 'keydown', { code: 'KeyD', repeat: false });
  s.fire(s.window, 'blur');
  t = s.input.tick();
  assert.equal(t.move_strafe, 0);
});

test('detach removes the listeners', () => {
  const s = setup();
  s.lock();
  s.input.detach();
  s.fire(s.document, 'keydown', { code: 'KeyW', repeat: false });
  assert.equal(s.input.tick().move_forward, 0);
});
