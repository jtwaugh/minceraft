// Sky gradient, shared by the shader (GLSL below) and the unit test (skyColor).
// Given the biome's base sky colour (the per-biome ramp output, RGB 0..1), a unit
// view direction and a unit sun direction: the base colour at the horizon, the
// base colour * ZENITH_SCALE straight up, mixed toward SKY_WARM by up to
// SUN_TINT_MAX within SUN_TINT_ANGLE degrees of the sun. (Decided in #620.)
// Under water the sky pass is skipped, so nothing here handles that case.
export const ZENITH_SCALE = 0.85;
export const SUN_TINT_MAX = 0.25;
export const SUN_TINT_ANGLE = 40; // degrees
export const SKY_WARM = [1.0, 0.8, 0.55];

const COS_TINT = Math.cos((SUN_TINT_ANGLE * Math.PI) / 180);

function smoothstep(a, b, x) {
  const t = Math.min(Math.max((x - a) / (b - a), 0), 1);
  return t * t * (3 - 2 * t);
}

export function skyColor(base, dir, sunDir) {
  const up = Math.min(Math.max(dir[1], 0), 1);
  const tint =
    SUN_TINT_MAX * smoothstep(COS_TINT, 1, dir[0] * sunDir[0] + dir[1] * sunDir[1] + dir[2] * sunDir[2]);
  return base.map((c, i) => {
    const gradient = c * (1 - up * (1 - ZENITH_SCALE));
    return gradient + (SKY_WARM[i] - gradient) * tint;
  });
}

// Same formula for the fragment shader; `dir` and `sunDir` are unit vectors.
export const SKY_GLSL = `
    vec3 skyColor(vec3 base, vec3 dir, vec3 sunDir) {
      vec3 c = base * mix(1.0, ${ZENITH_SCALE.toFixed(2)}, clamp(dir.y, 0.0, 1.0));
      float tint = ${SUN_TINT_MAX.toFixed(2)} * smoothstep(${COS_TINT.toFixed(6)}, 1.0, dot(dir, sunDir));
      return mix(c, vec3(${SKY_WARM.map((v) => v.toFixed(2)).join(', ')}), tint);
    }`;
