// Sky colour that follows the player. Pure maths, no WebGL: scene.js samples the
// biome and temperature maps around the player every few ticks and eases the sky
// colour toward the result.

export const SKY_EASE_SECONDS = 2;
export const SKY_SAMPLE_RADIUS = 8;

// An exponential ease has remaining error exp(-rate * t); 3 leaves 5% after the window.
const RATE = 3 / SKY_EASE_SECONDS;

/**
 * Mean sky colour (0-1 per channel) over a window of columns: `ids` are biome
 * indices into `biomes`, `temps` their temperatures, `ramp` is `ramp3`.
 */
export function sampleSky(ids, temps, biomes, ramp) {
  const sum = [0, 0, 0];
  for (let i = 0; i < ids.length; i++) {
    const c = ramp(biomes[ids[i]].sky, temps[i]);
    for (let k = 0; k < 3; k++) sum[k] += c[k];
  }
  return sum.map((v) => v / ids.length / 255);
}

/** One easing step of `dt` seconds from `cur` toward `target`; frame-rate independent, never overshoots. */
export function easeSky(cur, target, dt) {
  const a = 1 - Math.exp(-RATE * Math.max(dt, 0));
  return cur.map((v, k) => v + (target[k] - v) * a);
}

const smooth = (x) => x * x * (3 - 2 * x);
const mix3 = (a, b, t) => a.map((v, k) => v + (b[k] - v) * t);

/**
 * Sky base for the time of day. `base` is the biome mix (0-1 per channel),
 * `night` and `dusk` are 0-255 colours from the day data, `minDaylight` is the
 * data minimum. The night weight n runs 0 at daylight 1 to 1 at the minimum;
 * the colour goes base -> dusk over the first half and dusk -> night over the
 * second, each leg eased with smoothstep so there are no kinks at the joins.
 * Project choice: the curve is only a look, nothing in the sim reads it.
 */
export function blendSky(base, daylight, minDaylight, night, dusk) {
  const span = 1 - minDaylight;
  const n = span > 0 ? Math.min(Math.max((1 - daylight) / span, 0), 1) : 0;
  if (n <= 0) return base;
  const d = dusk.map((v) => v / 255);
  if (n <= 0.5) return mix3(base, d, smooth(2 * n));
  return mix3(d, night.map((v) => v / 255), smooth(2 * n - 1));
}
