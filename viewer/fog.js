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

// Far terrain (farmesh.js) fogs out at its own, much longer distance, and is
// cut away where the chunks are drawn: nearer than the chunks' fog end minus a
// margin (the margin keeps the far mesh's edge under the chunks' own fog).
export const FAR_FOG_END = 500;
export const FAR_CUT_MARGIN = 16;
export const farCutRadius = (chunkFogEnd) => chunkFogEnd - FAR_CUT_MARGIN;
