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
