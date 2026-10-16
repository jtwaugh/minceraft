// Fog curve, shared by the shader (GLSL below) and the unit test (fogAmount).
// Fog depends on the straight-line distance from the eye: 0 at FOG_NEAR * far,
// smoothstep up to exactly 1 at `far`.
export const FOG_NEAR = 0.5;

export function fogAmount(dist, far) {
  const t = Math.min(Math.max((dist - far * FOG_NEAR) / (far * (1 - FOG_NEAR)), 0), 1);
  return t * t * (3 - 2 * t);
}

// Same curve for the fragment shader; `far` is the distance of solid fog.
export const FOG_GLSL = `
    float fogAmount(float dist, float far) {
      return smoothstep(far * ${FOG_NEAR.toFixed(2)}, far, dist);
    }`;
