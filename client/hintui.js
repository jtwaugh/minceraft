// Controls hint as a nine-slice panel with pixel-font text (docs/design/ui-overhaul.md
// Phase D). It shows at game start, folds away the first time the mouse locks, and
// opens again from the `?` button in the top-right corner (until the mouse next locks).
// The layout is a pure function for Node tests; mountHint draws it.
//
//   hintLayout(cell, text, viewportW, viewportH) -> { scale, lines, cols, w, h, x, y }
//   mountHint(document, set, parent, view) -> { update(locked), toggle(), open }
//   createHintState() -> { update(locked), toggle(), open }   (the open/fold rule, no DOM)

import { drawPanel, PANEL_TILE } from './panel.js';
import { drawString } from './font.js';
import { interfaceScale, textureSize } from './uipx.js';
import { buttonSize, mountCornerButton, CORNER_MARGIN } from './cornerbutton.js';

export const HINT_TEXT = 'Click to look around. WASD move, Space jump (fly: up), Shift sneak (fly: down), F walk/fly, Esc releases the mouse.';
export const HINT_BUTTON = '?';

/** Texels of space between the panel's edge and the text, and of margin to the viewport edge. */
const PAD = PANEL_TILE + 1;
const MARGIN = 4;
/** Texels between lines of text, and between the button and the panel. */
const LINE_GAP = 1;
const BUTTON_GAP = 2;
/** Widest line, in characters: a narrow column under the corner button. */
const MAX_COLS = 32;

/** The hint's pixel scale: half the interface scale, rounded up. */
export function hintScale(viewportW, viewportH) {
  return Math.ceil(interfaceScale(viewportW, viewportH) / 2);
}

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
 * Panel layout for `text` in a viewport, in texels at the hint scale (lowered
 * until it fits): `lines`, panel size `w` x `h`, and the top-left `x`, `y` in CSS
 * pixels, right-aligned under the `?` button in the top-right corner. `cell` is
 * the font cell { w, h } in texels.
 */
export function hintLayout(cell, text, viewportW, viewportH) {
  const cw = Math.max(1, cell.w), ch = Math.max(1, cell.h);
  let scale = hintScale(viewportW, viewportH);
  for (;; scale--) {
    const top = CORNER_MARGIN + (buttonSize(cell, HINT_BUTTON).h + BUTTON_GAP) * scale;
    const cols = Math.min(MAX_COLS, Math.floor((Math.floor(viewportW / scale) - 2 * (PAD + MARGIN)) / cw));
    const lines = wrapText(text, Math.max(1, cols));
    const w = Math.max(...lines.map((l) => [...l].length), 0) * cw + 2 * PAD;
    const h = lines.length * ch + Math.max(0, lines.length - 1) * LINE_GAP + 2 * PAD;
    const fits = w + 2 * MARGIN <= viewportW / scale && top + (h + MARGIN) * scale <= viewportH;
    if (fits || scale <= 1) {
      return {
        scale, lines, cols, w, h,
        x: Math.max(0, viewportW - CORNER_MARGIN - w * scale),
        y: top,
      };
    }
  }
}

/** The hint's open/fold rule: starts open, folds on the first transition to locked, `toggle()` flips it. */
export function createHintState() {
  let open = true, wasLocked = false;
  return {
    get open() { return open; },
    toggle() { open = !open; },
    update(locked) {
      if (locked && !wasLocked) open = false;
      wasLocked = locked;
    },
  };
}

export function mountHint(document, set, parent = document.body, view = globalThis) {
  const root = document.createElement('div');
  root.id = 'hint';
  root.style.cssText = 'position: fixed; pointer-events: none; image-rendering: pixelated;';
  parent.append(root);
  const cell = textureSize(set, 'font_unknown') ?? { w: 4, h: 6 };
  let last = '';
  const state = createHintState();
  const show = () => { root.hidden = !state.open; };
  const api = {
    get open() { return state.open; },
    toggle() { state.toggle(); show(); },
    update(locked) { state.update(locked); show(); },
  };
  const button = mountCornerButton(document, set, { id: 'hint-button', label: HINT_BUTTON, onClick: () => api.toggle() });
  const layout = () => {
    const { innerWidth: vw, innerHeight: vh } = view;
    const key = `${vw}x${vh}`;
    if (key === last) return;
    last = key;
    const l = hintLayout(cell, HINT_TEXT, vw, vh);
    button.setScale(l.scale);
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
  return api;
}
