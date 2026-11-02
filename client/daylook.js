// The sky's clock as the client sees it. Pure maths, no WebGL: the sim reports
// the time of day at the latest tick, and the frame adds the step's alpha so the
// sun moves smoothly between ticks (docs/design/day-night-cycle.md, Precision).

/**
 * Time of day (0..1) a frame shows: the sim's time plus `alpha` of one tick,
 * wrapped. A frozen clock does not advance, so alpha adds nothing.
 */
export function interpolatedTime(time, alpha, dayTicks, frozen) {
  if (frozen) return time;
  const t = time + alpha / dayTicks;
  return t - Math.floor(t);
}

/** `{ time, sunDir, daylight }` for this frame, from the wasm walk's day exports. */
export function readDay(walk, alpha) {
  const time = interpolatedTime(walk.time_of_day(), alpha, walk.day_ticks(), walk.day_frozen());
  return { time, sunDir: Array.from(walk.sun_dir_at(time)), daylight: walk.daylight_at(time) };
}
