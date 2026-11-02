// Item icons for the model drawer. Each carryable item in data/items.toml
// ([[item]]) has an 8x8 icon in art/ whose tile name is the item's name; the
// drawer packs these into its atlas next to the model faces (see scene.js).

import { CLEAR_EDGES } from './models.js';

export const ITEM_TEXTURES = [
  'seeds', 'raw_beef', 'leather', 'raw_pork', 'raw_chicken', 'feather',
  'dandelion_flower', 'rose_flower', 'wheat',
  'raw_salmon', 'raw_tuna', 'raw_mackerel', 'raw_squid',
];

/** The icon texture name for item `name`, or null for an unknown item (drawn grey). */
export function itemTextureName(name) {
  return ITEM_TEXTURES.includes(name) ? name : null;
}

// Idle motion tunables, mirrored from data/item_motion.toml (checked by the test).
export const BOB_SPEED = 0.5;
export const BOB_AMPLITUDE = 0.15;
const GOLDEN = 0.6180339887498949;
const SLAB = 0.02; // thickness of the sprite's box, in item sizes

/**
 * The unit box (centred on the origin) of item `name`: a thin slab whose
 * front and back faces both show the icon, so a camera-facing pose sees it from
 * either side, and whose thin edges are transparent. Unknown items get grey faces.
 */
export function itemBoxes(name) {
  const tex = itemTextureName(name);
  const faces = tex ? { front: tex, back: tex, ...CLEAR_EDGES } : {};
  return [{ lo: [-0.5, -0.5, -SLAB / 2], size: [1, 1, SLAB], faces }];
}

/**
 * Column-major model matrix for the unit box of itemBoxes: scaled to
 * `item.size`, turned about the vertical to face `camera` ({ yaw } in radians,
 * as view.js basis) and moved to `item.pos` plus the idle bob. The bob depends
 * only on `timeSeconds` and `item.id`; ids are spread out of phase.
 */
export function itemPose(item, timeSeconds, camera) {
  const s = item.size;
  const phase = (item.id * GOLDEN) % 1;
  const bob = BOB_AMPLITUDE * s * Math.sin(2 * Math.PI * (BOB_SPEED * timeSeconds + phase));
  // The box's -z side is its front; turning it by the camera's yaw makes the
  // back face (local +z) look at the camera, which is what we draw as well.
  const c = Math.cos(camera.yaw), n = Math.sin(camera.yaw);
  return new Float32Array([
    c * s, 0, n * s, 0,
    0, s, 0, 0,
    -n * s, 0, c * s, 0,
    item.pos[0], item.pos[1] + bob, item.pos[2], 1,
  ]);
}

/** One { key, model } per item: `key` is the item's name (for itemBoxes), `model` its itemPose. */
export function itemDraws(items, timeSeconds, camera) {
  return items.map((item) => ({ key: item.name, model: itemPose(item, timeSeconds, camera) }));
}

export const STATION_ITEM_SIZE = 0.5; // edge of an item resting on a station, in blocks

/** `[{ cell, name }]` for the items on raft stations (Walk.station_cells, Walk.station_item_names). */
export function readStations(walk) {
  const cells = Array.from(walk.station_cells());
  return Array.from(walk.station_item_names()).map((name, i) => ({ cell: cells.slice(i * 3, i * 3 + 3), name }));
}

/**
 * Column-major model matrix, in raft-local coordinates (those of raftBoxes), of
 * an item resting on the station at `cell` `[x, y, z]`: centred over the cell's
 * top face with its bottom edge on it. Apply the raft matrix on top to place it
 * in the world.
 */
export function stationPose(cell, size = STATION_ITEM_SIZE) {
  return new Float32Array([
    size, 0, 0, 0,
    0, size, 0, 0,
    0, 0, size, 0,
    cell[0] + 0.5, cell[1] + 1 + size / 2, cell[2] + 0.5, 1,
  ]);
}

/** Column-major product a * b of two 4x4 matrices. */
export function mulMat4(a, b) {
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      let v = 0;
      for (let k = 0; k < 4; k++) v += a[k * 4 + r] * b[c * 4 + k];
      out[c * 4 + r] = v;
    }
  }
  return out;
}

/**
 * One { key, model } per station item: `key` the item name (for itemBoxes),
 * `model` the station pose carried by `raft`, the raft's own matrix, so the
 * item moves and turns with the raft's blocks.
 */
export function stationDraws(stations, raft) {
  return stations.map(({ cell, name }) => ({ key: name, model: mulMat4(raft, stationPose(cell)) }));
}
