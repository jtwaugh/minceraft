// Sampler parameters per draw pass, as plain data so they can be tested without
// WebGL. Names are WebGL constant names; scene.js resolves them against `gl`.
//
// The tile array's own wrap mode is not relied on: opaque and water faces
// repeat (greedy-merged quads tile the texture), cutout faces clamp so a plant
// or leaf edge never wraps in the opposite texel row, and cutout also stops at
// the 4x4 mip so far-away alpha doesn't blur out to nothing.

export const CUTOUT_MAX_LOD = 2;
export const MAX_ANISOTROPY = 8;

/**
 * Integer sampler parameters for `pass` ('opaque' | 'water' | 'cutout'), as
 * [pname, value] pairs of constant names.
 */
export function samplerParams(pass) {
  const cutout = pass === 'cutout';
  const wrap = cutout ? 'CLAMP_TO_EDGE' : 'REPEAT';
  return [
    ['TEXTURE_MAG_FILTER', 'NEAREST'],
    ['TEXTURE_MIN_FILTER', 'LINEAR_MIPMAP_LINEAR'],
    ['TEXTURE_WRAP_S', wrap],
    ['TEXTURE_WRAP_T', wrap],
  ];
}

/** Max LOD clamp for `pass`, or null to leave the default (no clamp). */
export function maxLod(pass) {
  return pass === 'cutout' ? CUTOUT_MAX_LOD : null;
}

/** Creates the sampler for `pass` on a WebGL2 context. */
export function createSampler(gl, pass) {
  const s = gl.createSampler();
  for (const [p, v] of samplerParams(pass)) gl.samplerParameteri(s, gl[p], gl[v]);
  const lod = maxLod(pass);
  if (lod !== null) gl.samplerParameterf(s, gl.TEXTURE_MAX_LOD, lod);
  const aniso = gl.getExtension('EXT_texture_filter_anisotropic');
  if (aniso) {
    const max = gl.getParameter(aniso.MAX_TEXTURE_MAX_ANISOTROPY_EXT);
    gl.samplerParameterf(s, aniso.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(MAX_ANISOTROPY, max));
  }
  return s;
}
