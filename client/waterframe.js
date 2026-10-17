// Water animation frame, as in viewer/index.html: four frames, advancing every
// 300 ms of frame time, and frame 0 throughout when the user prefers reduced motion.

export const WATER_FRAMES = 4;
export const WATER_FRAME_MS = 300;

/** Frame (0..3) for frame timestamp `nowMs`; always 0 when `reducedMotion`. */
export function waterFrame(nowMs, reducedMotion) {
  return reducedMotion ? 0 : Math.floor(nowMs / WATER_FRAME_MS) % WATER_FRAMES;
}
