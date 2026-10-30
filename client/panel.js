// Nine-slice panel helper for the DOM UI. Plain functions over the parsed
// tileset export; imports only ./uipx.js, so the root page can use it without
// wasm. The `panel_*` tiles are TILE x TILE texels: corners are fixed, edges
// repeat along one axis and `panel_c` repeats along both.
import { findTexture, textureCanvas } from './uipx.js';

export const PANEL_TILE = 4;

const NAMES = [
  ['panel_tl', 'panel_t', 'panel_tr'],
  ['panel_l', 'panel_c', 'panel_r'],
  ['panel_bl', 'panel_b', 'panel_br'],
];

/**
 * Splits one axis of `n` texels into [{ part, at, len, src }]: part 0 is the
 * leading corner, 1 the repeating middle (one entry per tile, the last one
 * cropped), 2 the trailing corner. Below two tiles the corners share the
 * length, the trailing one showing the far part of its tile (`src`).
 */
function axisSegments(n) {
  const t = PANEL_TILE;
  const lead = Math.min(t, n >> 1);
  const trail = Math.min(t, n - lead);
  const out = [];
  if (lead) out.push({ part: 0, at: 0, len: lead, src: 0 });
  for (let at = lead; at < n - trail; at += t) out.push({ part: 1, at, len: Math.min(t, n - trail - at), src: 0 });
  if (trail) out.push({ part: 2, at: n - trail, len: trail, src: t - trail });
  return out;
}

/**
 * Tile rects covering a `w` x `h` texel panel exactly, in CSS pixels from
 * (x, y) at `scale`: { name, x, y, w, h, sx, sy }. (sx, sy) is the texel offset
 * into the tile for cropped tiles; the rect shows `w/scale` x `h/scale` texels from there.
 */
export function panelRects(w, h, scale, x = 0, y = 0) {
  const cols = axisSegments(Math.max(0, w));
  const rows = axisSegments(Math.max(0, h));
  const out = [];
  for (const r of rows) {
    for (const c of cols) {
      out.push({
        name: NAMES[r.part][c.part],
        x: x + c.at * scale,
        y: y + r.at * scale,
        w: c.len * scale,
        h: r.len * scale,
        sx: c.src,
        sy: r.src,
      });
    }
  }
  return out;
}

/** Draws a `w` x `h` texel panel into a new pixelated canvas, each texel a scale x scale square. */
export function drawPanel(set, w, h, scale, doc = document) {
  const canvas = doc.createElement('canvas');
  canvas.width = w * scale;
  canvas.height = h * scale;
  canvas.style.imageRendering = 'pixelated';
  const ctx = canvas.getContext('2d');
  for (const r of panelRects(w, h, scale)) {
    const rows = findTexture(set, r.name);
    if (!rows) continue;
    for (let ty = 0; ty < r.h / scale; ty++) {
      for (let tx = 0; tx < r.w / scale; tx++) {
        const ch = rows[r.sy + ty]?.[r.sx + tx];
        const colour = set.palette[ch];
        if (ch === '.' || !colour) continue;
        ctx.fillStyle = colour;
        ctx.fillRect(r.x + tx * scale, r.y + ty * scale, scale, scale);
      }
    }
  }
  return canvas;
}

/**
 * Tile names for a panel state: 'raised' (default) uses `panel_*`, 'pressed'
 * uses `panel_pressed_*`, falling back to the raised tiles if the set lacks any.
 */
export function panelNames(set, state = 'raised') {
  if (state === 'pressed') {
    const names = NAMES.map((row) => row.map((n) => n.replace('panel_', 'panel_pressed_')));
    if (names.flat().every((n) => findTexture(set, n))) return names;
  }
  return NAMES;
}

/** Composes the 3x3 panel tiles into one canvas data URL, for use as a CSS border-image. */
export function panelImage(doc, set, state = 'raised') {
  const n = PANEL_TILE, canvas = doc.createElement('canvas');
  canvas.width = canvas.height = 3 * n;
  const ctx = canvas.getContext('2d');
  panelNames(set, state).forEach((row, y) => row.forEach((name, x) => {
    const tile = textureCanvas(set, name, doc);
    if (tile) ctx.drawImage(tile, x * n, y * n);
  }));
  return canvas.toDataURL();
}
