// Per-frame wind for the sway shader (docs/design/wind.md Phase B). GL-free:
// one wasm Wind query per frame at the player, plus a visual phase
// accumulator. The accumulator is not deterministic and is not part of the sim.

export const PHASE_RATE_MIN = 0.25; // fraction of SWAY_SPEED at zero wind
export const MAX_FRAME_DT = 0.25; // seconds; longer frames (tab switches) are clamped

const TURN = 2 * Math.PI;

/**
 * `wind` is a wasm `Wind`; `Wind` is its class (for `direction`); `swaySpeed`
 * is radians per second at full strength.
 */
export function createWindState(wind, Wind, swaySpeed) {
  let phase = 0;
  return {
    /**
     * Advance one frame at world (x, z), sim `tick + alpha`, `dt` seconds.
     * Returns { dir: [x, z] unit (zero when calm), strength 0..1, phase
     * radians in [0, 2pi), sway amplitude scale 0..1 }. Under
     * `reducedMotion` phase and sway are 0; dir and strength stay real.
     */
    frame(x, z, tick, alpha, dt, reducedMotion) {
      const [wx, wz] = wind.at(x, z, tick + alpha);
      const strength = wind.strength(wx, wz);
      const dir = [...Wind.direction(wx, wz)];
      if (reducedMotion) {
        phase = 0;
        return { dir, strength, phase: 0, sway: 0 };
      }
      const step = Math.min(Math.max(dt, 0), MAX_FRAME_DT);
      const rate = swaySpeed * (PHASE_RATE_MIN + (1 - PHASE_RATE_MIN) * strength);
      phase = (phase + step * rate) % TURN;
      return { dir, strength, phase, sway: strength };
    },
  };
}
