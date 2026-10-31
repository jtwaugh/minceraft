// The CPU half of meshing a streamed region, with no GL, so Node can run it.
// scene.js uploads what this returns.

import { columnTints } from '../viewer/mesh.js';
import { meshBlocksWasm } from '../viewer/meshwasm.js';
import { padRegion } from './regionpad.js';
import { REGION_SIZE } from './stream.js';

// Meshes region (rx, rz) of `walk`: the mesh arrays by kind (opaque, cutout, water,
// ice), plus the padded blocks, column tints and world origin [x, z] it was meshed from.
export function meshRegionCpu({ walk, terrain, ctx, seed, blendRadius }, { rx, rz }) {
  const x = rx * REGION_SIZE, z = rz * REGION_SIZE;
  // one column of the neighbours' edges around the region, so no seams at its borders
  const blocks = padRegion((a, b) => walk.region_blocks(a, b), (a, b, dx, dz) => walk.region_border(a, b, dx, dz), rx, rz, REGION_SIZE);
  const tints = columnTints(terrain, ctx, seed, x, z, REGION_SIZE, blendRadius);
  const mesh = meshBlocksWasm(terrain, ctx, blocks, tints, REGION_SIZE, 1, { origin: [x, z] });
  return { mesh, blocks, tints, origin: [x, z] };
}
