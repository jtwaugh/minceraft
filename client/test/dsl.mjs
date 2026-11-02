// A small script language for the client's input state machine: DOM events go
// in, frames advance the real wasm sim, and `expect` lines assert on what the
// controller (client/controller.js) reports. Nothing renders; assertions read
// state from memory, so behaviour is checked as a grammar of event sequences
// rather than by looking at screenshots.
//
// One statement per line; `#` starts a comment. A file holds one or more cases.
//
//   case <name>                 a fresh world: fake canvas/document/window, new sim
//   start fly|walk [raft] [debug] [stream] [at <x> <z>]  mode, and the raft test scene (default: walk, no terrain);
//                               the raft scene has three loose seeds on its islet and
//                               implies debug; `debug` turns the console keys on elsewhere;
//                               `stream` is a streamed world (seed 12345) starting over region 0 0,
//                               meshing one region per frame (`at <x> <z>` starts elsewhere, 72 up); every frame's new and remeshed regions
//                               are meshed on the CPU as scene.js does (client/regionmesh.js, no upload)
//   click                       click the canvas (requests pointer lock, which is granted at once)
//   lock | unlock | blur        pointer lock gained / lost; window loses focus (and the lock)
//   down <code> | up <code>     KeyboardEvent.code, e.g. KeyW, Space, ShiftLeft
//   repeat <code>               an auto-repeat keydown
//   press <code>                down then up
//   button down|up left|right   mouse buttons
//   mouse <dx> <dy>             pointer-locked mouse movement
//   wheel <dy>
//   type <text>                 printable keys (letters, digits, space, - _ .) into the console's
//                               text box while it is open; elsewhere plain key presses.
//                               With the console open, `press Enter` submits the box, Backspace
//                               deletes a character, Escape closes it without submitting
//                               (the browser's exit from pointer lock is faked: it unlocks)
//   respawn                     the death screen's respawn button: controller.respawn() (nothing while alive)
//   frame <ms> [xN]             run N frames of ms each (default 1)
//   spawn animal <kind>         put a cow, pig or chicken on the raft scene's islet
//   spawn fish <species>        put a fish (a name in data/fish_bodies.toml) in the water two blocks east of
//                               the player and level with the feet; an error if that is not water or the species is unknown
//   spawn item <name>           drop a loose item a block east of the player (any scene)
//   face animal                 turn to look at the first animal (a mousemove)
//   approach animal             teleport next to the first animal (it flees; reach is short)
//   held <held>                 what the player holds: empty, tool:hoe, item:seeds
//   drop <n>                    teleport n blocks up, to fall back and take fall damage
//   stand field                 teleport onto the raft scene's field and look straight down
//   stand planter               the same, on the islet's first planter
//   stand helm                  teleport onto the raft's helm cell, level gaze
//   stand deck                  teleport onto the raft's deck, a cell south of the helm, level gaze
//   place flower dandelion|rose   put that flower on the raft scene's field, in the cell the player
//                               looks at after `stand field`; `place tall_grass` puts tall grass there,
//                               `place air` clears it again
//   forage                      right-click the aimed tall grass (after `stand field`, `place tall_grass`),
//                               waiting out the cooldown after each miss, until a `foraged` event is seen
//                               (at most 200 tries); `forage miss` waits for a `forage_missed` event instead
//   runway <n>                  raft scene: stone ground n blocks long running ahead (-z) from the field's
//                               back edge, a 2-high stone wall across its end, and the player on its
//                               start facing along it (pitch level)
//   layout <w> <h>              HUD layout for a w x h viewport from the current state: `exec`/`execStep`
//                               return { ok: true, value: { scale, rects, problems } } (hudLayout and layoutChecks,
//                               tileset from viewer/textures.json); changes nothing in the world,
//                               but remembers the result for the `layout.*` paths
//   until <path> <op> <value>   run 100 ms frames until the expectation holds (at most 20000)
//   mark                        remember the current eye (and raft offset), for `moved` and `raft.moved`
//   expect event <name> [key=value ...]  an event with that name (and those fields) has been seen
//                               since the case began (`expect no event <name>` for the opposite)
//   expect <path> <op> <value>  op: = != < > <= >= ~ (within 1e-3)
//
// Paths: regions.loaded|freed|pending|x|z (stream only: regions meshed now, regions freed so far,
// regions waiting to be meshed, and the region under the eye), regions.max_per_frame, chunks.max_per_tick,
// verts.max_per_frame (stream only: the most work any one frame has done since the case began: regions
// meshed plus remeshed, chunks the sim loaded as in the controller's `regions.chunks`, and vertices meshed over
// all mesh kinds), render.faults (stream only: crack faults client/meshcheck.js finds in the `opaque` part of
// every region meshed or remeshed since the case began; 0 when the meshes have no cracks), mode (fly|walk|console|inventory|death; death while the player is dead), console.last (last command output line, or none),
// console.lines (output lines so far), console.text (the box), cheated, time (time of day, 0..1), time.frozen, sun.x|y|z (unit vector toward the sun this frame, time of day plus alpha), daylight (scene brightness this frame), locked, prompt (the click cue text, "Click to play", while the game waits for a click, else null; `expect prompt = none` tests for null), hint (open|folded: the controls hint panel), ticks, alpha, yaw, pitch, eye.x|y|z,
// moved (distance since mark), moved.x|y|z, lock_requests, target.kind
// (none|block|entity), target.id, target.face, rule.primary|secondary|last
// (the matched rule's action, or none), box (outlined: true|false), underwater,
// belt.selected (0-based slot), belt.slot.<1-5> (tool name or none), hands (carried
// item or none), held (empty|tool:<name>|item:<name>), held.drawn (the icon the
// held-item pass draws, or none), hands.swing|use (hand animation phases: 0 idle, 1 mid-animation), items (loose items in the world),
// field.ground and field.crop (block names at the field's middle tile, or air),
// flower.block (block name in the cell `place` filled, or air),
// planter.ground and planter.crop (the same for the first planter),
// driving (true|false: the player is steering the raft), raft.heading (radians), raft.x|z (raft offset from rest), raft.moved (the raft's distance since mark),
// sprinting (true|false: the player is sprinting), fov (vertical field of view in degrees, eased while sprinting), health (half-hearts, as the HUD shows it), hearts.full|half|empty, hud.selected, hud.hands, hud.ready (the selected tool when the hands are empty, or none), phase (gathering|service), phase.left (whole seconds),
// layout.scale, layout.problems (count), layout.belt.w (CSS px), layout.belt.fraction (belt width / viewport width): the last `layout` statement's result (an error before any),
// layout.timer|slotnumbers, layout.hands.label (inside|outside|missing: whether the timer box, every slot-number glyph rect, or the hands' label rect is in the last layout and entirely within the viewport),
// particles (live particles: a burst per `forage_missed` event, gone after about 0.6 s of frames),
// events.foraged (`foraged` events seen since the case began), events.overbudget (stream only: frames whose work,
// as counted for the work paths, went over a budget in client/workbudget.js; each is an event
// `overbudget <field>=<value> ...` naming the fields over, e.g. `expect no event overbudget`),
// events (number of events seen since the case began), sim_tick (the sim's tick count), paused, animals (alive or dying), animals.dying (how many are dying), animals.hurt (how many are flashing), fish (fish in the world), fish.near <x> <y> <z> <r> (fish within r blocks of that point; the path takes arguments: `expect fish.near 10 64 3 5 = 1`).

