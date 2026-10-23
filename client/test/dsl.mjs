// A small script language for the client's input state machine: DOM events go
// in, frames advance the real wasm sim, and `expect` lines assert on what the
// controller (client/controller.js) reports. Nothing renders; assertions read
// state from memory, so behaviour is checked as a grammar of event sequences
// rather than by looking at screenshots.
//
// One statement per line; `#` starts a comment. A file holds one or more cases.
//
//   case <name>                 a fresh world: fake canvas/document/window, new sim
//   start fly|walk [raft] [debug] [stream]  mode, and the raft test scene (default: walk, no terrain);
//                               the raft scene has three loose seeds on its islet and
//                               implies debug; `debug` turns the console keys on elsewhere;
//                               `stream` is a streamed world (seed 12345) starting over region 0 0,
//                               meshing one region per frame
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
//   respawn                     the death screen's respawn button: controller.respawn() (nothing while alive)
//   frame <ms> [xN]             run N frames of ms each (default 1)
//   spawn animal <kind>         put a cow, pig or chicken on the raft scene's islet
//   face animal                 turn to look at the first animal (a mousemove)
//   approach animal             teleport next to the first animal (it flees; reach is short)
//   held <held>                 what the player holds: empty, tool:hoe, item:seeds
//   drop <n>                    teleport n blocks up, to fall back and take fall damage
//   stand field                 teleport onto the raft scene's field and look straight down
//   stand planter               the same, on the islet's first planter
//   stand helm                  teleport onto the raft's helm cell, level gaze
//   place flower dandelion|rose   put that flower on the raft scene's field, in the cell the player
//                               looks at after `stand field`; `place tall_grass` puts tall grass there,
//                               `place air` clears it again
//   forage                      right-click the aimed tall grass (after `stand field`, `place tall_grass`),
//                               waiting out the cooldown after each miss, until a `foraged` event is seen
//                               (at most 200 tries)
//   runway <n>                  raft scene: stone ground n blocks long running ahead (-z) from the field's
//                               back edge, a 2-high stone wall across its end, and the player on its
//                               start facing along it (pitch level)
//   until <path> <op> <value>   run 100 ms frames until the expectation holds (at most 20000)
//   mark                        remember the current eye (and raft offset), for `moved` and `raft.moved`
//   expect event <name> [key=value ...]  an event with that name (and those fields) has been seen
//                               since the case began (`expect no event <name>` for the opposite)
//   expect <path> <op> <value>  op: = != < > <= >= ~ (within 1e-3)
//
// Paths: regions.loaded|freed|pending|x|z (stream only: regions meshed now, regions freed so far,
// regions waiting to be meshed, and the region under the eye), mode (fly|walk|console|inventory|death; death while the player is dead), console.last (last command output line, or none),
// console.lines (output lines so far), console.text (the box), cheated, locked, ticks, alpha, yaw, pitch, eye.x|y|z,
// moved (distance since mark), moved.x|y|z, lock_requests, target.kind
// (none|block|entity), target.id, target.face, rule.primary|secondary|last
// (the matched rule's action, or none), box (outlined: true|false), underwater,
// belt.selected (0-based slot), belt.slot.<1-5> (tool name or none), hands (carried
// item or none), held (empty|tool:<name>|item:<name>), items (loose items in the world),
// field.ground and field.crop (block names at the field's middle tile, or air),
// flower.block (block name in the cell `place` filled, or air),
// planter.ground and planter.crop (the same for the first planter),
// driving (true|false: the player is steering the raft), raft.heading (radians), raft.x|z (raft offset from rest), raft.moved (the raft's distance since mark),
// sprinting (true|false: the player is sprinting), fov (vertical field of view in degrees, eased while sprinting), health (half-hearts, as the HUD shows it), hearts.full|half|empty, hud.selected, hud.hands, phase (gathering|service), phase.left (whole seconds),
// events.foraged (`foraged` events seen since the case began),
// events (number of events seen since the case began), sim_tick (the sim's tick count), paused, animals (alive or dying), animals.dying (how many are dying), animals.hurt (how many are flashing).

