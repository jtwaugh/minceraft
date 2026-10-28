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

// Water depth (vertex byte 15: water blocks from the face down to the floor, 0
// on anything that is not water) to a colour factor and an alpha scale. Same
// curve as the map's terrainColor: depth is capped at WATER_DEPTH_CAP, shallow
// is lighter, deep is darker and more opaque. Depth 0 and 1 give (1, 1), so
// non-water meshes and one-block water keep the plain texture look. Tuned by eye.
export const WATER_DEPTH_CAP = 24;
export const WATER_DEEP_DARKEN = 0.55; // colour factor at the cap
export const WATER_DEEP_ALPHA = 1.6; // alpha scale at the cap (the result is clamped to 1)

function depthT(depth) {
  return Math.min(Math.max((depth - 1) / (WATER_DEPTH_CAP - 1), 0), 1);
}

export function waterColorFactor(depth) {
  return 1 - (1 - WATER_DEEP_DARKEN) * depthT(depth);
}

export function waterAlphaScale(depth) {
  return 1 + (WATER_DEEP_ALPHA - 1) * depthT(depth);
}

// `depth` is the raw byte (0-255) as a float.
export const WATER_DEPTH_GLSL = `
    float waterDepthT(float depth) {
      return clamp((depth - 1.0) / ${(WATER_DEPTH_CAP - 1).toFixed(1)}, 0.0, 1.0);
    }
    float waterColorFactor(float depth) {
      return 1.0 - ${(1 - WATER_DEEP_DARKEN).toFixed(3)} * waterDepthT(depth);
    }
    float waterAlphaScale(float depth) {
      return 1.0 + ${(WATER_DEEP_ALPHA - 1).toFixed(3)} * waterDepthT(depth);
    }`;
