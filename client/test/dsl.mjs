// A small script language for the client's input state machine: DOM events go
// in, frames advance the real wasm sim, and `expect` lines assert on what the
// controller (client/controller.js) reports. Nothing renders; assertions read
// state from memory, so behaviour is checked as a grammar of event sequences
// rather than by looking at screenshots.
//
// One statement per line; `#` starts a comment. A file holds one or more cases.
//
//   case <name>                 a fresh world: fake canvas/document/window, new sim
//   start fly|walk [raft] [debug]  mode, and the raft test scene (default: walk, no terrain);
//                               the raft scene has three loose seeds on its islet and
//                               implies debug; `debug` turns the console keys on elsewhere
//   click                       click the canvas (requests pointer lock)
//   lock | unlock | blur        pointer lock gained / lost; window loses focus
//   down <code> | up <code>     KeyboardEvent.code, e.g. KeyW, Space, ShiftLeft
//   repeat <code>               an auto-repeat keydown
//   press <code>                down then up
//   button down|up left|right   mouse buttons
//   mouse <dx> <dy>             pointer-locked mouse movement
//   wheel <dy>
//   type <text>                 printable keys (letters, digits, space, - _) into the console's
//                               text box while it is open; elsewhere plain key presses.
//                               With the console open, `press Enter` submits the box, Backspace
//                               deletes a character, Escape closes it without submitting
//                               (the browser's exit from pointer lock is faked: it unlocks)
//   frame <ms> [xN]             run N frames of ms each (default 1)
//   spawn animal <kind>         put a cow, pig or chicken on the raft scene's islet
//   face animal                 turn to look at the first animal (a mousemove)
//   approach animal             teleport next to the first animal (it flees; reach is short)
//   held <held>                 what the player holds: empty, tool:hoe, item:seeds
//   mark                        remember the current eye, for `moved`
//   expect <path> <op> <value>  op: = != < > <= >= ~ (within 1e-3)
//
// Paths: mode (fly|walk|console), console.last (last command output line, or none),
// console.lines (output lines so far), console.text (the box), cheated, locked, ticks, alpha, yaw, pitch, eye.x|y|z,
// moved (distance since mark), moved.x|y|z, lock_requests, target.kind
// (none|block|entity), target.id, target.face, rule.primary|secondary|last
// (the matched rule's action, or none), box (outlined: true|false), underwater,
// belt.selected (0-based slot), belt.slot.<1-5> (tool name or none), hands (carried
// item or none), held (empty|tool:<name>|item:<name>), items (loose items in the world),
// animals (alive or dying), animals.dying (how many are dying), animals.hurt (how many are flashing).

import { readFileSync } from 'node:fs';
import { initSync, InputSampler, SimClock, FlyCamera, Walk, Scene } from '../pkg/game_web.js';
import { attachInput } from '../input.js';
import { createController } from '../controller.js';
import { spawnTestItems, readAnimals, faceAnimal, spawnTestAnimals } from '../overlay.js';

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

function world({ mode = 'walk', raft = false, debug = raft }) {
  init();
  const target = (extra) => Object.assign(new EventTarget(), extra);
  const canvas = target({ lockRequests: 0, requestPointerLock() { this.lockRequests++; } });
  const document = target({
    pointerLockElement: null,
    exitPointerLock() {
      this.pointerLockElement = null;
      this.dispatchEvent(new Event('pointerlockchange'));
    },
  });
  const window = target();
  const sampler = new InputSampler();
  const input = attachInput({ sampler, canvas, document, window, debug });
  const walk = raft ? Walk.in_scene(new Scene()) : new Walk(32, 70.5, 32);
  if (raft) spawnTestItems(walk);
  const fly = new FlyCamera(32, 94, 32);
  const controller = createController({
    input, walk, fly, clock: new SimClock(), document, flying: mode === 'fly',
  });
  controller.frame(0);
  const fire = (t, type, props = {}) => t.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), props));
  return { canvas, document, window, input, controller, walk, fire, mark: controller.state.eye, text: '', output: [] };
}

const SYMBOLS = { ' ': 'Space', '-': 'Minus', _: 'Minus' };