import { readFileSync } from 'node:fs';
import { initSync, InputSampler, SimClock, FlyCamera, Walk, Scene } from '../pkg/game_web.js';
import * as terrain from '../../viewer/pkg/terrain_web.js';
import { B, VERTEX_BYTES, legendContext } from '../../viewer/mesh.js';
import { meshRegionCpu } from '../regionmesh.js';
import { overBudget } from '../workbudget.js';
import { meshCracks } from '../meshcheck.js';
import { REGION_SIZE } from '../stream.js';
import { attachInput } from '../input.js';
import { createController } from '../controller.js';
import { waitingForClick, CUE_TEXT } from '../waitingcue.js';
import { createHintState } from '../hintui.js';
import { heldDraw } from '../helditem.js';
import { hudLayout } from '../hudlayout.js';
import { layoutChecks } from '../hudcheck.js';
import { spawnTestItems, readAnimals, readFish, faceAnimal, spawnTestAnimals } from '../overlay.js';

let ready = false;
function init() {
  if (ready) return;
  initSync({ module: readFileSync(new URL('../pkg/game_web_bg.wasm', import.meta.url)) });
  terrain.initSync({ module: readFileSync(new URL('../../viewer/pkg/terrain_web_bg.wasm', import.meta.url)) });
  ready = true;
}

