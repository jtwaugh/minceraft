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

// Pose at phase 1 of the swing and of the use, mirrored from data/hands.toml.
export const HELD_SWING_OFFSET = [-0.2, 0.1, -0.1];
export const HELD_SWING_PITCH = 50;
export const HELD_USE_OFFSET = [-0.05, 0.05, -0.1];
export const HELD_USE_PITCH = 15;

/** The held thing's boxes: the flat sprite of its icon, as for world items. */
export function heldBoxes(held) {
  const tex = heldTextureName(held);
  if (!tex) return [];
  const faces = { front: tex, back: tex };
  return [{ lo: [-0.5, -0.5, -SLAB / 2], size: [1, 1, SLAB], faces }];
}

/**
 * The held thing's draw: null when the hands are empty, otherwise
 * `{ key, model }`. `key` names the icon; `model` is a column-major camera-space
 * matrix (x right, y up, -z ahead) for the unit box of heldBoxes: scaled,
 * yawed, rolled, then moved to the lower right of the view. `swing` and `use`
 * are the hand animation phases (0 idle, 1 mid-animation); each adds its offset
 * and a pitch about the sprite's centre, scaled by the phase. At 0 and 0 this is
 * the resting pose.
 */
export function heldDraw(held, swing = 0, use = 0) {
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
  const pitch = ((HELD_SWING_PITCH * swing + HELD_USE_PITCH * use) * Math.PI) / 180;
  if (pitch !== 0) {
    const cp = Math.cos(pitch), sp = Math.sin(pitch);
    for (let c = 0; c < 12; c += 4) {
      const y = model[c + 1], z = model[c + 2];
      model[c + 1] = cp * y - sp * z;
      model[c + 2] = sp * y + cp * z;
    }
  }
  for (let i = 0; i < 3; i++) model[12 + i] += HELD_SWING_OFFSET[i] * swing + HELD_USE_OFFSET[i] * use;
  return { key, model };
}
