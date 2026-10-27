// Pixel-art helper for the DOM UI (HUD, menu). Plain functions over the parsed
// tileset export (viewer/textures.json); imports nothing, so it never pulls in
// wasm and the root page can use it. Tile size is the grid's own size, so
// `slot` (10x10) and `digit_*` (4x6) need no special casing.

/** Integer pixel scale >= 1: the largest whole multiple of the base size that fits the viewport. */
export function uiScale(viewportW, viewportH, baseW, baseH) {
  const s = Math.min(Math.floor(viewportW / baseW), Math.floor(viewportH / baseH));
  return Number.isFinite(s) && s >= 1 ? s : 1;
}

/** The belt is 5 slots of 10 texels with 2-texel gaps. */
export const BELT_TEXELS_W = 5 * 10 + 4 * 2;
/** Largest share of the viewport width the belt may take; Phase B lowers this to one third. */
export const BELT_MAX_WIDTH_FRACTION = 0.5;
/** The layout the inventory and death screens use, in texels. */
const PANEL_TEXELS_W = 100;
const PANEL_TEXELS_H = 60;

/** Shared UI scale: the largest integer >= 1 keeping the belt within its width share and the 100x60 layout on screen. */
export function interfaceScale(viewportW, viewportH) {
  const belt = Math.floor((viewportW * BELT_MAX_WIDTH_FRACTION) / BELT_TEXELS_W);
  const s = Math.min(belt, uiScale(viewportW, viewportH, PANEL_TEXELS_W, PANEL_TEXELS_H));
  return Number.isFinite(s) && s >= 1 ? s : 1;
}

/** Finds a texture's grid (array of equal-length strings) by name across all batches; null if absent. */
export function findTexture(set, name) {
  for (const batch of Object.values(set?.batches ?? {})) {
    const rows = batch.textures?.[name];
    if (rows) return rows;
  }
  return null;
}

/** Texture size as { w, h }, or null if the name is unknown. */
export function textureSize(set, name) {
  const rows = findTexture(set, name);
  return rows && { w: rows[0].length, h: rows.length };
}

/** RGBA bytes for a grid: palette index to colour via `palette` ("#rrggbb" values), `.` transparent. */
export function pixelData(rows, palette) {
  const w = rows[0].length, out = new Uint8ClampedArray(w * rows.length * 4);
  rows.forEach((row, y) => {
    for (let x = 0; x < w; x++) {
      const hex = palette[row[x]];
      if (row[x] === '.' || !hex) continue;
      out.set([1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).concat(255), (y * w + x) * 4);
    }
  });
  return out;
}

/** Draws the named texture into a new canvas (1 canvas pixel per texel, pixelated); null if unknown. */
export function textureCanvas(set, name, doc = document) {
  const rows = findTexture(set, name);
  if (!rows) return null;
  const canvas = doc.createElement('canvas');
  canvas.width = rows[0].length;
  canvas.height = rows.length;
  canvas.style.imageRendering = 'pixelated';
  const data = pixelData(rows, set.palette);
  canvas.getContext('2d').putImageData(new ImageData(data, canvas.width, canvas.height), 0, 0);
  return canvas;
}

/** Same as textureCanvas, as a PNG data URL; null if unknown. */
export function textureUrl(set, name, doc = document) {
  return textureCanvas(set, name, doc)?.toDataURL() ?? null;
}