let meshCtx; // no textures in the harness: every face on layer 0, which changes no vertex counts
const STREAM_SEED = 12345n;

/**
 * Meshes this frame's new and remeshed regions as scene.js would (no upload); returns the work done
 * and the crack faults (client/meshcheck.js) in their `opaque` parts, after `damage` (tests only) edits each.
 */
function meshFrame(walk, r, damage) {
  meshCtx ??= legendContext(terrain.legend(), new Uint8Array(256 * 3 * 2));
  let verts = 0, faults = 0;
  for (const region of [...r.mesh, ...r.remesh]) {
    const { mesh } = meshRegionCpu({ walk, terrain, ctx: meshCtx, seed: STREAM_SEED }, region);
    for (const k of ['opaque', 'cutout', 'water', 'ice']) verts += mesh[k].verts.byteLength / VERTEX_BYTES;
    faults += meshCracks(damage ? damage(mesh.opaque) : mesh.opaque, REGION_SIZE).length;
  }
  return { work: { regions: r.mesh.length + r.remesh.length, chunks: r.chunks, verts }, faults };
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

function world({ mode = 'walk', raft = false, debug = raft, stream = false, at = [50.5, 40.5], budget = 1, damage = null }) {
  init();
  const target = (extra) => Object.assign(new EventTarget(), extra);
  const canvas = target({
    lockRequests: 0,
    requestPointerLock() { // as a browser grants it
      this.lockRequests++;
      document.pointerLockElement = canvas;
      document.dispatchEvent(new Event('pointerlockchange'));
    },
  });
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
  const walk = raft ? Walk.in_scene(scene) : stream ? Walk.streamed(STREAM_SEED, at[0], 72, at[1]) : new Walk(32, 70.5, 32);
  if (raft) spawnTestItems(walk);
  const fly = stream ? new FlyCamera(at[0], 110, at[1]) : new FlyCamera(32, 94, 32);
  const controller = createController({
    input, walk, fly, clock: new SimClock(), document, flying: mode === 'fly', stream: stream ? { budget } : null,
    sprintFov: { base: sampler.fov_degrees, extra: sampler.sprint_fov_extra_degrees, seconds: sampler.sprint_fov_ease_seconds },
  });
  const seen = [];
  const most = stream ? { regions: 0, chunks: 0, verts: 0 } : null; // running maxima of one frame's work
  const render = stream ? { faults: 0 } : null; // crack faults over every region meshed so far
  const hint = createHintState();
  document.addEventListener('pointerlockchange', () => hint.update(input.locked));
  const frame = controller.frame;
  controller.frame = (dt) => {
    const state = frame(dt);
    hint.update(input.locked);
    seen.push(...state.events);
    if (most) {
      const { work, faults } = meshFrame(walk, state.regions, damage);
      render.faults += faults;
      for (const [k, v] of Object.entries(work)) most[k] = Math.max(most[k], v);
      const over = overBudget(work);
      if (over.length) seen.push(`overbudget ${over.map((k) => `${k}=${work[k]}`).join(' ')}`);
    }
    return state;
  };
  controller.frame(0);
  const fire = (t, type, props = {}) => t.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), props));
  return { scene, canvas, document, window, input, hint, controller, walk, fire, mark: controller.state.eye, markRaft: [0, 0, 0], events: seen, most, render, text: '', output: [], layout: null };
}

