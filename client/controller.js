// The client's per-frame sim step, apart from WebGL so it runs in Node: the
// clock turns frame time into ticks, each tick drains one sampled input into
// the walking player or the debug fly camera, and the frame's view is the
// interpolated eye plus this frame's look (docs/design/basic-gameplay.md 2.2).
// F swaps walking and flying while the pointer is locked, keeping the position.
//
//   const c = createController({ input, walk, fly, clock, document, flying });
//   c.frame(dtSeconds)  -> { ticks, alpha, eye, yaw, pitch, fov, flying, aim, belt, hud, underwater, events }
//                          (aim and belt as in overlay.js; the walking player's, stale while flying)
//   c.submitConsole(line)  queue the line on the walk, close the console, relock the pointer
//   c.closeConsole()       close the console without submitting; the pointer stays unlocked
//   state.inventory        true while the inventory view is open; the sim ticks on with neutral input
//   state.dead             true while the player is dead (walk.dead()); input is dropped and ignored
//   c.respawn()            while dead, queue the `respawn` command on the walk, leave the death mode and
//                          ask for the pointer again; does nothing while alive
//   c.detach()          remove the F listener
//
// `fov` is the vertical field of view in degrees. It starts at `sprintFov.base` (look.fov_degrees) and,
// while the walking player sprints, eases toward base + `sprintFov.extra` and back, covering `extra`
// degrees per `sprintFov.seconds` of frame time (SPEC Sprinting, Camera). Flying never widens it.
//
// `hud` is hudModel (hud.js) for the walking player, filled from the belt and health.
//
// `events` is the sim's events since the previous frame (every tick of this one),
// oldest first, as `name key=value ...` strings; each is delivered once.
//
// `particles` is the live particle list (particles.js: a burst per `forage_missed` event),
// `particleCount` its length; scene.js only draws it.
//
// `underwater` is whether the drawn eye is in a water cell, in either mode;
// scene.js switches the fog and tint on it.
//
// With `stream` ({ budget }) the walk is a Walk.streamed world: each frame the
// controller feeds the sim's load and unload events to a stream (stream.js)
// centred on the eye, and `state.regions` is { mesh, free, loaded, freed, pending,
// centre }: regions to mesh and to free this frame, how many are meshed now, how
// many have been freed so far, and how many wait for the budget. While flying the
// sim is not ticked, so the walk's feet follow the fly camera whenever it enters
// a new region; otherwise nothing would load ahead of it.
//
// client/test/scripts/*.input drive this through the DSL in client/test/dsl.mjs.

import { stepWalk } from './walk.js';
import { readAim, readBelt } from './overlay.js';
import { hudModel } from './hud.js';
import { createStream, regionOf } from './stream.js';
import { createParticles } from './particles.js';

export function createController({ input, walk, fly, clock, document, flying = false, stream = null, sprintFov = { base: 70, extra: 0, seconds: 1 } }) {
  const regions = stream && createStream(stream);
  let followed = null;
  const state = { flying, inventory: false, dead: false, ticks: 0, alpha: 0, eye: [0, 0, 0], yaw: 0, pitch: 0, fov: sprintFov.base, aim: null, belt: null, hud: null, underwater: false, events: [], particles: [], particleCount: 0 };
  const particles = createParticles();

  const onKey = (e) => {
    if (e.code !== 'KeyF' || e.repeat || !input.locked || input.console || input.inventory) return;
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

  let respawning = false; // respawn queued, the sim has not yet run it
  let freed = 0;
  const streamRegions = (eye) => {
    const centre = regionOf(eye[0], eye[2]);
    if (state.flying && (followed?.rx !== centre.rx || followed?.rz !== centre.rz)) {
      followed = centre;
      walk.teleport(eye[0], eye[1] - 1.62, eye[2]); // streams around the fly eye
    }
    const { mesh, free } = regions.frame(walk.drain_stream(), centre);
    freed += free.length;
    const remesh = regions.edited(walk.drain_edited_regions());
    return { mesh, free, remesh, loaded: regions.meshed, freed, pending: regions.pending, centre };
  };

  return {
    state,
    frame(dt) {
      if (!walk.dead()) respawning = false;
      const dead = walk.dead() && !respawning;
      if (dead !== input.dead) input.setDead(dead);
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
      if (regions) state.regions = streamRegions(eye);
      const goal = sprintFov.base + (!state.flying && walk.sprinting() ? sprintFov.extra : 0);
      const step = (sprintFov.extra * dt) / sprintFov.seconds;
      const fov = state.fov < goal ? Math.min(goal, state.fov + step) : Math.max(goal, state.fov - step);
      const underwater = walk.liquid_at(eye[0], eye[1], eye[2]);
      const belt = readBelt(walk);
      const hud = hudModel({ belt, health: walk.health(), maxHealth: walk.max_health(), phase: walk.phase(), secondsLeft: walk.phase_seconds_left() });
      const events = Array.from(walk.drain_events());
      particles.spawnMisses(events);
      particles.update(dt);
      Object.assign(state, { inventory: input.inventory, dead, ticks, alpha, eye, yaw, pitch, fov, aim: readAim(walk, alpha), belt, hud, underwater, events, particles: particles.list, particleCount: particles.count });
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
    respawn() {
      if (!input.dead || respawning) return;
      respawning = true;
      walk.submit_command('respawn');
      input.setDead(false);
      input.requestLock();
    },
    detach() {
      document.removeEventListener('keydown', onKey);
    },
  };
}
