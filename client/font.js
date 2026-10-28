// Pixel-font string helper for the DOM UI. Plain functions over the parsed
// tileset export; imports only ./uipx.js, so the root page can use it without
// wasm. Every glyph sits in one fixed cell (the size of `font_unknown`), so a
// string's width is its length times the cell width times the scale.
import { findTexture, textureSize } from './uipx.js';

const UNKNOWN = 'font_unknown';

/** Texture name for a character: capitals-only font, so lower case uses the capital glyph; space is null. */
function glyphName(set, ch) {
  const c = ch.toUpperCase();
  let name = null;
  if (c >= 'A' && c <= 'Z' && c.length === 1) name = `font_${c.toLowerCase()}`;
  else if (c >= '0' && c <= '9' && c.length === 1) name = `digit_${c}`;
  return name && findTexture(set, name) ? name : UNKNOWN;
}

function cellSize(set) {
  return textureSize(set, UNKNOWN) ?? { w: 0, h: 0 };
}

/** Size of `text` in CSS pixels: { w, h }. */
export function measureString(set, text, scale) {
  const cell = cellSize(set);
  return { w: [...text].length * cell.w * scale, h: cell.h * scale };
}

/** One { ch, glyph, x, y, w, h } per character, in CSS pixels from (x, y); glyph is null for a space. */
export function stringRects(set, text, scale, x = 0, y = 0) {
  const cell = cellSize(set);
  return [...text].map((ch, i) => ({
    ch,
    glyph: ch === ' ' ? null : glyphName(set, ch),
    x: x + i * cell.w * scale,
    y,
    w: cell.w * scale,
    h: cell.h * scale,
  }));
}

/** Draws `text` into a new pixelated canvas, each texel a scale x scale square. */
export function drawString(set, text, scale, doc = document) {
  const { w, h } = measureString(set, text, scale);
  const canvas = doc.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  canvas.style.imageRendering = 'pixelated';
  const ctx = canvas.getContext('2d');
  for (const r of stringRects(set, text, scale)) {
    if (!r.glyph) continue;
    findTexture(set, r.glyph).forEach((row, ty) => {
      for (let tx = 0; tx < row.length; tx++) {
        const colour = set.palette[row[tx]];
        if (row[tx] === '.' || !colour) continue;
        ctx.fillStyle = colour;
        ctx.fillRect(r.x + tx * scale, r.y + ty * scale, scale, scale);
      }
    });
  }
  return canvas;
}
