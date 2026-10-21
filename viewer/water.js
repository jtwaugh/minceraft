// Water Fresnel, shared by the shader (GLSL below) and the unit test (fresnel).
// The sea surface is a horizontal plane, so the view angle is just abs(dir.y)
// of the unit eye-to-fragment direction. This is an approximation: lakes and
// rivers at other heights are still horizontal, so it holds for them, but
// vertical water faces get the term from the eye direction rather than their
// own normal. Schlick's approximation: 1 at grazing
// angles (full sky colour), FRESNEL_F0 looking straight down. The sky colour is
// taken in the reflected direction (dir.y flipped). Not applied under water.
export const FRESNEL_F0 = 0.02;

export function fresnel(dirY) {
  const c = Math.min(Math.abs(dirY), 1);
  return FRESNEL_F0 + (1 - FRESNEL_F0) * (1 - c) ** 5;
}

// `dir` is the unit eye-to-fragment direction.
export const WATER_GLSL = `
    float fresnel(vec3 dir) {
      float c = min(abs(dir.y), 1.0);
      float m = 1.0 - c;
      float m2 = m * m;
      return ${FRESNEL_F0.toFixed(2)} + ${(1 - FRESNEL_F0).toFixed(2)} * m2 * m2 * m;
    }`;
