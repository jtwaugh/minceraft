// Free-look fly camera: the sim side (fixed 20 Hz steps over TickInput) and
// the render side (interpolating the last two ticks). See
// docs/design/basic-gameplay.md sections 1 rule 1, 2.2 and 3.
//
// Debug camera only: no body, collision or gravity (those are Phase 2).
// W/S move along the look direction, A/D strafe, Space rises, Shift sinks.

export const TICK_RATE = 20;
export const TICK_MS = 1000 / TICK_RATE;
export const FLY_SPEED = 12; // blocks per second
const MAX_TICKS_PER_FRAME = 5; // after a stall, drop time instead of spiralling

/** Look vectors for yaw/pitch: yaw 0 faces +z, turning right increases yaw, pitch up is positive. */
export function lookVectors(yaw, pitch) {
  const [sy, cy, sp, cp] = [Math.sin(yaw), Math.cos(yaw), Math.sin(pitch), Math.cos(pitch)];
  return {
    forward: [-sy * cp, sp, cy * cp],
    right: [-cy, 0, -sy],
  };
}

/** One 20 Hz step: the position after applying a TickInput. Pure. */
export function stepFly(pos, input, speed = FLY_SPEED) {
  const { forward, right } = lookVectors(input.yaw, input.pitch);
  const up = (input.jump ? 1 : 0) - (input.sneak ? 1 : 0);
  const s = speed / TICK_RATE;
  return [0, 1, 2].map((i) =>
    pos[i] + s * (forward[i] * input.move_forward + right[i] * input.move_strafe + (i === 1 ? up : 0)));
}

/** Position between the last two ticks, a in 0..1. */
export function interpolate(prev, cur, a) {
  return prev.map((p, i) => p + (cur[i] - p) * a);
}

/** Turns wall-clock time into whole sim ticks plus the render fraction. */
export class TickClock {
  constructor() {
    this.last = null;
    this.acc = 0;
  }

  /** Call once per frame with a millisecond clock; returns { ticks, alpha }. */
  advance(now) {
    if (this.last !== null) this.acc += now - this.last;
    this.last = now;
    let ticks = Math.floor(this.acc / TICK_MS);
    this.acc -= ticks * TICK_MS;
    if (ticks > MAX_TICKS_PER_FRAME) ticks = MAX_TICKS_PER_FRAME;
    return { ticks, alpha: this.acc / TICK_MS };
  }
}
