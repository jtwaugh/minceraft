// Item icons for the model drawer. Each carryable item in data/items.toml
// ([[item]]) has an 8x8 icon in art/ whose tile name is the item's name; the
// drawer packs these into its atlas next to the model faces (see scene.js).

export const ITEM_TEXTURES = [
  'seeds', 'raw_beef', 'leather', 'raw_pork', 'raw_chicken', 'feather',
  'dandelion_flower', 'rose_flower', 'wheat',
];

/** The icon texture name for item `name`, or null for an unknown item (drawn grey). */
export function itemTextureName(name) {
  return ITEM_TEXTURES.includes(name) ? name : null;
}
