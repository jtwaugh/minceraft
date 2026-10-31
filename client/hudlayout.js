// Pure HUD geometry: integer-pixel rectangles for the crosshair, belt and hearts, from the HUD
// model, the tileset and the viewport. No DOM; imports only the pixel-art helpers. The constants
// below are the positions `mountHud`'s CSS fixes today. The timer sits on a nine-slice panel
// (panel.js) with the `sun` or `moon` icon for the phase beside its `digit_*` textures. The hands
// have their own slot on a strip right of the belt; the carried item's name is drawn above it in
// the pixel font (font.js).

import { hudScale, heartNames, digitNames } from './hud.js';
import { textureSize, BELT_PAD } from './uipx.js';
import { CROSSHAIR_CLEAR_MARGIN } from './hudcheck.js';
import { measureString, stringRects } from './font.js';

/** The belt's distance from the viewport bottom, in CSS pixels (not scaled). */
export const BOTTOM_MARGIN = 12;
/** Pixel-art gaps in texels, multiplied by the HUD scale: between slots, between hearts, and hearts to belt. */
export const SLOT_GAP = 2;
export const HEART_GAP = 2;
export const HEARTS_BELT_GAP = 4;
/** Texels between the belt strip and the hands strip. */
export const HANDS_GAP = 4;
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
 * `{ scale, rects }` for a `hudModel` result: `rects.crosshair`, `rects.belt` (the slot row), `rects.beltPanel` (the strip around it), `rects.slotNumbers` (one glyph rect per slot),
 * `rects.slots` (one per slot), `rects.slotIcons` (the item's icon centred in each slot, or null) and `rects.hearts` (the whole row) plus `rects.heartIcons`,
 * `rects.timer` (box centred at the top, with `phase` and `digits` rects inside), `rects.handsPanel` (the strip
 * right of the belt's), `rects.handsSlot` (the frame on it, with the carried item's `icon` rect or null) and
 * `rects.hands` (the name row above the hands strip, with the label rect and one rect per glyph; zero-sized when
 * empty). The belt and hands strips and the hearts share a centred column; the hearts sit above it on its left
 * edge and the name above it on its right edge, or above the hearts when the two would not fit side by side.
 */
export function hudLayout(model, set, viewport) {
  const { w: vw, h: vh } = viewport;
  const scale = hudScale(vw, vh);
  const grow = (r) => rect(r.x * scale, r.y * scale, r.w * scale, r.h * scale);
  const places = (names, y, gap) => {
    const r = row(set, names, 0, 0, gap);
    return { r, y };
  };

  // The crosshair texture at the HUD scale, centred on whole pixels (zero-sized if the tileset lacks it).
  const cs = textureSize(set, 'crosshair') || { w: 0, h: 0 };
  const crosshair = rect(Math.floor((vw - cs.w * scale) / 2), Math.floor((vh - cs.h * scale) / 2), cs.w * scale, cs.h * scale);

  const slotNames = model.slots.map((s) => (s.selected ? 'slot_selected' : 'slot'));
  const beltRow = places(slotNames, 0, SLOT_GAP);
  const heartRow = places(heartNames(model.hearts), 0, HEART_GAP);
  const beltW = beltRow.r.w * scale, beltH = beltRow.r.h * scale;
  const heartsW = heartRow.r.w * scale, heartsH = heartRow.r.h * scale;
  // The slots sit on a strip BELT_PAD texels wider on every side; the strip's bottom is at the margin.
  const pad = BELT_PAD * scale;
  const panelW = beltW + 2 * pad, panelH = beltH + 2 * pad;
  // The hands strip holds one plain slot frame, as tall as the belt's.
  const handsFrame = textureSize(set, 'slot') || { w: 0, h: 0 };
  const handsPanelW = handsFrame.w * scale + 2 * pad, handsGap = HANDS_GAP * scale;
  const rowW = panelW + handsGap + handsPanelW;
  const colW = Math.max(rowW, heartsW);
  const left = Math.floor((vw - colW) / 2);
  const panelY = vh - BOTTOM_MARGIN - panelH;
  const beltX = left + pad, beltY = panelY + pad;
  const handsPanelX = left + panelW + handsGap;
  const handsSlot = rect(handsPanelX + pad, panelY + pad, handsFrame.w * scale, handsFrame.h * scale);
  // The carried item's icon is centred in the hands frame on whole texels, as in the belt slots.
  const heldSize = model.hands ? textureSize(set, model.hands) : null;
  const heldIcon = heldSize && rect(
    handsSlot.x + Math.floor((handsFrame.w - heldSize.w) / 2) * scale, handsSlot.y + Math.floor((handsFrame.h - heldSize.h) / 2) * scale,
    heldSize.w * scale, heldSize.h * scale);
  const heartsY = panelY - HEARTS_BELT_GAP * scale - heartsH;

  const place = (r, x, y) => r.rects.map((q) => {
    const g = grow(q);
    return rect(x + g.x, y + g.y, g.w, g.h);
  });
  const slots = place(beltRow.r, beltX, beltY);
  // Each item's icon is centred in its frame on whole texels; null for an empty slot or an item without a texture.
  const slotIcons = slots.map((r, i) => {
    const name = model.slots[i].name, f = textureSize(set, slotNames[i]);
    const t = name && f ? textureSize(set, name) : null;
    return t && rect(r.x + Math.floor((f.w - t.w) / 2) * scale, r.y + Math.floor((f.h - t.h) / 2) * scale, t.w * scale, t.h * scale);
  });
  // Slot n (1 to N) shows its digit, one texel in from the frame's top left corner.
  const slotNumbers = slots.map((r, i) => {
    const n = (i + 1) % 10;
    return { ...stringRects(set, String(n), scale, r.x + scale, r.y + scale)[0], n };
  });
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

  // The name sits above the hands strip, right-aligned with it, in the pixel font.
  const name = model.hands ? String(model.hands) : '';
  const measure = (k) => (name ? measureString(set, name, k) : { w: 0, h: 0 });
  // The row shrinks (in whole texels) until it clears the crosshair's clear zone above the hearts.
  const crossBottom = crosshair.y + crosshair.h + CROSSHAIR_CLEAR_MARGIN;
  let handsScale = scale;
  while (handsScale > 1 && heartsY - measure(handsScale).h < crossBottom && measure(handsScale).h > 0) handsScale--;
  const { w: handsW, h: handsH } = measure(handsScale);
  const handsX = handsPanelX + handsPanelW - handsW;
  // Beside the hearts when they fit side by side, otherwise stacked above them.
  const beside = handsW === 0 || handsX >= left + heartsW + LABEL_GAP * scale;
  const handsY = (beside ? heartsY + heartsH : heartsY) - handsH;
  const labelRect = name ? rect(handsX, handsY, handsW, handsH) : null;

  return {
    scale,
    rects: {
      crosshair,
      belt: rect(beltX, beltY, beltW, beltH),
      beltPanel: rect(left, panelY, panelW, panelH),
      slots,
      slotIcons,
      slotNumbers,
      hearts: rect(left, heartsY, heartsW, heartsH),
      heartIcons,
      timer: {
        ...rect(timerX, TOP_MARGIN, timerW, timerH),
        phase: phaseSize ? timerParts[0] : null,
        digits: timerParts.slice(phaseSize ? 1 : 0),
      },
      handsPanel: rect(handsPanelX, panelY, handsPanelW, panelH),
      handsSlot: { ...handsSlot, icon: heldIcon },
      hands: {
        ...rect(handsX, handsY, handsW, handsH),
        scale: handsScale,
        label: labelRect && { ...labelRect, glyphs: stringRects(set, name, handsScale, handsX, handsY) },
      },
    },
  };
}
