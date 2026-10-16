// A small script language for the client's input state machine: DOM events go
// in, frames advance the real wasm sim, and `expect` lines assert on what the
// controller (client/controller.js) reports. Nothing renders; assertions read
// state from memory, so behaviour is checked as a grammar of event sequences
// rather than by looking at screenshots.
//
// One statement per line; `#` starts a comment. A file holds one or more cases.
//
//   case <name>                 a fresh world: fake canvas/document/window, new sim
//   start fly|walk [raft]       mode, and the raft test scene (default: walk, no terrain)
//   click                       click the canvas (requests pointer lock)
//   lock | unlock | blur        pointer lock gained / lost; window loses focus
//   down <code> | up <code>     KeyboardEvent.code, e.g. KeyW, Space, ShiftLeft
//   repeat <code>               an auto-repeat keydown
//   press <code>                down then up
//   button down|up left|right   mouse buttons
//   mouse <dx> <dy>             pointer-locked mouse movement
//   wheel <dy>
//   frame <ms> [xN]             run N frames of ms each (default 1)
//   held <held>                 what the player holds: empty, tool:hoe, item:seeds
//   mark                        remember the current eye, for `moved`
//   expect <path> <op> <value>  op: = != < > <= >= ~ (within 1e-3)
//
// Paths: mode (fly|walk), locked, ticks, alpha, yaw, pitch, eye.x|y|z,
// moved (distance since mark), moved.x|y|z, lock_requests, target.kind
// (none|block|entity), target.id, target.face, rule.primary|secondary|last
// (the matched rule's action, or none), box (outlined: true|false), underwater.

import { readFileSync } from 'node:fs';
import { initSync, InputSampler, SimClock, FlyCamera, Walk, Scene } from '../pkg/game_web.js';
import { attachInput } from '../input.js';
import { createController } from '../controller.js';

let ready = false;
function init() {
  if (ready) return;
  initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });
  ready = true;
}

/** Split a script into cases of `{ name, line, steps: [{ line, op, args }] }`. */
export function parse(src) {
  const cases = [];
  src.split('\n').forEach((raw, i) => {
    const text = raw.replace(/#.*/, '').trim();
    if (!text) return;
    const [op, ...args] = text.split(/\s+/);
    const line = i + 1;
    if (op === 'case') cases.push({ name: args.join(' '), line, steps: [] });
    else if (!cases.length) throw new Error(`line ${line}: '${op}' before any 'case'`);
    else cases.at(-1).steps.push({ line, op, args });
  });
  return cases;
}

function world({ mode = 'walk', raft = false }) {
  init();
  const target = (extra) => Object.assign(new EventTarget(), extra);
  const canvas = target({ lockRequests: 0, requestPointerLock() { this.lockRequests++; } });
  const document = target({ pointerLockElement: null });
  const window = target();
  const sampler = new InputSampler();
  const input = attachInput({ sampler, canvas, document, window });
  const walk = raft ? Walk.in_scene(new Scene()) : new Walk(32, 70.5, 32);
  const fly = new FlyCamera(32, 94, 32);
  const controller = createController({
    input, walk, fly, clock: new SimClock(), document, flying: mode === 'fly',
  });
  controller.frame(0);
  const fire = (t, type, props = {}) => t.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), props));
  return { canvas, document, window, input, controller, walk, fire, mark: controller.state.eye };
}

const BUTTONS = { left: 0, right: 2 };
const OPS = {
  '=': (a, b) => a === b,
  '!=': (a, b) => a !== b,
  '<': (a, b) => a < b,
  '>': (a, b) => a > b,
  '<=': (a, b) => a <= b,
  '>=': (a, b) => a >= b,
  '~': (a, b) => Math.abs(a - b) < 1e-3,
};

function read(w, path) {
  const s = w.controller.state;
  const d = s.eye.map((v, i) => v - w.mark[i]);
  const axis = { x: 0, y: 1, z: 2 };
  if (path === 'mode') return s.flying ? 'fly' : 'walk';
  if (path === 'locked') return w.input.locked;
  if (path === 'lock_requests') return w.canvas.lockRequests;
  if (path === 'moved') return Math.hypot(...d);
  if (['ticks', 'alpha', 'yaw', 'pitch', 'underwater'].includes(path)) return s[path];
  const aim = s.aim;
  if (path === 'target.kind') return aim.kind;
  if (path === 'target.id') return aim.id ?? -1;
  if (path === 'target.face') return aim.face ?? 'none';
  if (path === 'box') return aim.box !== null;
  if (/^rule\.(primary|secondary|last)$/.test(path)) return aim[path.slice(5)]?.action ?? 'none';
  const [head, ax] = path.split('.');
  if (ax in axis && head === 'eye') return s.eye[axis[ax]];
  if (ax in axis && head === 'moved') return d[axis[ax]];
  throw new Error(`unknown path '${path}'`);
}

function step(w, { op, args }) {
  const { canvas, document, window, fire } = w;
  const key = (type, code, repeat = false) => fire(document, type, { code, repeat });
  switch (op) {
    case 'click': return fire(canvas, 'click');
    case 'lock': document.pointerLockElement = canvas; return fire(document, 'pointerlockchange');
    case 'unlock': document.pointerLockElement = null; return fire(document, 'pointerlockchange');
    case 'blur': return fire(window, 'blur');
    case 'down': return key('keydown', args[0]);
    case 'up': return key('keyup', args[0]);
    case 'repeat': return key('keydown', args[0], true);
    case 'press': key('keydown', args[0]); return key('keyup', args[0]);
    case 'button': {
      const button = BUTTONS[args[1]];
      if (button === undefined || !['down', 'up'].includes(args[0])) throw new Error('button down|up left|right');
      return fire(document, `mouse${args[0]}`, { button });
    }
    case 'mouse': return fire(document, 'mousemove', { movementX: +args[0], movementY: +args[1] });
    case 'wheel': return fire(document, 'wheel', { deltaY: +args[0] });
    case 'frame': {
      const n = args[1] ? +args[1].replace(/^x/, '') : 1;
      for (let i = 0; i < n; i++) w.controller.frame(+args[0] / 1000);
      return;
    }
    case 'held': w.walk.set_held(args[0]); return;
    case 'mark': w.mark = w.controller.state.eye.slice(); return;
    case 'expect': {
      const [path, cmp, raw] = args;
      if (!OPS[cmp]) throw new Error(`unknown operator '${cmp}'`);
      const got = read(w, path);
      const want = typeof got === 'number' ? +raw : typeof got === 'boolean' ? raw === 'true' : raw;
      if (!OPS[cmp](got, want)) throw new Error(`expected ${path} ${cmp} ${raw}, got ${got}`);
      return;
    }
    default: throw new Error(`unknown statement '${op}'`);
  }
}

/** Run one parsed case; throws with the failing line number. */
export function run(c) {
  let i = 0;
  const opts = {};
  if (c.steps[0]?.op === 'start') {
    const [mode, scene] = c.steps[0].args;
    if (!['fly', 'walk'].includes(mode)) throw new Error(`line ${c.steps[0].line}: start fly|walk [raft]`);
    Object.assign(opts, { mode, raft: scene === 'raft' });
    i = 1;
  }
  const w = world(opts);
  try {
    for (; i < c.steps.length; i++) {
      try {
        step(w, c.steps[i]);
      } catch (e) {
        throw new Error(`line ${c.steps[i].line} (${c.steps[i].op} ${c.steps[i].args.join(' ')}): ${e.message}`);
      }
    }
  } finally {
    w.controller.detach();
    w.input.detach();
  }
}
