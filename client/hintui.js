// First-load hint as a nine-slice panel with pixel-font text (docs/design/ui-overhaul.md
// Phase D). The layout is a pure function for Node tests; mountHint draws it.
//
//   hintLayout(cell, text, viewportW, viewportH) -> { scale, lines, cols, w, h, x, y }
//   mountHint(document, set, parent, view) -> { setHidden(hidden) }

import { drawPanel, PANEL_TILE } from './panel.js';
import { drawString } from './font.js';
import { interfaceScale, textureSize } from './uipx.js';

export const HINT_TEXT = 'Click to look around. WASD move, Space jump (fly: up), Shift sneak (fly: down), F walk/fly, Esc releases the mouse.';

/** Texels of space between the panel's edge and the text, and of margin to the viewport edge. */
const PAD = PANEL_TILE + 1;
const MARGIN = 4;
/** Texels between lines of text. */
const LINE_GAP = 1;

/** Greedy word wrap into lines of at most `cols` characters; words longer than a line are split. */
export function wrapText(text, cols) {
  const lines = [];
  let line = '';
  for (let word of text.split(/\s+/).filter(Boolean)) {
    while ([...word].length > cols) {
      if (line) lines.push(line);
      line = '';
      lines.push([...word].slice(0, cols).join(''));
      word = [...word].slice(cols).join('');
    }
    if (line && [...line].length + 1 + [...word].length > cols) {
      lines.push(line);
      line = word;
    } else {
      line = line ? `${line} ${word}` : word;
    }
  }
  if (line) lines.push(line);
  return lines;
}

/**
 * Panel layout for `text` in a viewport, in texels at the interface scale (lowered
 * until it fits): `lines`, panel size `w` x `h`, and the top-left `x`, `y` in CSS
 * pixels, centred at the bottom. `cell` is the font cell { w, h } in texels.
 */
export function hintLayout(cell, text, viewportW, viewportH) {
  const cw = Math.max(1, cell.w), ch = Math.max(1, cell.h);
  let scale = interfaceScale(viewportW, viewportH);
  for (;; scale--) {
    const cols = Math.floor((Math.floor(viewportW / scale) - 2 * (PAD + MARGIN)) / cw);
    const lines = wrapText(text, Math.max(1, cols));
    const w = Math.max(...lines.map((l) => [...l].length), 0) * cw + 2 * PAD;
    const h = lines.length * ch + Math.max(0, lines.length - 1) * LINE_GAP + 2 * PAD;
    const fits = w + 2 * MARGIN <= viewportW / scale && h + 2 * MARGIN <= viewportH / scale;
    if (fits || scale <= 1) {
      return {
        scale, lines, cols, w, h,
        x: Math.floor((viewportW - w * scale) / 2),
        y: Math.floor(viewportH - (h + MARGIN) * scale),
      };
    }
  }
}

export function mountHint(document, set, parent = document.body, view = globalThis) {
  const root = document.createElement('div');
  root.id = 'hint';
  root.style.cssText = 'position: fixed; pointer-events: none; image-rendering: pixelated;';
  parent.append(root);
  const cell = textureSize(set, 'font_unknown') ?? { w: 4, h: 6 };
  let last = '';
  const layout = () => {
    const { innerWidth: vw, innerHeight: vh } = view;
    const key = `${vw}x${vh}`;
    if (key === last) return;
    last = key;
    const l = hintLayout(cell, HINT_TEXT, vw, vh);
    const panel = drawPanel(set, l.w, l.h, l.scale, document);
    panel.style.cssText += 'position: absolute; left: 0; top: 0;';
    const kids = [panel];
    l.lines.forEach((line, i) => {
      const text = drawString(set, line, l.scale, document);
      text.style.cssText += `position: absolute; left: ${PAD * l.scale}px; top: ${(PAD + i * (cell.h + LINE_GAP)) * l.scale}px;`;
      kids.push(text);
    });
    root.style.left = `${l.x}px`;
    root.style.top = `${l.y}px`;
    root.style.width = `${l.w * l.scale}px`;
    root.style.height = `${l.h * l.scale}px`;
    root.replaceChildren(...kids);
  };
  layout();
  view.addEventListener?.('resize', layout);
  return {
    setHidden(hidden) {
      root.hidden = hidden;
      if (!hidden) layout();
    },
  };
}