import { readFileSync } from 'node:fs';
import { initSync, InputSampler, SimClock, FlyCamera, Walk, Scene } from '../pkg/game_web.js';
import { B } from '../../viewer/mesh.js';
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

function world({ mode = 'walk', raft = false, debug = raft, stream = false }) {
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
  const scene = raft ? new Scene() : null;
  const walk = raft ? Walk.in_scene(scene) : stream ? Walk.streamed(12345n, 50.5, 72, 40.5) : new Walk(32, 70.5, 32);
  if (raft) spawnTestItems(walk);
  const fly = stream ? new FlyCamera(50.5, 110, 40.5) : new FlyCamera(32, 94, 32);
  const controller = createController({
    input, walk, fly, clock: new SimClock(), document, flying: mode === 'fly', stream: stream ? { budget: 1 } : null,
    sprintFov: { base: sampler.fov_degrees, extra: sampler.sprint_fov_extra_degrees, seconds: sampler.sprint_fov_ease_seconds },
  });
  const seen = [];
  const frame = controller.frame;
  controller.frame = (dt) => {
    const state = frame(dt);
    seen.push(...state.events);
    return state;
  };
  controller.frame(0);
  const fire = (t, type, props = {}) => t.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), props));
  return { scene, canvas, document, window, input, controller, walk, fire, mark: controller.state.eye, markRaft: [0, 0, 0], events: seen, text: '', output: [] };
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

/** The field's middle tile, one above the ground: where `place` puts plants. */
function flowerCell(w) {
  const [fx, fz] = w.scene.field, m = Math.floor(w.scene.field_size / 2);
  return [fx + m, w.scene.field_y + 1, fz + m];
}

