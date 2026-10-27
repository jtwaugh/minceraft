// DOM panel for the inventory view (docs/design/ui-art-and-inventory.md Phase C):
// draws inventoryModel with the Phase B pieces (nine-slice panel_*, slot frames,
// item icons) at an integer scale, pixelated. Shown only while the controller
// says the inventory is open. The layout helpers are pure for Node tests.
//
//   inventoryScale(w, h) -> integer scale >= 1
//   itemLabel(name) -> display text from an item name ("raw_beef" -> "Raw beef")
//   panelSize(slotCount, slotW, slotH, pad) -> { w, h } in texels
//   mountInventory(document, set, parent, view) -> { update(open, model) }

import { inventoryModel } from './inventory.js';
import { textureCanvas, textureSize, interfaceScale } from './uipx.js';

const PANEL = [['panel_tl', 'panel_t', 'panel_tr'], ['panel_l', 'panel_c', 'panel_r'], ['panel_bl', 'panel_b', 'panel_br']];
export const PANEL_TEXELS = 4;

/** Pixel scale: whole multiples of a 100x60 texel layout that fits the viewport. */
export function inventoryScale(viewportW, viewportH) {
  return interfaceScale(viewportW, viewportH);
}

export function itemLabel(name) {
  const text = String(name ?? '').replace(/_/g, ' ');
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Panel size in texels: a row of slots with `pad` texels around and between them. */
export function panelSize(slotCount, slotW, slotH, pad) {
  return { w: slotCount * slotW + (slotCount + 1) * pad, h: slotH + 2 * pad };
}

const CSS = `
#inventory { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; pointer-events: none; font: 12px/1.2 ui-monospace, monospace; color: #fff; image-rendering: pixelated; }
#inventory[hidden] { display: none; }
#inventory-panel { box-sizing: border-box; border-style: solid; border-image-slice: ${PANEL_TEXELS}; border-image-repeat: stretch; image-rendering: pixelated; display: flex; flex-direction: column; align-items: center; }
#inventory-belt { display: flex; }
.inv-slot { position: relative; display: flex; flex-direction: column; align-items: center; }
.inv-frame { position: relative; }
.inv-frame canvas { position: absolute; left: 0; top: 0; image-rendering: pixelated; }
.inv-label { text-shadow: 0 0 3px #000; min-height: 1.2em; }
`;

export function panelImage(doc, set) {
  const n = PANEL_TEXELS, canvas = doc.createElement('canvas');
  canvas.width = canvas.height = 3 * n;
  const ctx = canvas.getContext('2d');
  PANEL.forEach((row, y) => row.forEach((name, x) => {
    const tile = textureCanvas(set, name, doc);
    if (tile) ctx.drawImage(tile, x * n, y * n);
  }));
  return canvas.toDataURL();
}

export function mountInventory(document, set, parent = document.body, view = globalThis) {
  const style = document.createElement('style');
  style.textContent = CSS;
  const root = document.createElement('div');
  root.id = 'inventory';
  root.hidden = true;
  root.innerHTML = '<div id="inventory-panel"><div id="inventory-hands"></div><div id="inventory-belt"></div></div>';
  parent.append(style, root);
  const panel = root.querySelector('#inventory-panel'), hands = root.querySelector('#inventory-hands'), belt = root.querySelector('#inventory-belt');
  let last = '';
  return {
    update(open, belt_) {
      root.hidden = !open;
      if (!open || !belt_) return;
      const model = inventoryModel(set, belt_, itemLabel);
      const key = JSON.stringify(model) + view.innerWidth + 'x' + view.innerHeight;
      if (key === last) return;
      last = key;
      const scale = inventoryScale(view.innerWidth, view.innerHeight);
      const pad = 3, slot = textureSize(set, 'slot') ?? { w: 10, h: 10 };
      const draw = (name) => {
        const c = textureCanvas(set, name, document);
        if (!c) return null;
        const { w, h } = textureSize(set, name);
        c.style.width = `${w * scale}px`;
        c.style.height = `${h * scale}px`;
        return c;
      };
      const span = (cls, text) => Object.assign(document.createElement('div'), { className: cls, textContent: text ?? '' });
      panel.style.borderImageSource = `url(${panelImage(document, set)})`;
      panel.style.borderWidth = `${PANEL_TEXELS * scale}px`;
      panel.style.padding = `${pad * scale}px`;
      panel.style.gap = `${pad * scale}px`;
      belt.style.gap = `${pad * scale}px`;
      hands.textContent = model.hands.label ? `Hands: ${model.hands.label}` : 'Hands: empty';
      belt.replaceChildren(...model.slots.map((s) => {
        const el = document.createElement('div');
        el.className = s.selected ? 'inv-slot selected' : 'inv-slot';
        const frame = document.createElement('div');
        frame.className = 'inv-frame';
        frame.style.width = `${slot.w * scale}px`;
        frame.style.height = `${slot.h * scale}px`;
        const f = draw(s.frame), ic = s.icon && draw(s.icon);
        if (f) frame.append(f);
        if (ic) {
          const t = textureSize(set, s.icon);
          ic.style.left = `${Math.floor((slot.w - t.w) / 2) * scale}px`;
          ic.style.top = `${Math.floor((slot.h - t.h) / 2) * scale}px`;
          frame.append(ic);
        }
        el.append(frame, span('inv-label', s.label));
        return el;
      }));
    },
  };
}
