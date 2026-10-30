// DOM death screen (docs/design/player-death.md Phase B): a nine-slice panel with
// a title and one respawn button, built from the inventory view's panel pieces.
// Shown only while the controller is dead; the pointer is free, so it takes
// pointer events. The layout helpers are pure for Node tests.
//
//   deathScale(w, h) -> integer scale >= 1
//   deathModel() -> [{ id, kind, text }]: exactly the elements the screen has
//   mountDeath(document, set, controller, parent, view) -> { update(dead) }

import { panelImage, PANEL_TEXELS } from './inventoryui.js';
import { interfaceScale } from './uipx.js';
import { drawString } from './font.js';
import { panelImage as statePanel } from './panel.js';
import { PALETTE, buttonCss } from './uistyle.js';

export const DEATH_TITLE = 'You died';
export const DEATH_RESPAWN = 'Respawn';

export function deathScale(viewportW, viewportH) {
  return interfaceScale(viewportW, viewportH);
}

export function deathModel() {
  return [
    { id: 'death-title', kind: 'title', text: DEATH_TITLE },
    { id: 'death-respawn', kind: 'button', text: DEATH_RESPAWN },
  ];
}

const BUTTON_ID = '#death-respawn';
const CSS = `
#death { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; pointer-events: auto; background: ${PALETTE.deathDim}; color: ${PALETTE.text}; image-rendering: pixelated; }
#death[hidden] { display: none; }
#death-panel { box-sizing: border-box; border-style: solid; border-image-slice: ${PANEL_TEXELS}; border-image-repeat: stretch; image-rendering: pixelated; display: flex; flex-direction: column; align-items: center; }
#death canvas { display: block; }
`;

export function mountDeath(document, set, controller, parent = document.body, view = globalThis) {
  const style = document.createElement('style');
  style.textContent = CSS;
  const buttonStyle = document.createElement('style');
  const root = document.createElement('div');
  root.id = 'death';
  root.hidden = true;
  const panel = document.createElement('div');
  panel.id = 'death-panel';
  const els = deathModel().map((m) => {
    const el = document.createElement(m.kind === 'button' ? 'button' : 'div');
    el.id = m.id;
    el.setAttribute('aria-label', m.text);
    return el;
  });
  els[1].addEventListener('click', () => controller.respawn());
  panel.append(...els);
  root.append(panel);
  parent.append(style, buttonStyle, root);
  let last = '';
  return {
    update(dead) {
      root.hidden = !dead;
      if (!dead) return;
      const key = `${view.innerWidth}x${view.innerHeight}`;
      if (key === last) return;
      last = key;
      const scale = deathScale(view.innerWidth, view.innerHeight), pad = 3 * scale;
      panel.style.borderImageSource = `url(${panelImage(document, set)})`;
      panel.style.borderWidth = `${PANEL_TEXELS * scale}px`;
      panel.style.padding = `${pad}px`;
      panel.style.gap = `${pad}px`;
      els.forEach((el, i) => el.replaceChildren(drawString(set, deathModel()[i].text, scale, document)));
      buttonStyle.textContent = buttonCss(BUTTON_ID, scale, statePanel(document, set, 'raised'), statePanel(document, set, 'pressed'));
    },
  };
}
