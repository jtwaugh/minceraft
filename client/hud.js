// The minimum HUD (docs/design/basic-gameplay.md section 12): crosshair, tool
// belt with the selected slot, the carried item, and health hearts. The model
// is plain data that Node tests check; the DOM renderer is a thin mirror.
//
//   hudModel({ belt, health, maxHealth, phase, secondsLeft, pulse, message }) ->
//     { crosshair, timer: { phase, secondsLeft }, slots: [{ name, selected, dim }], selected, hands, ready, stowed, pulse, hearts: { full, half, empty }, message: { text, alpha } }
//
//   slotView(set, name, selected) -> { frame, icon }   frame is `slot` or `slot_selected`; icon is the item's texture name or null
//   mountHud(document, set, parent) draws frames and icons from the parsed tileset at an integer scale (interfaceScale).
//
// `ready` is the selected tool's name when the hands are empty, else null.
// `stowed` is true while the hands are full, so the belt's tools are put away; every slot is then `dim`.
// `pulse` is the hands-frame pulse strength, 0..1 (the controller's timer; 0 when idle).
// `message` is the message line's view (message.js); text '' and alpha 0 when nothing shows.
// Dim slots draw their frame and icon at STOWED_ALPHA, so the selection frame reads as greyed too.
// The ready tool is drawn in the hands frame at READY_ALPHA; a carried item is opaque.
// Health is in half-hearts, so 19 of 20 is nine full hearts and one half.

import { hudLayout, PHASE_ICONS } from './hudlayout.js';
import { drawString } from './font.js';
import { drawPanel } from './panel.js';
import { findTexture, textureCanvas, interfaceScale } from './uipx.js';

const READY_ALPHA = 0.5;
const STOWED_ALPHA = 0.4;

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

export function hudModel({ belt, health, maxHealth, phase = 'gathering', secondsLeft = 0, pulse = 0, message = { text: '', alpha: 0 } }) {
  const hp = Math.max(0, Math.min(health, maxHealth));
  const full = Math.floor(hp / 2), half = hp % 2;
  return {
    crosshair: true,
    timer: { phase, secondsLeft },
    slots: belt.slots.map((name, i) => ({ name, selected: i === belt.selected, dim: belt.hands != null })),
    selected: belt.selected,
    hands: belt.hands,
    ready: !belt.hands && belt.held?.startsWith('tool:') ? belt.held.slice(5) : null,
    stowed: belt.hands != null,
    pulse,
    hearts: { full, half, empty: Math.ceil(maxHealth / 2) - full - half },
    message,
  };
}

const CSS = `
#hud { position: fixed; inset: 0; pointer-events: none; font: 12px/14px ui-monospace, monospace; color: #fff; }
#hud > *, #hud-cross > *, #hud-timer > *, #hud-hands > *, #hud-hearts > *, #hud-belt-panel > *, #hud-belt > *, .slot, .slot > * { position: absolute; }
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
  const handsPanel = make('hud-hands-panel'), handsSlot = make('hud-hands-slot');
  handsSlot.className = 'slot';
  root.append(cross, timer, hands, hearts, beltPanel, belt, handsPanel, handsSlot);
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
      const cc = icon('crosshair', { x: 0, y: 0, w: rects.crosshair.w, h: rects.crosshair.h }, 0, 0);
      cross.replaceChildren(...(cc ? [cc] : []));

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
      hands.replaceChildren(...(m.hands && h.label && h.label.w ? [label(m.hands, h.label, h, h.scale)] : []));

      const hp = rects.handsPanel, hs = rects.handsSlot;
      place(handsPanel, hp);
      handsPanel.replaceChildren(drawPanel(set, hp.w / scale, hp.h / scale, scale, document));
      place(handsSlot, hs);
      const handsIcon = hs.icon && icon(m.hands || m.ready, hs.icon, hs.x, hs.y);
      if (handsIcon && !m.hands) handsIcon.style.opacity = READY_ALPHA;
      handsSlot.replaceChildren(...[icon('slot', { ...hs, x: 0, y: 0 }, 0, 0), handsIcon].filter(Boolean));

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
        if (frame && slot.dim) frame.style.opacity = STOWED_ALPHA;
        if (frame) el.append(frame);
        const ir = rects.slotIcons[i];
        if (v.icon && ir) {
          const ic = icon(v.icon, ir, r.x, r.y);
          if (ic && slot.dim) ic.style.opacity = STOWED_ALPHA;
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
