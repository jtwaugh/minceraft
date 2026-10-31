// Per-column canopy shade for the mesher: how much of the ground around a
// column lies under leaves. A tint input for grass under trees
// (docs/design/shaded-grass-under-trees.md), computed from the meshed blocks.
// It is deliberately not the generator's `under_canopy` (crates/terrain/src/flora.rs),
// which is binary and decided at generation time; this is a fraction over a window.

import { B, H, LEAVES } from './mesh.js';

/**
 * Shade fraction in [0, 1] for each of the size x size inner columns (row
 * z * size + x). `blocks` holds (size + 2 * pad)^2 full-height columns, laid
 * out ((z * w) + x) * H + y with w = size + 2 * pad.
 *
 * A column is shaded when, scanning down from the top, a leaf block lies above
 * its first grass block; columns without grass are not shaded. The fraction is
 * the shaded count over the (2r + 1)^2 window divided by the window's column
 * count, both integers, so it depends only on the window's blocks. The window
 * is clipped at the edge of the padded array.
 */
export function shadeFractions(blocks, size, pad, r) {
  const w = size + 2 * pad;
  // Prefix sums of the shaded flags: sum[(z * (w + 1)) + x] counts columns < z, < x.
  const sum = new Int32Array((w + 1) * (w + 1));
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const base = (z * w + x) * H;
    let leaf = false, flag = 0;
    for (let y = H - 1; y >= 0; y--) {
      const b = blocks[base + y];
      if (b === B.grass) { flag = leaf ? 1 : 0; break; }
      if (LEAVES[b]) leaf = true;
    }
    const i = (z + 1) * (w + 1) + x + 1;
    sum[i] = flag + sum[i - 1] + sum[i - w - 1] - sum[i - w - 2];
  }
  const out = new Float64Array(size * size);
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    const z0 = Math.max(0, z + pad - r), z1 = Math.min(w, z + pad + r + 1);
    const x0 = Math.max(0, x + pad - r), x1 = Math.min(w, x + pad + r + 1);
    const count = sum[z1 * (w + 1) + x1] - sum[z0 * (w + 1) + x1] - sum[z1 * (w + 1) + x0] + sum[z0 * (w + 1) + x0];
    out[z * size + x] = count / ((z1 - z0) * (x1 - x0));
  }
  return out;
}
