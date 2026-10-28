// Far-terrain mesh from a coarse survey grid (the `grid` shape of
// flyover/plan.js: x0, z0, step, nx, nz, top, biome). Pure data, no DOM/GL.

export const FAR_SEA = 63;   // flat plane used for water cells
export const SKIRT_DEPTH = 16; // how far the skirt bottom sits below the lowest surface vertex

/**
 * Colour [r, g, b] (0-255, clamped) of a survey sample from its biome and top
 * alone, by the rule of the page's draft map. `palette` is supplied by the
 * caller: { sand, snow, grass: [r, g, b], desert: biome id, snowy: Set of ids }.
 */
export function farColor(top, biome, palette) {
  let r, g, b;
  if (top < 64) {
    const depth = Math.min(FAR_SEA - top, 24) / 24;
    r = 20 + 30 * (1 - depth); g = 55 + 60 * (1 - depth); b = 140 + 70 * (1 - depth);
  } else {
    const c = biome === palette.desert ? palette.sand : palette.snowy.has(biome) ? palette.snow : palette.grass;
    const s = 0.72 + (top - 40) / 150;
    r = c[0] * s; g = c[1] * s; b = c[2] * s;
  }
  return [r, g, b].map((v) => Math.max(0, Math.min(255, Math.round(v))));
}

/**
 * One vertex per sample, two triangles per cell, plus a skirt around the border.
 * Positions are world coordinates minus `origin` ({x, y, z}, default 0).
 * Returns { positions: Float32Array, indices: Uint32Array, biome: Uint8Array,
 * top: Uint8Array (per vertex, survey values), color: Uint8Array (rgb per
 * vertex, only when `palette` is given; see farColor), surfaceQuads, skirtQuads, skirtBottom }.
 */
export function farMesh(g, origin = { x: 0, y: 0, z: 0 }, palette = null) {
  const { nx, nz, step } = g;
  const ox = origin.x || 0, oy = origin.y || 0, oz = origin.z || 0;
  const n = nx * nz;
  const height = (c) => Math.max(FAR_SEA, g.top[c]);
  let lo = Infinity;
  for (let c = 0; c < n; c++) lo = Math.min(lo, height(c));
  const skirtBottom = lo - SKIRT_DEPTH;

  // Border vertices clockwise seen from above: north, east, south, west.
  const ring = [];
  for (let i = 0; i < nx - 1; i++) ring.push(i);
  for (let k = 0; k < nz - 1; k++) ring.push(k * nx + nx - 1);
  for (let i = nx - 1; i > 0; i--) ring.push((nz - 1) * nx + i);
  for (let k = nz - 1; k > 0; k--) ring.push(k * nx);

  const surfaceQuads = Math.max(nx - 1, 0) * Math.max(nz - 1, 0);
  const skirtQuads = surfaceQuads ? ring.length : 0;
  const nv = n + (skirtQuads ? ring.length : 0);
  const positions = new Float32Array(nv * 3);
  const biome = new Uint8Array(nv), top = new Uint8Array(nv);
  const color = palette ? new Uint8Array(nv * 3) : null;
  for (let c = 0; c < n; c++) {
    positions[c * 3] = g.x0 + (c % nx) * step - ox;
    positions[c * 3 + 1] = height(c) - oy;
    positions[c * 3 + 2] = g.z0 + Math.floor(c / nx) * step - oz;
    biome[c] = g.biome[c]; top[c] = g.top[c];
    if (color) color.set(farColor(top[c], biome[c], palette), c * 3);
  }
  const indices = new Uint32Array((surfaceQuads + skirtQuads) * 6);
  let p = 0;
  for (let k = 0; k + 1 < nz; k++) {
    for (let i = 0; i + 1 < nx; i++) {
      const a = k * nx + i, b = a + 1, c = a + nx, d = c + 1;
      indices.set([a, c, b, b, c, d], p); p += 6;
    }
  }
  if (skirtQuads) {
    const m = ring.length;
    for (let r = 0; r < m; r++) {
      const v = n + r, s = ring[r];
      positions[v * 3] = positions[s * 3];
      positions[v * 3 + 1] = skirtBottom - oy;
      positions[v * 3 + 2] = positions[s * 3 + 2];
      biome[v] = biome[s]; top[v] = top[s];
      if (color) color.copyWithin(v * 3, s * 3, s * 3 + 3);
    }
    for (let r = 0; r < m; r++) {
      const q = (r + 1) % m;
      const a = ring[r], b = ring[q], aBot = n + r, bBot = n + q;
      indices.set([a, b, bBot, a, bBot, aBot], p); p += 6;
    }
  }
  return { positions, indices, biome, top, color, surfaceQuads, skirtQuads, skirtBottom: skirtBottom - oy };
}
