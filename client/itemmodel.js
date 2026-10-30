// Item icons for the model drawer. Each carryable item in data/items.toml
// ([[item]]) has an 8x8 icon in art/ whose tile name is the item's name; the
// drawer packs these into its atlas next to the model faces (see scene.js).

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
 * either side. Unknown items get grey faces.
 */
export function itemBoxes(name) {
  const tex = itemTextureName(name);
  const faces = tex ? { front: tex, back: tex } : {};
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
