// Half-resolution meshes for far chunks: blocks are downsampled to 2x2x2
// cells and meshed with meshBlocks, then positions are doubled so a cell is
// two blocks wide. The vertex format is meshBlocks's. A cell's tile spans the
// cell once, so UVs stay as meshBlocks writes them.

import { B, H, meshBlocks } from './mesh.js';

/**
 * Downsample `blocks` (layout of meshBlocks: (size + 2 * pad)^2 columns, 128
 * high) to cells of 2x2x2 blocks. A cell takes its most common non-air block,
 * the lowest id on a tie, and is air when all eight are. Cells tile the inner
 * area from its first column, so `size` must be even; the pad ring becomes
 * floor(pad / 2) cells (an odd pad loses its outermost column).
 * Returns { cells, size: size / 2, pad: floor(pad / 2) }; cells use the same
 * layout, filled to y = 63.
 */
export function downsampleBlocks(blocks, size, pad = 0) {
  if (size % 2) throw new Error('downsampleBlocks: size must be even');
  const w = size + 2 * pad, cp = pad >> 1, cs = size / 2, cw = cs + 2 * cp;
  const cells = new Uint8Array(cw * cw * H);
  const vals = new Uint8Array(8);
  for (let cz = 0; cz < cw; cz++) for (let cx = 0; cx < cw; cx++) {
    const x = (cx - cp) * 2 + pad, z = (cz - cp) * 2 + pad, out = (cz * cw + cx) * H;
    for (let cy = 0; cy < H / 2; cy++) {
      let n = 0;
      for (let dz = 0; dz < 2; dz++) for (let dx = 0; dx < 2; dx++) for (let dy = 0; dy < 2; dy++) {
        const b = blocks[((z + dz) * w + x + dx) * H + cy * 2 + dy];
        if (b !== B.air) vals[n++] = b;
      }
      let best = 0, bestN = 0;
      for (let i = 0; i < n; i++) {
        let c = 0;
        for (let j = 0; j < n; j++) if (vals[j] === vals[i]) c++;
        if (c > bestN || (c === bestN && vals[i] < best)) { best = vals[i]; bestN = c; }
      }
      cells[out + cy] = best;
    }
  }
  return { cells, size: cs, pad: cp };
}

/**
 * Mesh a region at half resolution: same arguments and result as meshBlocks
 * (`tints` for the inner size x size columns; the vertex tint is the one of
 * each cell's first column, with no corner blend). `origin` is the world
 * (x, z) of the first column and must be even, so cells line up with those of
 * neighbouring areas. Leaves are meshed as fast leaves. Positions, and meanY
 * and minY, are in blocks. `mesher` is meshBlocks or a function with its
 * arguments and result (the wasm one, meshwasm.js).
 */
export function meshHalf(ctx, blocks, tints, size, pad = 0, { origin = [0, 0], greedy = true, mesher = meshBlocks } = {}) {
  const { cells, size: cs, pad: cp } = downsampleBlocks(blocks, size, pad);
  const pick = (a) => a && Array.from({ length: cs * cs }, (_, c) => a[(Math.floor(c / cs) * 2) * size + (c % cs) * 2]);
  const ct = { grass: pick(tints.grass), leaf: pick(tints.leaf), water: pick(tints.water), dry: pick(tints.dry) };
  const m = mesher(ctx, cells, ct, cs, cp, { fast: true, origin: [origin[0] >> 1, origin[1] >> 1], greedy });
  for (const k of ['opaque', 'cutout', 'water', 'ice']) {
    const i16 = new Int16Array(m[k].verts);
    for (let v = 0; v < m[k].faces * 4; v++) for (let j = 0; j < 3; j++) i16[v * 8 + j] *= 2;
  }
  m.meanY *= 2; m.minY *= 2;
  return m;
}