const SYMBOLS = { ' ': 'Space', '-': 'Minus', _: 'Minus', '.': 'Period' };

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
  const work = { 'regions.max_per_frame': 'regions', 'chunks.max_per_tick': 'chunks', 'verts.max_per_frame': 'verts' }[path];
  if (work) {
    if (!w.most) throw new Error(`${path} needs \`start ... stream\``);
    return w.most[work];
  }
  if (path === 'render.faults') {
    if (!w.render) throw new Error(`${path} needs \`start ... stream\``);
    return w.render.faults;
  }
  if (path.startsWith('regions.')) {
    const r = s.regions;
    if (!r) throw new Error('regions.* needs `start ... stream`');
    const field = { loaded: r.loaded, freed: r.freed, pending: r.pending, x: r.centre.rx, z: r.centre.rz }[path.slice(8)];
    if (field === undefined) throw new Error(`unknown path '${path}'`);
    return field;
  }
  if (path.startsWith('layout.')) {
    if (!w.layout) throw new Error(`${path} needs an earlier \`layout <w> <h>\``);
    const v = w.layout[path.slice(7)];
    if (v === undefined) throw new Error(`unknown path '${path}'`);
    return v;
  }
  if (path === 'particles') return s.particleCount;
  if (path === 'events.foraged') return w.events.filter((e) => e.startsWith('foraged ')).length;
  if (path === 'events.overbudget') {
    if (!w.most) throw new Error(`${path} needs \`start ... stream\``);
    return w.events.filter((e) => e.startsWith('overbudget ')).length;
  }
  if (path === 'events') return w.events.length;
  if (path === 'sim_tick') return w.walk.sim_tick();
  if (path === 'raft.moved') {
    const o = w.walk.raft_offset(s.alpha);
    return Math.hypot(o[0] - w.markRaft[0], o[2] - w.markRaft[2]);
  }
  if (path === 'time') return w.walk.time_of_day();
  if (/^sun\.[xyz]$/.test(path)) return s.day.sunDir[axis[path[4]]];
  if (path === 'daylight') return s.day.daylight;
  if (path === 'time.frozen') return w.walk.day_frozen();
  if (path === 'driving') return w.walk.driving();
  if (path === 'raft.heading') return w.walk.raft_heading(s.alpha);
  if (/^raft\.[xz]$/.test(path)) return w.walk.raft_offset(s.alpha)[axis[path[5]]];
  if (path === 'sprinting') return w.walk.sprinting();
  if (path === 'paused') return w.walk.paused();
  if (path === 'locked') return w.input.locked;
  if (path === 'prompt') return waitingForClick(w.input) ? CUE_TEXT : null;
  if (path === 'hint') return w.hint.open ? 'open' : 'folded';
  if (path === 'lock_requests') return w.canvas.lockRequests;
  if (path === 'moved') return Math.hypot(...d);
  if (['ticks', 'alpha', 'yaw', 'pitch', 'fov', 'underwater'].includes(path)) return s[path];
  if (path === 'belt.selected') return s.belt.selected;
  if (/^belt\.slot\.[1-9]$/.test(path)) return s.belt.slots[+path.slice(-1) - 1] ?? 'none';
  if (path === 'hands') return s.belt.hands ?? 'none';
  if (path === 'held') return s.belt.held;
  if (path === 'hands.swing') return s.belt.swing;
  if (path === 'hands.use') return s.belt.use;
  if (path === 'held.drawn') return heldDraw(s.belt.held)?.key ?? 'none';
  if (path === 'health') return s.hud.hearts.full * 2 + s.hud.hearts.half;
  if (path.startsWith('hearts.')) return s.hud.hearts[path.slice(7)];
  if (path === 'phase') return s.hud.timer.phase;
  if (path === 'phase.left') return s.hud.timer.secondsLeft;
  if (path === 'hud.selected') return s.hud.selected;
  if (path === 'hud.hands') return s.hud.hands ?? 'none';
  if (path === 'hud.ready') return s.hud.ready ?? 'none';
  if (path === 'items') return w.walk.loose_items();
  if (path === 'fish') return readFish(w.walk).length;
  if (path.startsWith('fish.near')) {
    const [x, y, z, r] = path.slice(9).trim().split(/\s+/).map(Number);
    if (![x, y, z, r].every(Number.isFinite)) throw new Error('fish.near <x> <y> <z> <r>');
    return readFish(w.walk).filter((f) => Math.hypot(f.pos[0] - x, f.pos[1] - y, f.pos[2] - z) <= r).length;
  }
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

