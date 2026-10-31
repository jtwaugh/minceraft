// A region's blocks with a one-column border from its neighbours, so the
// mesher sees across region borders and draws no faces that a neighbour hides.
// Region blocks are 128 x 128 columns with no border (Walk.region_blocks),
// laid out x then z then y like every block array here. Each neighbour gives
// only the strip that faces the region (Walk.region_border): its edge
// columns, or one column at a corner, laid out the same way.
//
//   padRegion((rx, rz) => Walk.region_blocks(rx, rz),
//             (rx, rz, dx, dz) => Walk.region_border(rx, rz, dx, dz), rx, rz, 128)  -> 130 x 130 columns
//
// A neighbour that is not loaded (`border` returns empty) is read as the
// region's own nearest edge column, so no wall shows at the edge of the loaded world.

import { H } from '../viewer/mesh.js';

export function padRegion(get, border, rx, rz, size) {
  const own = get(rx, rz);
  const w = size + 2;
  const out = new Uint8Array(w * w * H);
  const cache = new Map();
  const strip = (dx, dz) => {
    const k = `${dx},${dz}`;
    if (!cache.has(k)) {
      const b = border(rx + dx, rz + dz, dx, dz);
      cache.set(k, b.length ? b : null);
    }
    return cache.get(k);
  };
  for (let z = -1; z <= size; z++) {
    const dz = z < 0 ? -1 : z >= size ? 1 : 0;
    for (let x = -1; x <= size; x++) {
      const dx = x < 0 ? -1 : x >= size ? 1 : 0;
      const nb = dx === 0 && dz === 0 ? null : strip(dx, dz);
      // a strip is one column wide across the border, `size` along it
      const c = nb ? (dz === 0 ? z : 0) * (dx === 0 ? size : 1) + (dx === 0 ? x : 0)
        : Math.min(Math.max(z, 0), size - 1) * size + Math.min(Math.max(x, 0), size - 1);
      out.set((nb ?? own).subarray(c * H, (c + 1) * H), ((z + 1) * w + x + 1) * H);
    }
  }
  return out;
}