function read(w, path) {
  const s = w.controller.state;
  const d = s.eye.map((v, i) => v - w.mark[i]);
  const axis = { x: 0, y: 1, z: 2 };
  if (path === 'mode') return w.input.dead ? 'death' : w.input.console ? 'console' : w.input.inventory ? 'inventory' : s.flying ? 'fly' : 'walk';
  if (path.startsWith('console.') || path === 'cheated') {
    w.output.push(...w.walk.drain_command_output());
    if (path === 'cheated') return w.walk.cheated();
    if (path === 'console.last') return w.output.at(-1) ?? 'none';
    if (path === 'console.lines') return w.output.length;
    if (path === 'console.text') return w.text;
  }
  if (path.startsWith('regions.')) {
    const r = s.regions;
    if (!r) throw new Error('regions.* needs `start ... stream`');
    const field = { loaded: r.loaded, freed: r.freed, pending: r.pending, x: r.centre.rx, z: r.centre.rz }[path.slice(8)];
    if (field === undefined) throw new Error(`unknown path '${path}'`);
    return field;
  }
  if (path === 'events.foraged') return w.events.filter((e) => e.startsWith('foraged ')).length;
  if (path === 'events') return w.events.length;
  if (path === 'sim_tick') return w.walk.sim_tick();
  if (path === 'raft.moved') {
    const o = w.walk.raft_offset(s.alpha);
    return Math.hypot(o[0] - w.markRaft[0], o[2] - w.markRaft[2]);
  }
  if (path === 'driving') return w.walk.driving();
  if (path === 'raft.heading') return w.walk.raft_heading(s.alpha);
  if (/^raft\.[xz]$/.test(path)) return w.walk.raft_offset(s.alpha)[axis[path[5]]];
  if (path === 'sprinting') return w.walk.sprinting();
  if (path === 'paused') return w.walk.paused();
  if (path === 'locked') return w.input.locked;
  if (path === 'lock_requests') return w.canvas.lockRequests;
  if (path === 'moved') return Math.hypot(...d);
  if (['ticks', 'alpha', 'yaw', 'pitch', 'fov', 'underwater'].includes(path)) return s[path];
  if (path === 'belt.selected') return s.belt.selected;
  if (/^belt\.slot\.[1-9]$/.test(path)) return s.belt.slots[+path.slice(-1) - 1] ?? 'none';
  if (path === 'hands') return s.belt.hands ?? 'none';
  if (path === 'held') return s.belt.held;
  if (path === 'health') return s.hud.hearts.full * 2 + s.hud.hearts.half;
  if (path.startsWith('hearts.')) return s.hud.hearts[path.slice(7)];
  if (path === 'phase') return s.hud.timer.phase;
  if (path === 'phase.left') return s.hud.timer.secondsLeft;
  if (path === 'hud.selected') return s.hud.selected;
  if (path === 'hud.hands') return s.hud.hands ?? 'none';
  if (path === 'items') return w.walk.loose_items();
  if (path === 'animals') return readAnimals(w.walk).length;
  if (path === 'animals.dying') return readAnimals(w.walk).filter((a) => a.dying).length;
  if (path === 'animals.hurt') return readAnimals(w.walk).filter((a) => a.hurt).length;
  if (path === 'field.ground' || path === 'field.crop') {
    const [fx, fz] = w.scene.field, m = Math.floor(w.scene.field_size / 2);
    const id = w.walk.block_at(fx + m, w.scene.field_y + (path === 'field.crop'), fz + m);
    return Object.keys(B).find((k) => B[k] === id) ?? `block ${id}`;
  }
  if (path === 'flower.block') {
    const id = w.walk.block_at(...flowerCell(w));
    return Object.keys(B).find((k) => B[k] === id) ?? `block ${id}`;
  }
  if (path === 'planter.ground' || path === 'planter.crop') {
    const [px, pz] = w.scene.planters;
    const id = w.walk.block_at(px, w.scene.field_y + (path === 'planter.crop'), pz);
    return Object.keys(B).find((k) => B[k] === id) ?? `block ${id}`;
  }
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
    case 'respawn': return w.controller.respawn();
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
    case 'drop': { // fall from <n> blocks above the current feet
      const [fx, fy, fz] = w.walk.feet();
      w.walk.teleport(fx, fy + Number(args[0]), fz);
      return;
    }
    case 'stand': {
      if (!['field', 'planter', 'helm'].includes(args[0]) || !w.scene) throw new Error('stand field|planter|helm (raft scene only)');
      if (args[0] === 'helm') {
        const [hx, hy, hz] = w.scene.helm;
        w.walk.teleport(hx + 0.5, hy + 1, hz + 0.5);
        w.controller.frame(0.05);
        return;
      }
      const [fx, fz] = args[0] === 'field' ? w.scene.field : w.scene.planters;
      const m = args[0] === 'field' ? Math.floor(w.scene.field_size / 2) : 0;
      w.walk.teleport(fx + m + 0.5, w.scene.field_y + 1, fz + m + 0.5);
      // far more than a quarter turn, so the pitch clamps at straight down
      fire(document, 'mousemove', { movementX: 0, movementY: 2000 });
      w.controller.frame(0.05);
      return;
    }
    case 'runway': {
      const n = Number(args[0]);
      if (!w.scene || !(n >= 1)) throw new Error('runway <n> (raft scene only)');
      const [fx, fz] = w.scene.field, y = w.scene.field_y;
      for (let dz = 0; dz <= n; dz++) {
        for (let dx = 0; dx < w.scene.field_size; dx++) {
          const wall = dz === n;
          for (const h of wall ? [1, 2] : [0]) {
            if (!w.walk.place_block(fx + dx, y + h, fz + 2 - dz, B.stone)) throw new Error('cannot build runway');
          }
        }
      }
      w.walk.teleport(fx + 1.5, y + 1, fz + 2.5);
      fire(document, 'mousemove', { movementX: 0, movementY: -2000 });
      fire(document, 'mousemove', { movementX: 0, movementY: 0 });
      w.controller.frame(0.05);
      return;
    }
    case 'place': {
      const name = args[0] === 'flower' ? args[1] : args[0];
      if (!w.scene || !['dandelion', 'rose', 'tall_grass', 'air'].includes(name)) {
        throw new Error('place flower dandelion|rose, or place tall_grass|air (raft scene only)');
      }
      const [x, y, z] = flowerCell(w);
      if (!w.walk.place_block(x, y, z, B[name])) throw new Error('cannot place there');
      return;
    }
    case 'forage': {
      const foraged = () => read(w, 'events.foraged');
      for (let tries = 0; tries < 200 && !foraged(); tries++) {
        fire(document, 'mousedown', { button: BUTTONS.right });
        w.controller.frame(0.1);
        fire(document, 'mouseup', { button: BUTTONS.right });
        w.controller.frame(0.1);
        // a miss starts the 1200-tick cooldown; wait it out (65 s at 20 Hz)
        if (!foraged()) for (let i = 0; i < 650; i++) w.controller.frame(0.1);
      }
      if (!foraged()) throw new Error('no forage succeeded in 200 tries');
      return;
    }
    case 'until': {
      for (let i = 0; i < 20000; i++) {
        try { step(w, { op: 'expect', args }); return; } catch { w.controller.frame(0.1); }
      }
      return step(w, { op: 'expect', args });
    }
    case 'held': w.walk.set_held(args[0]); return;
    case 'mark': w.mark = w.controller.state.eye.slice(); w.markRaft = w.walk.raft_offset(w.controller.state.alpha); return;
    case 'expect': {
      const negated = args[0] === 'no' && args[1] === 'event';
      if (negated || args[0] === 'event') {
        const [name, ...fields] = args.slice(negated ? 2 : 1);
        if (!name) throw new Error('expect [no] event <name> [key=value ...]');
        const hit = w.events.some((e) => {
          const [n, ...kv] = e.split(' ');
          return n === name && fields.every((f) => kv.includes(f));
        });
        if (hit === negated) throw new Error(`${negated ? 'unexpected' : 'no'} event ${name}; seen: ${w.events.join(' | ') || 'none'}`);
        return;
      }
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

const START_USAGE = 'start fly|walk [raft] [debug] [stream]';

/** The world options for a `start` line's arguments; throws the usage message when they are bad. */
function parseStart(args) {
  if (!args.length) return {};
  const [mode, ...flags] = args;
  if (!['fly', 'walk'].includes(mode) || flags.some((f) => !['raft', 'debug', 'stream'].includes(f))) {
    throw new Error(START_USAGE);
  }
  const opts = { mode, raft: flags.includes('raft'), stream: flags.includes('stream') };
  if (flags.includes('debug')) opts.debug = true;
  return opts;
}

/**
 * A live world driven one DSL line at a time. `startArgs` are the words after `start`
 * (e.g. ['walk', 'raft']); none means the default world. `exec` (a DSL line) and
 * `execStep` (a parsed `{ op, args }`) never throw: they return `{ ok: true }` or
 * `{ ok: false, error }`. `transcript()` is the session as a valid script: `case <name>`,
 * the `start` line, then every line that succeeded (errored ones are dropped).
 * Call `close()` when done.
 */
export function createSession(startArgs = [], name = 'session') {
  const w = world(parseStart(startArgs));
  let open = true;
  const lines = [`case ${name}`, `start ${(startArgs.length ? startArgs : ['walk']).join(' ')}`];
  const execStep = ({ op, args }) => {
    try {
      if (!open) throw new Error('session is closed');
      if (op === 'start') throw new Error('start is only allowed when the session is created');
      if (op) {
        step(w, { op, args });
        lines.push([op, ...args].join(' '));
      }
      return { ok: true };
    } catch (e) {
      return { ok: false, error: e.message };
    }
  };
  return {
    exec(line) {
      const [op, ...args] = line.replace(/#.*/, '').trim().split(/\s+/);
      return execStep({ op, args });
    },
    execStep,
    read: (path) => read(w, path),
    transcript: () => `${lines.join('\n')}\n`,
    close() {
      if (!open) return;
      open = false;
      w.controller.detach();
      w.input.detach();
    },
  };
}

/** Run one parsed case; throws with the failing line number. */
export function run(c) {
  const first = c.steps[0]?.op === 'start' ? c.steps[0] : null;
  let session;
  try {
    if (first && !first.args.length) throw new Error(START_USAGE);
    session = createSession(first?.args);
  } catch (e) {
    throw new Error(`line ${first.line}: ${e.message}`);
  }
  try {
    for (const { line, op, args } of c.steps.slice(first ? 1 : 0)) {
      const r = session.execStep({ op, args });
      if (!r.ok) throw new Error(`line ${line} (${op} ${args.join(' ')}): ${r.error}`);
    }
  } finally {
    session.close();
  }
}
