// Mip chain for one square RGBA8 layer, in place of gl.generateMipmap (see
// docs/design/viewer-rendering.md, F4). Colour is averaged weighted by alpha,
// so transparent texels add no black, and each level's alpha is scaled so the
// share of texels at or above 0.5 (the cutout threshold) matches level 0.

const CUTOFF = 0.5;

// Returns an array of Uint8Array levels, level 0 first (a copy of `rgba`),
// down to 1x1. `size` must be a power of two. Input is straight
// (non-premultiplied) RGBA. With `premultiply`, every level's colour is
// multiplied by that level's final alpha, so colour / alpha is the same
// alpha-weighted mean as without it.
export function buildMips(rgba, size, { premultiply = false } = {}) {
  if (!(size >= 1) || (size & (size - 1)) !== 0) throw new Error('size must be a power of two');
  if (rgba.length !== size * size * 4) throw new Error('rgba length does not match size');

  let cover = 0;
  for (let i = 3; i < rgba.length; i += 4) if (rgba[i] >= CUTOFF * 255) cover++;
  const frac = cover / (size * size);

  // Working levels in float: straight colour 0..255 and alpha 0..1.
  let col = new Float32Array(size * size * 3);
  let alp = new Float32Array(size * size);
  for (let i = 0; i < size * size; i++) {
    col[i * 3] = rgba[i * 4];
    col[i * 3 + 1] = rgba[i * 4 + 1];
    col[i * 3 + 2] = rgba[i * 4 + 2];
    alp[i] = rgba[i * 4 + 3] / 255;
  }
  const levels = [pack(col, alp, premultiply)];

  for (let n = size; n > 1; ) {
    const m = n >> 1;
    const ncol = new Float32Array(m * m * 3);
    const nalp = new Float32Array(m * m);
    for (let y = 0; y < m; y++) {
      for (let x = 0; x < m; x++) {
        let a = 0;
        let r = 0;
        let g = 0;
        let b = 0;
        for (let k = 0; k < 4; k++) {
          const i = (2 * y + (k >> 1)) * n + 2 * x + (k & 1);
          const w = alp[i];
          a += w;
          r += col[i * 3] * w;
          g += col[i * 3 + 1] * w;
          b += col[i * 3 + 2] * w;
        }
        const o = y * m + x;
        if (a > 0) {
          ncol[o * 3] = r / a;
          ncol[o * 3 + 1] = g / a;
          ncol[o * 3 + 2] = b / a;
        }
        nalp[o] = a / 4;
      }
    }
    scaleCoverage(nalp, frac);
    col = ncol;
    alp = nalp;
    n = m;
    levels.push(pack(col, alp, premultiply));
  }
  return levels;
}

// Adjust `alp` (0..1) so that round(frac * length) texels reach CUTOFF and
// the rest stay below it. Alpha is scaled up when too few texels reach it;
// ties are broken by position, and texels that would pass CUTOFF beyond the
// wanted count are capped just under it.
function scaleCoverage(alp, frac) {
  const want = Math.round(frac * alp.length);
  const order = Array.from(alp.keys()).sort((p, q) => alp[q] - alp[p] || p - q);
  const edge = want > 0 ? alp[order[want - 1]] : 0;
  const s = edge > 0 && edge < CUTOFF ? CUTOFF / edge : 1;
  const below = CUTOFF * 0.999;
  order.forEach((i, rank) => {
    const a = Math.min(alp[i] * s, 1);
    alp[i] = rank < want ? Math.max(a, CUTOFF) : Math.min(a, below);
  });
}

function pack(col, alp, premultiply) {
  const out = new Uint8Array(alp.length * 4);
  for (let i = 0; i < alp.length; i++) {
    const f = premultiply ? alp[i] : 1;
    out[i * 4] = Math.round(col[i * 3] * f);
    out[i * 4 + 1] = Math.round(col[i * 3 + 1] * f);
    out[i * 4 + 2] = Math.round(col[i * 3 + 2] * f);
    out[i * 4 + 3] = Math.round(alp[i] * 255);
  }
  return out;
}

// Mip chain for a single-channel layer (for example a tint mask): plain box
// average, level 0 first, down to 1x1. Returns Uint8Array levels.
export function buildMips1(data, size) {
  if (!(size >= 1) || (size & (size - 1)) !== 0) throw new Error('size must be a power of two');
  if (data.length !== size * size) throw new Error('data length does not match size');
  const levels = [Uint8Array.from(data)];
  let prev = Float32Array.from(data);
  for (let n = size; n > 1; ) {
    const m = n >> 1;
    const next = new Float32Array(m * m);
    for (let y = 0; y < m; y++) {
      for (let x = 0; x < m; x++) {
        const i = 2 * y * n + 2 * x;
        next[y * m + x] = (prev[i] + prev[i + 1] + prev[i + n] + prev[i + n + 1]) / 4;
      }
    }
    levels.push(Uint8Array.from(next, Math.round));
    prev = next;
    n = m;
  }
  return levels;
}
