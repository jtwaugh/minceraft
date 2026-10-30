// A small pixel-font button pinned to a viewport corner: a raised panel with
// its label in the pixel font. It takes clicks, so pressing it never reaches
// the canvas and never locks the mouse. Imports only the DOM helpers.
//
//   buttonSize(cell, label) -> { w, h }   in texels
//   mountCornerButton(document, set, { id, corner, label, scale, onClick }) -> { el, w, h, setLabel(label), setScale(scale) }
//     corner  'top-left' | 'top-right'

import { drawPanel, PANEL_TILE } from './panel.js';
import { drawString } from './font.js';
import { textureSize } from './uipx.js';

/** Margin from the viewport edge, in CSS pixels (not scaled). */
export const CORNER_MARGIN = 8;
/** Texels between the panel's edge and the label. */
const PAD = PANEL_TILE - 1;

export function buttonSize(cell, label) {
  return { w: [...label].length * cell.w + 2 * PAD, h: cell.h + 2 * PAD };
}

export function mountCornerButton(document, set, { id, corner = 'top-right', label, scale = 1, onClick }) {
  const el = document.createElement('div');
  el.id = id;
  const side = corner === 'top-left' ? 'left' : 'right';
  el.style.cssText = `position: fixed; top: ${CORNER_MARGIN}px; ${side}: ${CORNER_MARGIN}px; pointer-events: auto; cursor: pointer; image-rendering: pixelated;`;
  el.addEventListener?.('click', (e) => {
    e.stopPropagation?.();
    onClick();
  });
  el.addEventListener?.('mousedown', (e) => e.stopPropagation?.());
  document.body.append(el);
  const cell = textureSize(set, 'font_unknown') ?? { w: 4, h: 6 };
  const api = {
    el, w: 0, h: 0,
    setLabel(next) { label = next; draw(); },
    setScale(next) { if (next !== scale) { scale = next; draw(); } },
  };
  const draw = () => {
    const { w, h } = buttonSize(cell, label);
    const panel = drawPanel(set, w, h, scale, document);
    panel.style.cssText += 'position: absolute; left: 0; top: 0;';
    const text = drawString(set, label, scale, document);
    text.style.cssText += `position: absolute; left: ${PAD * scale}px; top: ${PAD * scale}px;`;
    el.style.width = `${w * scale}px`;
    el.style.height = `${h * scale}px`;
    el.replaceChildren(panel, text);
    api.w = w * scale;
    api.h = h * scale;
  };
  draw();
  return api;
}
