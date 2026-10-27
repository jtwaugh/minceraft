// The `blend` query setting: biome colour blend radius, an integer 0 to 7.
// Returns undefined (use the legend default) unless the value is a number.
export const BLEND_MAX = 7;
export function parseBlend(raw) {
  if (raw == null || String(raw).trim() === '') return undefined;
  const n = Number(raw);
  if (!Number.isFinite(n)) return undefined;
  return Math.min(BLEND_MAX, Math.max(0, Math.round(n)));
}
