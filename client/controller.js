// The client's per-frame sim step, apart from WebGL so it runs in Node: the
// clock turns frame time into ticks, each tick drains one sampled input into
// the walking player or the debug fly camera, and the frame's view is the
// interpolated eye plus this frame's look (docs/design/basic-gameplay.md 2.2).
// F swaps walking and flying while the pointer is locked, keeping the position.
//
//   const c = createController({ input, walk, fly, clock, document, flying });
//   c.frame(dtSeconds)  -> { ticks, alpha, eye, yaw, pitch, flying, aim, belt, underwater }
//                          (aim and belt as in overlay.js; the walking player's, stale while flying)
//   c.submitConsole(line)  queue the line on the walk, close the console, relock the pointer
//   c.closeConsole()       close the console without submitting; the pointer stays unlocked
//   c.detach()          remove the F listener
//
// `underwater` is whether the drawn eye is in a water cell, in either mode;
// scene.js switches the fog and tint on it.
//
// client/test/scripts/*.input drive this through the DSL in client/test/dsl.mjs.

import { stepWalk } from './walk.js';
import { readAim, readBelt } from './overlay.js';

export function createController({ input, walk, fly, clock, document, flying = false }) {
  const state = { flying, ticks: 0, alpha: 0, eye: [0, 0, 0], yaw: 0, pitch: 0, aim: null, belt: null, underwater: false };

  const onKey = (e) => {
    if (e.code !== 'KeyF' || e.repeat || !input.locked || input.console) return;
    const eyeNow = walk.eye(1), feet = walk.feet();
    if (state.flying) {
      const [x, y, z] = fly.eye(1);
      walk.teleport(x, y - (eyeNow[1] - feet[1]), z);
    } else {
      fly.teleport(eyeNow[0], eyeNow[1], eyeNow[2]);
    }
    state.flying = !state.flying;
  };
  document.addEventListener('keydown', onKey);

  return {
    state,
    frame(dt) {
      const ticks = clock.advance(dt);
      const alpha = clock.alpha;
      const { yaw, pitch } = input.look();
      let eye; // input.drain() holds no keys while the pointer is unlocked
      if (state.flying) {
        for (let i = 0; i < ticks; i++) {
          const t = input.drain();
          fly.tick(t);
          t.free();
        }
        eye = Array.from(fly.eye(alpha));
      } else {
        eye = stepWalk(walk, ticks, alpha, () => input.drain());
      }
      const underwater = walk.liquid_at(eye[0], eye[1], eye[2]);
      Object.assign(state, { ticks, alpha, eye, yaw, pitch, aim: readAim(walk, alpha), belt: readBelt(walk), underwater });
      return state;
    },
    submitConsole(line) {
      if (!input.console) return;
      walk.submit_command(line);
      input.closeConsole({ submitted: true });
    },
    closeConsole() {
      input.closeConsole();
    },
    detach() {
      document.removeEventListener('keydown', onKey);
    },
  };
}