function keyCode(ch) {
  if (/^[a-z]$/i.test(ch)) return `Key${ch.toUpperCase()}`;
  if (/^[0-9]$/.test(ch)) return `Digit${ch}`;
  if (SYMBOLS[ch]) return SYMBOLS[ch];
  throw new Error(`cannot type '${ch}'`);
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
  if (path === 'mode') return w.input.console ? 'console' : s.flying ? 'fly' : 'walk';
  if (path.startsWith('console.') || path === 'cheated') {
    w.output.push(...w.walk.drain_command_output());
    if (path === 'cheated') return w.walk.cheated();
    if (path === 'console.last') return w.output.at(-1) ?? 'none';
    if (path === 'console.lines') return w.output.length;
    if (path === 'console.text') return w.text;
  }
  if (path === 'locked') return w.input.locked;
  if (path === 'lock_requests') return w.canvas.lockRequests;
  if (path === 'moved') return Math.hypot(...d);
  if (['ticks', 'alpha', 'yaw', 'pitch', 'underwater'].includes(path)) return s[path];
  if (path === 'belt.selected') return s.belt.selected;
  if (/^belt\.slot\.[1-9]$/.test(path)) return s.belt.slots[+path.slice(-1) - 1] ?? 'none';
  if (path === 'hands') return s.belt.hands ?? 'none';
  if (path === 'held') return s.belt.held;
  if (path === 'items') return w.walk.loose_items();
  if (path === 'animals') return readAnimals(w.walk).length;
  if (path === 'animals.dying') return readAnimals(w.walk).filter((a) => a.dying).length;
  if (path === 'animals.hurt') return readAnimals(w.walk).filter((a) => a.hurt).length;
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
  const key = (type, code, repeat = false) => {
    if (w.input.console && type === 'keydown') {
      // the text box owns these keys while the console is open
      if (code === 'Enter') { w.controller.submitConsole(w.text); w.text = ''; return; }
      if (code === 'Backspace') { w.text = w.text.slice(0, -1); return; }
      if (code === 'Escape') { w.controller.closeConsole(); w.text = ''; return; }
    }
    fire(document, type, { code, repeat });
  };
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
    case 'spawn': {
      if (args[0] !== 'animal' || !args[1]) throw new Error('spawn animal <kind>');
      spawnTestAnimals(w.walk, [args[1]]);
      return;
    }
    case 'face': {
      if (args[0] !== 'animal') throw new Error('face animal');
      const [dx, dy] = faceAnimal(w.walk, w.controller.state);
      return fire(document, 'mousemove', { movementX: dx, movementY: dy });
    }
    case 'approach': { 
      if (args[0] !== 'animal') throw new Error('approach animal');
      const a = readAnimals(w.walk)[0];
      if (!a) throw new Error('no animal to approach');
      const [fx, , fz] = w.walk.feet();
      const d = Math.hypot(a.pos[0] - fx, a.pos[2] - fz) || 1;
      const k = 1 - 1.2 / d;
      // feet 1.62 below the animal's middle, so the eye ray runs level
      w.walk.teleport(fx + (a.pos[0] - fx) * k, a.pos[1] + a.size[1] / 2 - 1.62, fz + (a.pos[2] - fz) * k);
      return;
    }
    case 'type': {
      const text = args.join(' ');
      for (const ch of text) {
        if (w.input.console) w.text += ch;
        key('keydown', keyCode(ch));
        key('keyup', keyCode(ch));
      }
      return;
    }
    case 'held': w.walk.set_held(args[0]); return;
    case 'mark': w.mark = w.controller.state.eye.slice(); return;
    case 'expect': {
      const [path, cmp, ...rest] = args;
      const raw = rest.join(' ');
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
    const [mode, ...flags] = c.steps[0].args;
    if (!['fly', 'walk'].includes(mode) || flags.some((f) => !['raft', 'debug'].includes(f))) {
      throw new Error(`line ${c.steps[0].line}: start fly|walk [raft] [debug]`);
    }
    Object.assign(opts, { mode, raft: flags.includes('raft') });
    if (flags.includes('debug')) opts.debug = true;
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
