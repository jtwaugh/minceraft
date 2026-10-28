// Pure HUD geometry: integer-pixel rectangles for the crosshair, belt and hearts, from the HUD
// model, the tileset and the viewport. No DOM; imports only the pixel-art helpers. The constants
// below are the positions `mountHud`'s CSS fixes today. The timer sits on a nine-slice panel
// (panel.js) with the `sun` or `moon` icon for the phase beside its `digit_*` textures. The hands'
// item name is drawn in the pixel font (font.js).

import { hudScale, heartNames, digitNames } from './hud.js';
import { textureSize } from './uipx.js';
import { CROSSHAIR_CLEAR_MARGIN } from './hudcheck.js';
import { measureString, stringRects } from './font.js';

/** Crosshair side and the belt's distance from the viewport bottom, in CSS pixels (not scaled). */
export const CROSSHAIR_SIZE = 16;
export const BOTTOM_MARGIN = 12;
/** Pixel-art gaps in texels, multiplied by the HUD scale: between slots, between hearts, and hearts to belt. */
export const SLOT_GAP = 2;
export const HEART_GAP = 2;
export const HEARTS_BELT_GAP = 4;
/** The timer's distance from the viewport top, in CSS pixels (not scaled). */
export const TOP_MARGIN = 8;
/** Timer padding (sides, top and bottom; the top and bottom clear the panel's border tiles) and the gap inside the timer and hands rows, in texels. */
export const TIMER_PAD_X = 8;
export const TIMER_PAD_Y = 4;
export const LABEL_GAP = 4;
/** The icon texture drawn for each timer phase. */
export const PHASE_ICONS = { gathering: 'sun', service: 'moon' };

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
 * `rects.slots` (one per slot) and `rects.hearts` (the whole row) plus `rects.heartIcons`,
 * `rects.timer` (box centred at the top, with `phase` and `digits` rects inside) and `rects.hands`
 * (icon and label rects, the label with one rect per glyph; zero-sized when empty). The belt and hearts share a centred column;
 * the hearts sit above it on its left edge and the hands above it on its right edge, or above the hearts
 * when the two would not fit side by side.
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

  const phaseName = PHASE_ICONS[model.timer.phase];
  const phaseSize = phaseName ? textureSize(set, phaseName) : null;
  const timerRow = row(set, [...(phaseSize ? [phaseName] : []), ...digitNames(model.timer.secondsLeft)], 0, 0, LABEL_GAP);
  const timerW = timerRow.w * scale + 2 * TIMER_PAD_X * scale;
  const timerH = timerRow.h * scale + 2 * TIMER_PAD_Y * scale;
  const timerX = Math.floor((vw - timerW) / 2);
  const timerLeft = timerX + TIMER_PAD_X * scale, timerTop = TOP_MARGIN + TIMER_PAD_Y * scale;
  // Parts are centred vertically in the row.
  const timerParts = timerRow.rects.map((q) => {
    const g = grow(q);
    return rect(timerLeft + g.x, timerTop + Math.floor((timerRow.h * scale - g.h) / 2), g.w, g.h);
  });

  // The hands sit above the belt, right-aligned with it; the label is the item name in the pixel font.
  const icon = model.hands ? row(set, [model.hands], 0, 0, 0) : { rects: [], w: 0, h: 0 };
  const hasIcon = icon.rects.length > 0;
  const name = model.hands ? String(model.hands) : '';
  const measure = (k) => {
    const label = name ? measureString(set, name, k) : { w: 0, h: 0 };
    return { label, w: icon.w * k + (hasIcon && label.w ? LABEL_GAP * k : 0) + label.w, h: Math.max(icon.h * k, label.h) };
  };
  // The row shrinks (in whole texels) until it clears the crosshair's clear zone above the hearts.
  const crossBottom = Math.floor((vh + CROSSHAIR_SIZE) / 2) + CROSSHAIR_CLEAR_MARGIN;
  let handsScale = scale;
  while (handsScale > 1 && heartsY - measure(handsScale).h < crossBottom && measure(handsScale).h > 0) handsScale--;
  const { label: labelSize, w: handsW, h: handsH } = measure(handsScale);
  const iconW = icon.w * handsScale, iconH = icon.h * handsScale;
  const handsX = left + beltW - handsW;
  // Beside the hearts when they fit side by side, otherwise stacked above them.
  const beside = handsW === 0 || handsX >= left + heartsW + LABEL_GAP * scale;
  const handsY = (beside ? heartsY + heartsH : heartsY) - handsH;
  const labelX = handsX + iconW + (hasIcon && labelSize.w ? LABEL_GAP * handsScale : 0);
  const labelRect = name ? rect(labelX, handsY, labelSize.w, labelSize.h) : null;

  return {
    scale,
    rects: {
      crosshair: rect(Math.floor((vw - CROSSHAIR_SIZE) / 2), Math.floor((vh - CROSSHAIR_SIZE) / 2), CROSSHAIR_SIZE, CROSSHAIR_SIZE),
      belt: rect(left, beltY, beltW, beltH),
      slots,
      hearts: rect(left, heartsY, heartsW, heartsH),
      heartIcons,
      timer: {
        ...rect(timerX, TOP_MARGIN, timerW, timerH),
        phase: phaseSize ? timerParts[0] : null,
        digits: timerParts.slice(phaseSize ? 1 : 0),
      },
      hands: {
        ...rect(handsX, handsY, handsW, handsH),
        icon: hasIcon ? rect(handsX, handsY, iconW, iconH) : null,
        label: labelRect && { ...labelRect, glyphs: stringRects(set, name, handsScale, labelX, handsY) },
      },
    },
  };
}
