// DOM death screen (docs/design/player-death.md Phase B): a nine-slice panel with
// a title and one respawn button, built from the inventory view's panel pieces.
// Shown only while the controller is dead; the pointer is free, so it takes
// pointer events. The layout helpers are pure for Node tests.
//
//   deathScale(w, h) -> integer scale >= 1
//   deathModel() -> [{ id, kind, text }]: exactly the elements the screen has
//   mountDeath(document, set, controller, parent, view) -> { update(dead) }

import { panelImage, PANEL_TEXELS } from './inventoryui.js';
import { uiScale } from './uipx.js';

export const DEATH_TITLE = 'You died';
export const DEATH_RESPAWN = 'Respawn';

export function deathScale(viewportW, viewportH) {
  return uiScale(viewportW, viewportH, 100, 60);
}

export function deathModel() {
  return [
    { id: 'death-title', kind: 'title', text: DEATH_TITLE },
    { id: 'death-respawn', kind: 'button', text: DEATH_RESPAWN },
  ];
}

const CSS = `
#death { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; pointer-events: auto; background: rgba(60, 0, 0, 0.5); font: 12px/1.2 ui-monospace, monospace; color: #fff; image-rendering: pixelated; }
#death[hidden] { display: none; }
#death-panel { box-sizing: border-box; border-style: solid; border-image-slice: ${PANEL_TEXELS}; border-image-repeat: stretch; image-rendering: pixelated; display: flex; flex-direction: column; align-items: center; }
#death-respawn { font: inherit; color: inherit; cursor: pointer; border: 0; background: none; text-shadow: 0 0 3px #000; }
`;

export function mountDeath(document, set, controller, parent = document.body, view = globalThis) {
  const style = document.createElement('style');
  style.textContent = CSS;
  const root = document.createElement('div');
  root.id = 'death';
  root.hidden = true;
  const panel = document.createElement('div');
  panel.id = 'death-panel';
  const els = deathModel().map((m) => {
    const el = document.createElement(m.kind === 'button' ? 'button' : 'div');
    el.id = m.id;
    el.textContent = m.text;
    return el;
  });
  els[1].addEventListener('click', () => controller.respawn());
  panel.append(...els);
  root.append(panel);
  parent.append(style, root);
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
      panel.style.fontSize = `${4 * scale}px`;
    },
  };
}
