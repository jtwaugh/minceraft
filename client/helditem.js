// The held item's icon. `held` is the string readBelt (overlay.js) reports:
// empty | tool:<name> | item:<name>. Tools have an icon named after the tool
// ([[tool]] in data/items.toml); items use ITEM_TEXTURES (itemmodel.js).

import { itemTextureName } from './itemmodel.js';

export const HELD_TEXTURES = ['sword', 'hoe'];

/** The icon tile name for `held`, or null for empty or an unknown name. */
export function heldTextureName(held) {
  if (typeof held !== 'string') return null;
  if (held.startsWith('tool:')) {
    const name = held.slice(5);
    return HELD_TEXTURES.includes(name) ? name : null;
  }
  if (held.startsWith('item:')) return itemTextureName(held.slice(5));
  return null;
}

const SLAB = 0.02; // thickness of the sprite's box, in item sizes, as itemBoxes

// Resting pose tunables, mirrored from data/held.toml (checked by the test).
export const HELD_OFFSET = [0.35, -0.3, -0.6];
export const HELD_SCALE = 0.4;
export const HELD_TILT_YAW = 25;
export const HELD_TILT_ROLL = 20;

/** The held thing's boxes: the flat sprite of its icon, as for world items. */
export function heldBoxes(held) {
  const tex = heldTextureName(held);
  if (!tex) return [];
  const faces = { front: tex, back: tex };
  return [{ lo: [-0.5, -0.5, -SLAB / 2], size: [1, 1, SLAB], faces }];
}

/**
 * The held thing's draw at rest: null when the hands are empty, otherwise
 * `{ key, model }`. `key` names the icon; `model` is a column-major camera-space
 * matrix (x right, y up, -z ahead) for the unit box of heldBoxes: scaled,
 * yawed, rolled, then moved to the lower right of the view.
 */
export function heldDraw(held) {
  const key = heldTextureName(held);
  if (!key) return null;
  const s = HELD_SCALE;
  const yaw = (HELD_TILT_YAW * Math.PI) / 180, roll = (HELD_TILT_ROLL * Math.PI) / 180;
  const cy = Math.cos(yaw), sy = Math.sin(yaw), cr = Math.cos(roll), sr = Math.sin(roll);
  // M = T * Ry(yaw) * Rz(roll) * S; columns are the images of x, y, z.
  const model = new Float32Array([
    s * cy * cr, s * sr, -s * sy * cr, 0,
    -s * cy * sr, s * cr, s * sy * sr, 0,
    s * sy, 0, s * cy, 0,
    HELD_OFFSET[0], HELD_OFFSET[1], HELD_OFFSET[2], 1,
  ]);
  return { key, model };
}
