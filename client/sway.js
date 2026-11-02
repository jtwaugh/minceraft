// Wind sway for cross plants. Pure maths, mirrored by the cutout vertex
// shader in scene.js: a vertex with weight `top` (mesh byte 14, 0..1 once
// normalised) moves sideways by a small sine of time and world position, so a
// plant's base (weight 0) stays planted and its top moves most.

export const SWAY_AMPLITUDE = 0.08; // blocks, at weight 1
export const SWAY_SPEED = 1.6; // radians per second
export const SWAY_WAVE = 0.45; // radians per block
export const TRUNK_SWAY_AMPLITUDE = SWAY_AMPLITUDE / 4; // blocks, trunk lean at weight 1

/** Wave phase (radians, wrapped to one turn) for `nowMs` of frame time; 0 when `reducedMotion`. */
export function swayPhase(nowMs, reducedMotion) {
  return reducedMotion ? 0 : (nowMs / 1000 * SWAY_SPEED) % (2 * Math.PI);
}

/** Wind input `{ dir: [x, z], strength }`: `dir` is a unit vector ([0, 0] for none), `strength` 0..1. */
// Until the wind is queried per frame, callers pass this: the old fixed direction at full strength.
export const FIXED_WIND = { dir: [0.857493, 0.514496], strength: 1 };

/** Share of the amplitude that remains at strength 0; strength 1 gives the whole amplitude. */
export const SWAY_FLOOR = 0.1;

/** [dx, dz] offset in blocks of a vertex at world (x, z) with weight 0..1 at wave `phase`, leaning along `wind.dir`. */
export function swayOffset(weight, x, z, phase, wind, amplitude = SWAY_AMPLITUDE) {
  const [dx, dz] = wind.dir;
  const scale = SWAY_FLOOR + (1 - SWAY_FLOOR) * wind.strength;
  const a = amplitude * weight * scale * Math.sin(phase + (x * dx + z * dz) * SWAY_WAVE);
  return [a * dx, a * dz];
}

/** [dx, dz] trunk lean in blocks of a log vertex at world (x, z) with weight 0..1: the plant wave, smaller. */
export function trunkLean(weight, x, z, phase, wind) {
  return swayOffset(weight, x, z, phase, wind, TRUNK_SWAY_AMPLITUDE);
}
