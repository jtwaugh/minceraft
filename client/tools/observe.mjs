// What a player perceives versus what the test harness can read
// (docs/design/play-critic-agent.md section 2). One table classifies every path
// `read()` in client/test/dsl.mjs serves, so the player-facing tool and the debug
// view can't drift apart.
//
//   player   drawn by the HUD or the screen as-is
//   partial  visible only in part: target.id, target.face and box show as the outline,
//            console.* only while the console is open, phase.left as digits
//   debug    internal state a player can't see
//
//   PATHS                       path -> 'player' | 'partial' | 'debug' (`eye.x|y|z`-style groups spelled out)
//   playerObservation(session)  plain data of what a player perceives
//   debugObservation(session)   every debug path that the session's world can serve
//
// `session` is a createSession() result; only `session.read` is used.

import { digitNames } from '../hud.js';

const BELT_SLOTS = [1, 2, 3, 4, 5];
const axes = (head) => ['x', 'y', 'z'].map((a) => `${head}.${a}`);

const CLASSES = {
  player: [
    'mode', 'locked', 'target.kind', 'belt.selected', ...BELT_SLOTS.map((n) => `belt.slot.${n}`),
    'hands', 'health', 'hearts.full', 'hearts.half', 'hearts.empty', 'hud.selected', 'hud.hands', 'phase',
  ],
  partial: ['target.id', 'target.face', 'box', 'console.last', 'console.text', 'phase.left'],
  debug: [
    'regions.loaded', 'regions.freed', 'regions.pending', 'regions.x', 'regions.z',
    'console.lines', 'cheated', 'ticks', 'alpha', 'yaw', 'pitch', ...axes('eye'),
    'moved', ...axes('moved'), 'lock_requests', 'rule.primary', 'rule.secondary', 'rule.last',
    'underwater', 'held', 'items', 'field.ground', 'field.crop', 'flower.block',
    'planter.ground', 'planter.crop', 'events', 'events.foraged', 'sim_tick', 'paused', 'sprinting', 'fov',
    'animals', 'animals.dying', 'animals.hurt',
  ],
};

export const PATHS = Object.freeze(
  Object.fromEntries(Object.entries(CLASSES).flatMap(([kind, paths]) => paths.map((p) => [p, kind]))),
);

/** Everything a player perceives, as plain data. */
export function playerObservation(session) {
  const r = session.read;
  const mode = r('mode');
  const secondsLeft = r('phase.left');
  const target = { kind: r('target.kind'), outlined: r('box') };
  return {
    mode,
    locked: r('locked'),
    hud: {
      hearts: { full: r('hearts.full'), half: r('hearts.half'), empty: r('hearts.empty') },
      belt: { selected: r('hud.selected'), slots: BELT_SLOTS.map((n) => r(`belt.slot.${n}`)) },
      hands: r('hud.hands'),
      timer: { phase: r('phase'), digits: digitNames(secondsLeft).map((d) => d.slice(-1)).join('') },
    },
    target,
    console: mode === 'console' ? { text: r('console.text'), last: r('console.last') } : null,
  };
}

/** Every debug-classified path the session can serve, by path. Paths this world lacks are left out. */
export function debugObservation(session) {
  const out = {};
  for (const path of CLASSES.debug) {
    try { out[path] = session.read(path); } catch { /* needs a scene or a streamed world */ }
  }
  return out;
}
