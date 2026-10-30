// Texel snapping for the block shader (GLSL below), with the same arithmetic in
// JS (snapTexel) for the unit test. `px` is a texture coordinate in texels and
// `fw` its screen-space width (fwidth). Outside a band `fw` wide around each
// texel boundary the result is the texel centre; inside it the result ramps
// across the boundary, so minified faces stay smooth. The ramp is centred on
// the boundary, so the snapped texel is always floor(px): a face's far edge
// (px just below a tile boundary) never reaches the next tile, which under
// REPEAT would be the opposite edge of the same tile. Inside a greedy-merged
// quad px passes the boundary itself and so reaches the next block's texel 0.

export function snapTexel(px, fw) {
  const seam = Math.floor(px + 0.5);
  return seam + Math.min(Math.max((px - seam) / Math.max(fw, 1e-4), -0.5), 0.5);
}

export const SNAP_GLSL = `
    vec2 snapTexel(vec2 px, vec2 fw) {
      vec2 seam = floor(px + 0.5);
      return seam + clamp((px - seam) / max(fw, vec2(1e-4)), -0.5, 0.5);
    }`;
