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
