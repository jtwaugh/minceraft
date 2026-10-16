// Distance fade for cross plants. Pure maths, mirrored by the cutout vertex
// shader in scene.js: beyond `start` blocks from the eye the plant's top
// vertices (mesh byte 14) sink toward its base, so it is gone
// `PLANT_FADE_RANGE` blocks later.

export const PLANT_FADE_RANGE = 8;

/** Height scale of a plant `dist` blocks away: 1 up to `start`, 0 from start + range. */
export function plantScale(dist, start = 48, range = PLANT_FADE_RANGE) {
  return 1 - Math.min(Math.max((dist - start) / range, 0), 1);
}
