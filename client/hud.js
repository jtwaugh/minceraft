// The minimum HUD (docs/design/basic-gameplay.md section 12): crosshair, tool
// belt with the selected slot, the carried item, and health hearts. The model
// is plain data that Node tests check; the DOM renderer is a thin mirror.
//
//   hudModel({ belt, health, maxHealth, phase, secondsLeft }) ->
//     { crosshair, timer: { phase, secondsLeft }, slots: [{ name, selected }], selected, hands, hearts: { full, half, empty } }
//
//   slotView(set, name, selected) -> { frame, icon }   frame is `slot` or `slot_selected`; icon is the item's texture name or null
//   mountHud(document, set, parent) draws frames and icons from the parsed tileset at an integer scale (uiScale).
//
// Health is in half-hearts, so 19 of 20 is nine full hearts and one half.

import { findTexture, textureCanvas, textureSize, uiScale } from './uipx.js';

/** Which frame and icon a belt slot shows. Empty slots and items without a texture get a frame only. */
export function slotView(set, name, selected) {
  return { frame: selected ? 'slot_selected' : 'slot', icon: name && findTexture(set, name) ? name : null };
}

/** Pixel scale for the HUD: whole multiples of the 10x10 slot, about 8 slots wide on the viewport. */
export function hudScale(viewportW, viewportH) {
  return uiScale(viewportW, viewportH, 80, 45);
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
#hud { position: fixed; inset: 0; pointer-events: none; font: 12px/1.2 ui-monospace, monospace; color: #fff; }
#hud-cross { position: absolute; left: 50%; top: 50%; width: 16px; height: 16px; transform: translate(-50%, -50%);
  background: linear-gradient(#fff, #fff) center / 2px 100% no-repeat, linear-gradient(#fff, #fff) center / 100% 2px no-repeat; mix-blend-mode: difference; }
#hud-bottom { position: absolute; left: 50%; bottom: 12px; transform: translateX(-50%); display: flex; flex-direction: column; align-items: flex-start; gap: 4px; }
#hud-timer { position: absolute; left: 50%; top: 8px; transform: translateX(-50%); padding: 2px 8px; background: rgba(0, 0, 0, .55); }
#hud-hearts { display: flex; gap: 2px; }
.heart { width: 14px; height: 14px; background: #3a0a0a; }
.heart.full { background: #d22; }
.heart.half { background: linear-gradient(90deg, #d22 50%, #3a0a0a 50%); }
#hud-belt { display: flex; gap: 2px; }
.slot { position: relative; image-rendering: pixelated; }
.slot canvas { position: absolute; left: 0; top: 0; image-rendering: pixelated; }
#hud-hands { min-height: 1.2em; text-shadow: 0 0 3px #000; display: flex; align-items: center; gap: 4px; }
#hud-hands canvas { image-rendering: pixelated; }
`;

export function mountHud(document, set, parent = document.body, view = globalThis) {
  const style = document.createElement('style');
  style.textContent = CSS;
  const root = document.createElement('div');
  root.id = 'hud';
  root.innerHTML = '<div id="hud-cross"></div><div id="hud-timer"></div><div id="hud-bottom"><div id="hud-hands"></div><div id="hud-hearts"></div><div id="hud-belt"></div></div>';
  parent.append(style, root);
  const cross = root.querySelector('#hud-cross'), timer = root.querySelector('#hud-timer'), hands = root.querySelector('#hud-hands');
  const hearts = root.querySelector('#hud-hearts'), belt = root.querySelector('#hud-belt');
  let last = '';
  return {
    update(m) {
      const key = JSON.stringify(m);
      if (key === last) return;
      last = key;
      cross.hidden = !m.crosshair;
      timer.textContent = `${m.timer.phase} ${m.timer.secondsLeft}s`;
      const scale = hudScale(view.innerWidth, view.innerHeight);
      const icon = (name) => {
        const c = textureCanvas(set, name, document);
        if (!c) return null;
        const { w, h } = textureSize(set, name);
        c.style.width = `${w * scale}px`;
        c.style.height = `${h * scale}px`;
        return c;
      };
      const hand = m.hands && icon(m.hands);
      hands.replaceChildren(...(m.hands ? [hand, Object.assign(document.createElement('span'), { textContent: m.hands })].filter(Boolean) : []));
      const kinds = [...Array(m.hearts.full).fill('full'), ...Array(m.hearts.half).fill('half'), ...Array(m.hearts.empty).fill('empty')];
      hearts.replaceChildren(...kinds.map((k) => Object.assign(document.createElement('div'), { className: `heart ${k}` })));
      belt.replaceChildren(...m.slots.map((slot) => {
        const v = slotView(set, slot.name, slot.selected);
        const el = document.createElement('div');
        el.className = slot.selected ? 'slot selected' : 'slot';
        const frame = icon(v.frame);
        if (frame) el.append(frame);
        const ic = v.icon && icon(v.icon);
        if (ic) {
          // Centre the icon in the frame, on whole texels.
          const f = textureSize(set, v.frame), t = textureSize(set, v.icon);
          ic.style.left = `${Math.floor((f.w - t.w) / 2) * scale}px`;
          ic.style.top = `${Math.floor((f.h - t.h) / 2) * scale}px`;
          el.append(ic);
        }
        return el;
      }));
    },
  };
}