let tileset;

function layout(w, args) {
  const [vw, vh] = args.map(Number);
  if (args.length !== 2 || !(vw > 0) || !(vh > 0)) throw new Error('usage: layout <w> <h> (positive viewport size in pixels)');
  tileset ??= JSON.parse(readFileSync(new URL('../../viewer/textures.json', import.meta.url), 'utf8'));
  const viewport = { w: vw, h: vh };
  const { scale, rects } = hudLayout(w.controller.state.hud, tileset, viewport);
  const result = { scale, rects, problems: layoutChecks({ scale, rects }, viewport) };
  const where = (rs) => {
    if (!rs.length || rs.some((r) => !r || !(r.w > 0 && r.h > 0))) return 'missing';
    return rs.every((r) => r.x >= 0 && r.y >= 0 && r.x + r.w <= vw && r.y + r.h <= vh) ? 'inside' : 'outside';
  };
  w.layout = {
    scale, problems: result.problems.length, 'belt.w': rects.belt.w, 'belt.fraction': rects.belt.w / vw,
    timer: where([rects.timer]), slotnumbers: where(rects.slotNumbers), 'hands.label': where([rects.hands.label]),
  };
  return result;
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
    case 'layout': return layout(w, args);
    case 'click': return fire(canvas, 'click');
    case 'lock': document.pointerLockElement = canvas; return fire(document, 'pointerlockchange');
    case 'unlock': document.pointerLockElement = null; return fire(document, 'pointerlockchange');
    case 'respawn': return w.controller.respawn();
    case 'blur': // the browser drops the lock on focus loss
      document.pointerLockElement = null;
      fire(document, 'pointerlockchange');
      return fire(window, 'blur');
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
      if (args[0] === 'item' && args[1]) {
        const [x, y, z] = w.walk.feet();
        w.walk.spawn_item(args[1], x + 1, y + 0.2, z);
        return;
      }
      if (args[0] === 'fish' && args[1]) {
        const [x, y, z] = w.walk.feet();
        const cell = [Math.floor(x) + 2, Math.floor(y + 0.5), Math.floor(z)];
        if (w.walk.block_at(...cell) !== B.water) {
          throw new Error(`spawn fish: no water at ${cell.join(' ')}`);
        }
        if (w.walk.spawn_fish(args[1], cell[0] + 0.5, cell[1], cell[2] + 0.5, 0) < 0) throw new Error(`spawn fish: unknown species '${args[1]}'`);
        return;
      }
      if (args[0] !== 'animal' || !args[1]) throw new Error('spawn animal <kind> | spawn fish <species> | spawn item <name>');
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
      if (!['field', 'planter', 'helm', 'deck'].includes(args[0]) || !w.scene) throw new Error('stand field|planter|helm|deck (raft scene only)');
      if (args[0] === 'deck') {
        const [hx, hy, hz] = w.scene.helm;
        w.walk.teleport(hx + 0.5, hy + 1, hz + 1.5);
        w.controller.frame(0.05);
        return;
      }
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
      const miss = args[0] === 'miss';
      const done = () => (miss ? w.events.some((e) => e.startsWith('forage_missed ')) : read(w, 'events.foraged'));
      for (let tries = 0; tries < 200 && !done(); tries++) {
        fire(document, 'mousedown', { button: BUTTONS.right });
        w.controller.frame(0.1);
        fire(document, 'mouseup', { button: BUTTONS.right });
        w.controller.frame(0.1);
        // either outcome starts the 1200-tick cooldown; wait it out (65 s at 20 Hz)
        if (!done()) for (let i = 0; i < 650; i++) w.controller.frame(0.1);
      }
      if (!done()) throw new Error(`no forage ${miss ? 'missed' : 'succeeded'} in 200 tries`);
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
      const at = Math.max(1, args.findIndex((a) => OPS[a]));
      const path = args.slice(0, at).join(' '), cmp = args[at], raw = args.slice(at + 1).join(' ');
      if (!OPS[cmp]) throw new Error(`unknown operator '${cmp}'`);
      const got = read(w, path) ?? 'none'; // a null path (prompt) is written none
      const want = typeof got === 'number' ? +raw : typeof got === 'boolean' ? raw === 'true' : raw;
      if (!OPS[cmp](got, want)) throw new Error(`expected ${path} ${cmp} ${raw}, got ${got}`);
      return;
    }
    default: throw new Error(`unknown statement '${op}'`);
  }
}

