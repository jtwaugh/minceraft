// The CPU half of meshing a streamed region, with no GL, so Node can run it.
// scene.js uploads what this returns.
//
//   gatherRegion({ walk }, { rx, rz })   on the thread owning the sim wasm: { blocks, origin }
//   meshPadded({ terrain, ctx, seed, blendRadius }, { rx, rz, blocks })
//                                        plain data in, no walk, so it can run in a worker:
//                                        { mesh, tints, origin, snow }
//   meshRegionCpu(env, { rx, rz })       the two composed, plus `blocks`

import { columnTints } from '../viewer/mesh.js';
import { meshBlocksWasm } from '../viewer/meshwasm.js';
import { snowOnFoliage } from './particles.js';
import { padRegion } from './regionpad.js';
import { REGION_SIZE } from './stream.js';

// The padded blocks of region (rx, rz) of `walk` and the world origin [x, z] they start at.
export function gatherRegion({ walk }, { rx, rz }) {
  // one column of the neighbours' edges around the region, so no seams at its borders
  const blocks = padRegion((a, b) => walk.region_blocks(a, b), (a, b, dx, dz) => walk.region_border(a, b, dx, dz), rx, rz, REGION_SIZE);
  return { blocks, origin: [rx * REGION_SIZE, rz * REGION_SIZE] };
}

// Meshes the padded `blocks` of region (rx, rz): the mesh arrays by kind (opaque, cutout,
// water, ice), the column tints, the world origin [x, z] and the snowy foliage cells.
export function meshPadded({ terrain, ctx, seed, blendRadius }, { rx, rz, blocks }) {
  const x = rx * REGION_SIZE, z = rz * REGION_SIZE;
  const tints = columnTints(terrain, ctx, seed, x, z, REGION_SIZE, blendRadius);
  const mesh = meshBlocksWasm(terrain, ctx, blocks, tints, REGION_SIZE, 1, { origin: [x, z] });
  return { mesh, tints, origin: [x, z], snow: snowOnFoliage(blocks, REGION_SIZE, 1, [x, z]) };
}

// Meshes region (rx, rz) of `walk`: meshPadded's result plus the padded blocks.
export function meshRegionCpu({ walk, terrain, ctx, seed, blendRadius }, { rx, rz }) {
  const { blocks } = gatherRegion({ walk }, { rx, rz });
  return { ...meshPadded({ terrain, ctx, seed, blendRadius }, { rx, rz, blocks }), blocks };
}
