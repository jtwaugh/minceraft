// Pure HUD geometry: integer-pixel rectangles for the crosshair, belt and hearts, from the HUD
// model, the tileset and the viewport. No DOM; imports only the pixel-art helpers. The constants
// below are the positions `mountHud`'s CSS fixes today. The timer and hands are not laid out here.

import { hudScale, heartNames } from './hud.js';
import { textureSize } from './uipx.js';

/** Crosshair side and the belt's distance from the viewport bottom, in CSS pixels (not scaled). */
export const CROSSHAIR_SIZE = 16;
export const BOTTOM_MARGIN = 12;
/** Pixel-art gaps in texels, multiplied by the HUD scale: between slots, between hearts, and hearts to belt. */
export const SLOT_GAP = 2;
export const HEART_GAP = 2;
export const HEARTS_BELT_GAP = 4;

const rect = (x, y, w, h) => ({ x, y, w, h });

/** Lays out a row of same-height textures from (x, y); returns the rects and the row's width. */
function row(set, names, x, y, gap) {
  const rects = [];
  let cx = x, h = 0;
  for (const name of names) {
    const t = textureSize(set, name);
    if (!t) continue;
    rects.push(rect(cx, y, t.w, t.h));
    cx += t.w + gap;
    h = Math.max(h, t.h);
  }
  return { rects, w: Math.max(0, cx - x - gap), h };
}

/**
 * `{ scale, rects }` for a `hudModel` result: `rects.crosshair`, `rects.belt` (the whole row),
 * `rects.slots` (one per slot) and `rects.hearts` (the whole row) plus `rects.heartIcons`.
 * Hearts sit above the belt, both left-aligned in a column centred horizontally.
 */
export function hudLayout(model, set, viewport) {
  const { w: vw, h: vh } = viewport;
  const scale = hudScale(vw, vh);
  const grow = (r) => rect(r.x * scale, r.y * scale, r.w * scale, r.h * scale);
  const places = (names, y, gap) => {
    const r = row(set, names, 0, 0, gap);
    return { r, y };
  };

  const slotNames = model.slots.map((s) => (s.selected ? 'slot_selected' : 'slot'));
  const beltRow = places(slotNames, 0, SLOT_GAP);
  const heartRow = places(heartNames(model.hearts), 0, HEART_GAP);
  const beltW = beltRow.r.w * scale, beltH = beltRow.r.h * scale;
  const heartsW = heartRow.r.w * scale, heartsH = heartRow.r.h * scale;
  const colW = Math.max(beltW, heartsW);
  const left = Math.floor((vw - colW) / 2);
  const beltY = vh - BOTTOM_MARGIN - beltH;
  const heartsY = beltY - HEARTS_BELT_GAP * scale - heartsH;

  const place = (r, x, y) => r.rects.map((q) => {
    const g = grow(q);
    return rect(x + g.x, y + g.y, g.w, g.h);
  });
  const slots = place(beltRow.r, left, beltY);
  const heartIcons = place(heartRow.r, left, heartsY);

  return {
    scale,
    rects: {
      crosshair: rect(Math.floor((vw - CROSSHAIR_SIZE) / 2), Math.floor((vh - CROSSHAIR_SIZE) / 2), CROSSHAIR_SIZE, CROSSHAIR_SIZE),
      belt: rect(left, beltY, beltW, beltH),
      slots,
      hearts: rect(left, heartsY, heartsW, heartsH),
      heartIcons,
    },
  };
}
