// Root menu page logic (docs/design/root-menu.md section 1). The link-building
// is DOM-free so node tests can cover it; the DOM part only runs in a browser.
import { ALPHA, menuEntries } from './scenes.js';
import { textureCanvas, uiScale } from './uipx.js';

// Nine-slice panel pieces, 4x4 texels each, and the water tile the sea scrolls.
export const PANEL = [['panel_tl', 'panel_t', 'panel_tr'], ['panel_l', 'panel_c', 'panel_r'], ['panel_bl', 'panel_b', 'panel_br']];
export const SEA_TILE = 'water_0';
const PANEL_TEXELS = 4, SEA_TEXELS = 8;
// Layout the pixel scale is chosen for, in texels.
const BASE_W = 160, BASE_H = 120;

/** Debug mode for the menu: on for ?debug=1. */
export function menuDebug(search) {
  return new URLSearchParams(search).get('debug') === '1';
}

/**
 * Flat description of the menu: one item per entry, each with its scene links
 * (empty for entries without a scene picker). Hrefs are relative to the root.
 */
export function menuModel(alpha, debug) {
  return menuEntries(alpha, debug).map((e) => ({
    name: e.name,
    label: e.name.replace(/ \(alpha\)$/, ''),
    href: e.href,
    alpha: e.key === 'testplay' && alpha,
    scenes: (e.scenes ?? []).map((s) => ({ name: s.name, description: s.description, href: s.href })),
  }));
}

function link(doc, href, text, title) {
  const a = doc.createElement('a');
  a.href = href;
  a.textContent = text;
  if (title) a.title = title;
  return a;
}

export function renderMenu(doc, root, model) {
  for (const entry of model) {
    const section = doc.createElement('section');
    const h = doc.createElement('h2');
    h.appendChild(link(doc, entry.href, entry.label));
    if (entry.alpha) {
      const tag = doc.createElement('span');
      tag.className = 'alpha';
      tag.textContent = 'alpha';
      h.appendChild(tag);
    }
    section.appendChild(h);
    if (entry.scenes.length) {
      const ul = doc.createElement('ul');
      for (const s of entry.scenes) {
        const li = doc.createElement('li');
        li.appendChild(link(doc, s.href, s.name, s.description));
        ul.appendChild(li);
      }
      section.appendChild(ul);
    }
    root.appendChild(section);
  }
}


/** Composes the 3x3 panel pieces into one canvas, for use as a CSS border-image. */
function panelImage(doc, set) {
  const n = PANEL_TEXELS, canvas = doc.createElement('canvas');
  canvas.width = canvas.height = 3 * n;
  const ctx = canvas.getContext('2d');
  PANEL.forEach((row, y) => row.forEach((name, x) => {
    const tile = textureCanvas(set, name, doc);
    if (tile) ctx.drawImage(tile, x * n, y * n);
  }));
  return canvas.toDataURL();
}

/**
 * Switches the rendered menu to the tile-drawn look. Everything is set as CSS
 * variables on the root, so the plain links keep working if this is skipped.
 */
export function styleMenu(doc, win, set) {
  const sea = textureCanvas(set, SEA_TILE, doc);
  if (!sea) return false;
  const scale = uiScale(win.innerWidth, win.innerHeight, BASE_W, BASE_H);
  const style = doc.documentElement.style;
  style.setProperty('--px', `${scale}px`);
  style.setProperty('--panel', `url(${panelImage(doc, set)})`);
  style.setProperty('--sea', `url(${sea.toDataURL()})`);
  style.setProperty('--sea-size', `${SEA_TEXELS * scale}px`);
  doc.body.classList.add('styled');
  return true;
}

if (typeof document !== 'undefined') {
  const root = document.getElementById('menu');
  if (root) {
    renderMenu(document, root, menuModel(ALPHA, menuDebug(location.search)));
    fetch(new URL('../viewer/textures.json', import.meta.url))
      .then((r) => r.json())
      .then((set) => {
        styleMenu(document, window, set);
        window.addEventListener('resize', () => styleMenu(document, window, set));
      })
      .catch(() => {});
  }
}