const START_USAGE = 'start fly|walk [raft] [debug] [stream] [at <x> <z>]';

/** The world options for a `start` line's arguments; throws the usage message when they are bad. */
function parseStart(args) {
  if (!args.length) return {};
  const [mode, ...rest] = args;
  const i = rest.indexOf('at');
  const flags = i < 0 ? rest : rest.slice(0, i);
  const at = i < 0 ? null : rest.slice(i + 1).map(Number);
  if (at && (at.length !== 2 || !at.every(Number.isFinite) || !flags.includes('stream'))) throw new Error(START_USAGE);
  if (!['fly', 'walk'].includes(mode) || flags.some((f) => !['raft', 'debug', 'stream'].includes(f))) {
    throw new Error(START_USAGE);
  }
  const opts = { mode, raft: flags.includes('raft'), stream: flags.includes('stream') };
  if (flags.includes('debug')) opts.debug = true;
  if (at) opts.at = at;
  return opts;
}

/**
 * A live world driven one DSL line at a time. `startArgs` are the words after `start`
 * (e.g. ['walk', 'raft']); none means the default world. `exec` (a DSL line) and
 * `execStep` (a parsed `{ op, args }`) never throw: they return `{ ok: true }` or
 * `{ ok: false, error }`. `transcript()` is the session as a valid script: `case <name>`,
 * the `start` line, then every line that succeeded (errored ones are dropped).
 * Call `close()` when done. `streamBudget` (tests only) meshes that many regions per frame in a stream world;
 * `damage` (tests only) maps each meshed region's `opaque` part to the one `render.faults` checks.
 */
export function createSession(startArgs = [], name = 'session', { streamBudget = 1, damage = null } = {}) {
  const w = world({ ...parseStart(startArgs), budget: streamBudget, damage });
  let open = true;
  const lines = [`case ${name}`, `start ${(startArgs.length ? startArgs : ['walk']).join(' ')}`];
  const execStep = ({ op, args }) => {
    try {
      if (!open) throw new Error('session is closed');
      if (op === 'start') throw new Error('start is only allowed when the session is created');
      let value;
      if (op) {
        value = step(w, { op, args });
        lines.push([op, ...args].join(' '));
      }
      return op === 'layout' ? { ok: true, value } : { ok: true };
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
