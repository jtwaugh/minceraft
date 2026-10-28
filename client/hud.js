// The minimum HUD (docs/design/basic-gameplay.md section 12): crosshair, tool
// belt with the selected slot, the carried item, and health hearts. The model
// is plain data that Node tests check; the DOM renderer is a thin mirror.
//
//   hudModel({ belt, health, maxHealth, phase, secondsLeft }) ->
//     { crosshair, timer: { phase, secondsLeft }, slots: [{ name, selected }], selected, hands, hearts: { full, half, empty } }
//
//   slotView(set, name, selected) -> { frame, icon }   frame is `slot` or `slot_selected`; icon is the item's texture name or null
//   mountHud(document, set, parent) draws frames and icons from the parsed tileset at an integer scale (interfaceScale).
//
// Health is in half-hearts, so 19 of 20 is nine full hearts and one half.

import { hudLayout, PHASE_ICONS } from './hudlayout.js';
import { drawString } from './font.js';
import { drawPanel } from './panel.js';
import { findTexture, textureCanvas, textureSize, interfaceScale } from './uipx.js';

/** Which frame and icon a belt slot shows. Empty slots and items without a texture get a frame only. */
export function slotView(set, name, selected) {
  return { frame: selected ? 'slot_selected' : 'slot', icon: name && findTexture(set, name) ? name : null };
}

/** Pixel scale for the HUD: the shared UI scale rule. */
export function hudScale(viewportW, viewportH) {
  return interfaceScale(viewportW, viewportH);
}

/** Digit texture names for a non-negative whole number: 0 -> ['digit_0'], 125 -> ['digit_1', 'digit_2', 'digit_5']. */
export function digitNames(n) {
  const v = Math.max(0, Math.floor(Number(n)) || 0);
  return [...String(v)].map((d) => `digit_${d}`);
}

/** Heart texture names left to right: full, then half, then empty. */
export function heartNames({ full, half, empty }) {
  return [...Array(full).fill('heart_full'), ...Array(half).fill('heart_half'), ...Array(empty).fill('heart_empty')];
}

export function hudModel({ belt, health, maxHealth, phase = 'gathering', secondsLeft = 0 }) {
  const hp = Math.max(0, Math.min(health, maxHealth));
  const full = Math.floor(hp / 2), half = hp % 2;
  return {
    crosshair: true,
    timer: { phase, secondsLeft },
    slots: belt.slots.map((name, i) => ({ name, selected: i === belt.selected })),
    selected: belt.selected,
    hands: belt.hands,
    hearts: { full, half, empty: Math.ceil(maxHealth / 2) - full - half },
  };
}

const CSS = `
#hud { position: fixed; inset: 0; pointer-events: none; font: 12px/14px ui-monospace, monospace; color: #fff; }
#hud > *, #hud-timer > *, #hud-hands > *, #hud-hearts > *, #hud-belt-panel > *, #hud-belt > *, .slot { position: absolute; }
#hud-cross { background: linear-gradient(#fff, #fff) center / 2px 100% no-repeat, linear-gradient(#fff, #fff) center / 100% 2px no-repeat; mix-blend-mode: difference; }
#hud canvas { image-rendering: pixelated; }
#hud-hands { text-shadow: 0 0 3px #000; }
#hud span { white-space: nowrap; }
`;

/** Sets a rectangle's position and size as CSS pixels, offset by (ox, oy) when it sits inside a placed parent. */
function place(el, r, ox = 0, oy = 0) {
  el.style.left = `${r.x - ox}px`;
  el.style.top = `${r.y - oy}px`;
  el.style.width = `${r.w}px`;
  el.style.height = `${r.h}px`;
}

export function mountHud(document, set, parent = document.body, view = globalThis) {
  const style = document.createElement('style');
  style.textContent = CSS;
  const root = document.createElement('div');
  root.id = 'hud';
  const make = (id) => Object.assign(document.createElement('div'), { id });
  const cross = make('hud-cross'), timer = make('hud-timer'), hands = make('hud-hands');
  const hearts = make('hud-hearts'), beltPanel = make('hud-belt-panel'), belt = make('hud-belt');
  root.append(cross, timer, hands, hearts, beltPanel, belt);
  parent.append(style, root);
  let last = '';
  return {
    update(m) {
      const viewport = { w: view.innerWidth, h: view.innerHeight };
      const key = JSON.stringify([m, viewport]);
      if (key === last) return;
      last = key;
      cross.hidden = !m.crosshair;
      const { scale, rects } = hudLayout(m, set, viewport);
      const icon = (name, r, ox, oy) => {
        const c = textureCanvas(set, name, document);
        if (c) place(c, r, ox, oy);
        return c;
      };
      const label = (name, r, parent, k) => {
        const c = drawString(set, String(name), k, document);
        place(c, r, parent.x, parent.y);
        return c;
      };
      place(cross, rects.crosshair);

      const t = rects.timer;
      place(timer, t);
      const panel = drawPanel(set, t.w / scale, t.h / scale, scale, document);
      timer.replaceChildren(
        panel,
        ...(t.phase ? [icon(PHASE_ICONS[m.timer.phase], t.phase, t.x, t.y)] : []),
        ...digitNames(m.timer.secondsLeft).map((n, i) => icon(n, t.digits[i], t.x, t.y)).filter(Boolean),
      );

      const h = rects.hands;
      place(hands, h);
      hands.replaceChildren(...(m.hands ? [
        h.icon && icon(m.hands, h.icon, h.x, h.y),
        h.label && h.label.w ? label(m.hands, h.label, h, h.scale) : null,
      ].filter(Boolean) : []));

      place(hearts, rects.hearts);
      hearts.replaceChildren(...heartNames(m.hearts).map((n, i) => icon(n, rects.heartIcons[i], rects.hearts.x, rects.hearts.y)).filter(Boolean));

      const bp = rects.beltPanel;
      place(beltPanel, bp);
      beltPanel.replaceChildren(drawPanel(set, bp.w / scale, bp.h / scale, scale, document));

      place(belt, rects.belt);
      belt.replaceChildren(...m.slots.map((slot, i) => {
        const v = slotView(set, slot.name, slot.selected);
        const r = rects.slots[i];
        const el = document.createElement('div');
        el.className = slot.selected ? 'slot selected' : 'slot';
        place(el, r, rects.belt.x, rects.belt.y);
        const frame = icon(v.frame, { x: 0, y: 0, w: r.w, h: r.h }, 0, 0);
        if (frame) el.append(frame);
        if (v.icon) {
          // Centre the icon in the frame, on whole texels.
          const f = textureSize(set, v.frame), tx = textureSize(set, v.icon);
          const ic = icon(v.icon, { x: Math.floor((f.w - tx.w) / 2) * scale, y: Math.floor((f.h - tx.h) / 2) * scale, w: tx.w * scale, h: tx.h * scale }, 0, 0);
          if (ic) el.append(ic);
        }
        const num = rects.slotNumbers[i];
        if (num.glyph) {
          const digit = drawString(set, String(num.n), scale, document);
          place(digit, num, r.x, r.y);
          el.append(digit);
        }
        return el;
      }));
    },
  };
}
