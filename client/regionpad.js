// A region's blocks with a one-column border from its neighbours, so the
// mesher sees across region borders and draws no faces that a neighbour hides.
// Region blocks are 128 x 128 columns with no border (Walk.region_blocks),
// laid out x then z then y like every block array here.
//
//   padRegion((rx, rz) => Walk.region_blocks(rx, rz), rx, rz, 128)  -> 130 x 130 columns
//
// A neighbour that is not loaded (`get` returns empty) is read as the region's
// own nearest edge column, so no wall shows at the edge of the loaded world.

import { H } from '../viewer/mesh.js';

export function padRegion(get, rx, rz, size) {
  const own = get(rx, rz);
  const w = size + 2;
  const out = new Uint8Array(w * w * H);
  const cache = new Map();
  const blocks = (dx, dz) => {
    const k = `${dx},${dz}`;
    if (!cache.has(k)) {
      const b = dx === 0 && dz === 0 ? own : get(rx + dx, rz + dz);
      cache.set(k, b.length ? b : null);
    }
    return cache.get(k);
  };
  for (let z = -1; z <= size; z++) {
    const dz = z < 0 ? -1 : z >= size ? 1 : 0;
    for (let x = -1; x <= size; x++) {
      const dx = x < 0 ? -1 : x >= size ? 1 : 0;
      const nb = blocks(dx, dz);
      const [sx, sz] = nb ? [(x + size) % size, (z + size) % size] : [Math.min(Math.max(x, 0), size - 1), Math.min(Math.max(z, 0), size - 1)];
      const src = (nb ?? own).subarray((sz * size + sx) * H, (sz * size + sx + 1) * H);
      out.set(src, ((z + 1) * w + x + 1) * H);
    }
  }
  return out;
}
