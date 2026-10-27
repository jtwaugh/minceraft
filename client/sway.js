// Wind sway for cross plants. Pure maths, mirrored by the cutout vertex
// shader in scene.js: a vertex with weight `top` (mesh byte 14, 0..1 once
// normalised) moves sideways by a small sine of time and world position, so a
// plant's base (weight 0) stays planted and its top moves most.

export const SWAY_AMPLITUDE = 0.08; // blocks, at weight 1
export const SWAY_SPEED = 1.6; // radians per second
export const SWAY_WAVE = 0.45; // radians per block

/** Wave phase (radians, wrapped to one turn) for `nowMs` of frame time; 0 when `reducedMotion`. */
export function swayPhase(nowMs, reducedMotion) {
  return reducedMotion ? 0 : (nowMs / 1000 * SWAY_SPEED) % (2 * Math.PI);
}

/** [dx, dz] offset in blocks of a vertex at world (x, z) with weight 0..1 at wave `phase`. */
export function swayOffset(weight, x, z, phase, amplitude = SWAY_AMPLITUDE) {
  const a = amplitude * weight;
  return [a * Math.sin(phase + (x + 0.6 * z) * SWAY_WAVE), a * Math.cos(phase + (z - 0.6 * x) * SWAY_WAVE)];
}
