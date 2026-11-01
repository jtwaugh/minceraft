// Scene list and root menu entries (docs/design/root-menu.md sections 1 and 2).
// Plain data and pure functions: no DOM, no wasm. Adding a scene means adding
// an entry to SCENES; scene.js and the menu both read it from here.

// True while the game is in alpha: Test play is on the main menu. When false,
// Test play shows only with debug mode on.
export const ALPHA = true;

// `id` is the ?scene= value; the open-world scene has none (id: null).
export const SCENES = [
  { id: null, name: 'Open world', description: 'Generated terrain around the seed' },
  { id: 'raft', name: 'Raft', description: 'A raft rocking on a flat sea' },
];

/** The scene for a ?scene= value, or undefined if the id is unknown. */
export function sceneById(id) {
  if (id === null || id === undefined || id === '') return undefined;
  return SCENES.find((s) => s.id === id);
}

/**
 * Entries for the main menu list. World gen is always there; Test play (with
 * its scene picker) is there when `alpha` is on, or when `debug` is on.
 * Each entry is { key, name, href, scenes? }; the "alpha" label is part of
 * Test play's name while the flag is on.
 */
export function menuEntries(alpha, debug) {
  const entries = [{ key: 'worldgen', name: 'World gen', href: 'viewer/' }];
  if (alpha || debug) {
    entries.push({
      key: 'testplay',
      name: alpha ? 'Test play (alpha)' : 'Test play',
      href: 'client/',
      scenes: SCENES.map((s) => ({
        ...s,
        href: s.id === null ? 'client/' : `client/?scene=${encodeURIComponent(s.id)}`,
      })),
    });
  }
  return entries;
}
