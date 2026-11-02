// The "Click to play" cue (docs/design/click-to-play-cue.md Phase B): a small centred
// nine-slice panel with pixel-font text, shown whenever waitingForClick() holds.
// The layout is a pure function for Node tests; mountCue draws it.
//
//   cueLayout(cell, text, viewportW, viewportH) -> { scale, w, h, x, y }
//   mountCue(document, set, parent, view) -> { update(waiting) }

import { drawPanel, PANEL_TILE } from './panel.js';
import { drawString } from './font.js';
import { interfaceScale, textureSize } from './uipx.js';
import { CUE_TEXT } from './waitingcue.js';

/** Texels between the panel's edge and the text. */
const PAD = PANEL_TILE + 1;

/** Panel size in texels and its top-left in CSS pixels, centred in the viewport at the interface scale (lowered until it fits). */
export function cueLayout(cell, text, viewportW, viewportH) {
  const cw = Math.max(1, cell.w), ch = Math.max(1, cell.h);
  const w = [...text].length * cw + 2 * PAD, h = ch + 2 * PAD;
  let scale = interfaceScale(viewportW, viewportH);
  while (scale > 1 && (w * scale > viewportW || h * scale > viewportH)) scale--;
  return { scale, w, h, x: Math.floor((viewportW - w * scale) / 2), y: Math.floor((viewportH - h * scale) / 2) };
}

export function mountCue(document, set, parent = document.body, view = globalThis) {
  const root = document.createElement('div');
  root.id = 'cue';
  root.hidden = true;
  root.style.cssText = 'position: fixed; pointer-events: none; image-rendering: pixelated;';
  parent.append(root);
  const cell = textureSize(set, 'font_unknown') ?? { w: 4, h: 6 };
  const layout = () => {
    const l = cueLayout(cell, CUE_TEXT, view.innerWidth, view.innerHeight);
    const panel = drawPanel(set, l.w, l.h, l.scale, document);
    panel.style.cssText += 'position: absolute; left: 0; top: 0;';
    const text = drawString(set, CUE_TEXT, l.scale, document);
    text.style.cssText += `position: absolute; left: ${PAD * l.scale}px; top: ${PAD * l.scale}px;`;
    root.style.left = `${l.x}px`;
    root.style.top = `${l.y}px`;
    root.style.width = `${l.w * l.scale}px`;
    root.style.height = `${l.h * l.scale}px`;
    root.replaceChildren(panel, text);
  };
  layout();
  view.addEventListener?.('resize', layout);
  return { update(waiting) { root.hidden = !waiting; } };
}
