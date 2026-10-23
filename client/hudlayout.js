// Pure HUD geometry: integer-pixel rectangles for the crosshair, belt and hearts, from the HUD
// model, the tileset and the viewport. No DOM; imports only the pixel-art helpers. The constants
// below are the positions `mountHud`'s CSS fixes today. The timer and hands labels (phase word, item name)
// are textures named `phase_<phase>` and `label_<item>` (docs/design/play-critic-agent.md, note 7); a
// missing texture is drawn as text, which
// takes `rects.timer.text` / `rects.hands.text` in the same row.

import { hudScale, heartNames, digitNames } from './hud.js';
import { textureSize } from './uipx.js';

/** Crosshair side and the belt's distance from the viewport bottom, in CSS pixels (not scaled). */
export const CROSSHAIR_SIZE = 16;
export const BOTTOM_MARGIN = 12;
/** Pixel-art gaps in texels, multiplied by the HUD scale: between slots, between hearts, and hearts to belt. */
export const SLOT_GAP = 2;
export const HEART_GAP = 2;
export const HEARTS_BELT_GAP = 4;
/** The timer's distance from the viewport top, in CSS pixels (not scaled). */
export const TOP_MARGIN = 8;
/** Timer padding (sides, top and bottom) and the gap inside the timer and hands rows, in texels. */
export const TIMER_PAD_X = 8;
export const TIMER_PAD_Y = 2;
export const LABEL_GAP = 4;
/** Fallback text (a word with no texture) in CSS pixels, matching the HUD's 12px monospace with a 14px line. */
export const TEXT_CHAR_W = 8;
export const TEXT_H = 14;

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
 * (icon and label rects; zero-sized when empty), bottom-aligned just above the hearts and
 * left-aligned with the belt. Hearts sit above the belt, both left-aligned in a column centred horizontally.
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

  const timerNames = [`phase_${model.timer.phase}`, ...digitNames(model.timer.secondsLeft)];
  const hasPhase = textureSize(set, timerNames[0]) !== null;
  const timerRow = row(set, timerNames, 0, 0, LABEL_GAP);
  // Without a phase texture the word is text, ahead of the digits.
  const wordW = hasPhase ? 0 : String(model.timer.phase).length * TEXT_CHAR_W;
  const digitsX = hasPhase || !timerRow.w ? wordW : wordW + LABEL_GAP * scale;
  const timerW = digitsX + timerRow.w * scale + 2 * TIMER_PAD_X * scale;
  const timerH = Math.max(timerRow.h * scale, hasPhase ? 0 : TEXT_H) + 2 * TIMER_PAD_Y * scale;
  const timerX = Math.floor((vw - timerW) / 2);
  const timerLeft = timerX + TIMER_PAD_X * scale, timerTop = TOP_MARGIN + TIMER_PAD_Y * scale;
  const timerParts = place(timerRow, timerLeft + (hasPhase ? 0 : digitsX), timerTop);

  const handNames = model.hands ? [model.hands, `label_${model.hands}`] : [];
  const handRow = row(set, handNames, 0, 0, LABEL_GAP);
  const hasIcon = model.hands && textureSize(set, model.hands) !== null;
  const hasLabel = model.hands && textureSize(set, `label_${model.hands}`) !== null;
  // An item with no label texture is named in text, after the icon (or alone).
  const handText = model.hands && !hasLabel ? String(model.hands).length * TEXT_CHAR_W : 0;
  const textX = handRow.w ? handRow.w * scale + LABEL_GAP * scale : 0;
  const handsH = Math.max(handRow.h * scale, handText ? TEXT_H : 0);
  const handsY = heartsY - handsH;
  const handParts = place(handRow, left, handsY);

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
        phase: hasPhase ? timerParts[0] : null,
        text: hasPhase ? null : rect(timerLeft, timerTop, wordW, TEXT_H),
        digits: timerParts.slice(hasPhase ? 1 : 0),
      },
      hands: {
        ...rect(left, handsY, handText ? textX + handText : handRow.w * scale, handsH),
        text: handText ? rect(left + textX, handsY, handText, TEXT_H) : null,
        icon: hasIcon ? handParts[0] : null,
        label: handParts[hasIcon ? 1 : 0] ?? null,
      },
    },
  };
}
