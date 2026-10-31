// World seed text: what the menu's seed box and client/?seed= accept. Plain
// functions, no DOM, no wasm. The rule matches the viewer's seed box
// (viewer/index.html parseSeed), so the same text gives the same world in both.

export const DEFAULT_SEED = '12345';

/**
 * Seed text to the u64 the wasm takes: integers (negative ones wrap to 64
 * bits), anything else hashed with FNV-1a 64 over its UTF-8 bytes. Blank text
 * is the default seed.
 */
export function parseSeed(text) {
  const s = String(text ?? '').trim() || DEFAULT_SEED;
  if (/^-?\d+$/.test(s)) return BigInt.asUintN(64, BigInt(s));
  let h = 0xcbf29ce484222325n;
  for (const b of new TextEncoder().encode(s)) {
    h ^= BigInt(b);
    h = BigInt.asUintN(64, h * 0x100000001b3n);
  }
  return h;
}

/** A fresh random seed as decimal text; `random` returns [0, 1). */
export function randomSeed(random = Math.random) {
  return String(Math.floor(random() * 2 ** 31) * 2 ** 21 + Math.floor(random() * 2 ** 21));
}

/** `href` with `?seed=` set to the seed text, or unchanged when the text is blank. */
export function withSeed(href, text) {
  const s = String(text ?? '').trim();
  if (!s) return href;
  const [path, query = ''] = href.split('?');
  const qs = new URLSearchParams(query);
  qs.set('seed', s);
  return `${path}?${qs}`;
}
