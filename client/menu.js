// Root menu page logic (docs/design/root-menu.md section 1). The link-building
// is DOM-free so node tests can cover it; the DOM part only runs in a browser.
import { ALPHA, menuEntries } from './scenes.js';
import { textureCanvas, textureSize, interfaceScale } from './uipx.js';
import { panelImage } from './panel.js';
import { buttonCss } from './uistyle.js';
import { randomSeed, withSeed, DEFAULT_SEED } from './seed.js';

// Nine-slice panel pieces, 4x4 texels each, and the water tile the sea scrolls.
export const PANEL = [['panel_tl', 'panel_t', 'panel_tr'], ['panel_l', 'panel_c', 'panel_r'], ['panel_bl', 'panel_b', 'panel_br']];
export const SEA_TILE = 'water_0';
export const WORDMARK = 'wordmark';
/** Entry links: section headings and scene links. */
export const BUTTON_SELECTOR = 'body.styled h2 a, body.styled li a';
const SEA_TEXELS = 8;
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
    seedBox: e.key === 'testplay',
    scenes: (e.scenes ?? []).map((s) => ({ name: s.name, description: s.description, href: s.href })),
  }));
}

function link(doc, href, text, title) {
  const a = doc.createElement('a');
  a.href = href;
  a.dataset.href = href;
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
    if (entry.seedBox) section.appendChild(seedBox(doc, section));
    root.appendChild(section);
  }
}

/**
 * Seed row for Test play: a text box and a Random button. Every link in the
 * section gets ?seed= from the box (no ?seed= while it is blank: the default).
 */
function seedBox(doc, section) {
  const row = doc.createElement('p');
  row.className = 'seed';
  const label = doc.createElement('label');
  label.textContent = 'Seed ';
  const input = doc.createElement('input');
  input.id = 'seed';
  input.placeholder = DEFAULT_SEED;
  input.autocomplete = 'off';
  input.spellcheck = false;
  label.appendChild(input);
  const random = doc.createElement('button');
  random.type = 'button';
  random.textContent = 'Random';
  const sync = () => {
    for (const a of section.querySelectorAll('a[data-href]')) a.href = withSeed(a.dataset.href, input.value);
  };
  input.addEventListener('input', sync);
  random.addEventListener('click', () => { input.value = randomSeed(); sync(); });
  row.append(label, random);
  // A value the browser restores on Back still reaches the links.
  setTimeout(sync, 0);
  return row;
}

/** Menu scale: the shared rule, bounded so the 160x120 layout fits. */
export function menuScale(viewportW, viewportH) {
  return interfaceScale(viewportW, viewportH, BASE_W, BASE_H);
}

/** Wordmark size in texels, or null when the tileset lacks it (the text h1 stays). */
export function wordmarkTexels(set) {
  return textureSize(set, WORDMARK);
}

/**
 * Switches the rendered menu to the tile-drawn look. Everything is set as CSS
 * variables on the root, so the plain links keep working if this is skipped.
 */
export function styleMenu(doc, win, set) {
  const sea = textureCanvas(set, SEA_TILE, doc);
  if (!sea) return false;
  const scale = menuScale(win.innerWidth, win.innerHeight);
  const style = doc.documentElement.style;
  style.setProperty('--px', `${scale}px`);
  style.setProperty('--panel', `url(${panelImage(doc, set)})`);
  style.setProperty('--sea', `url(${sea.toDataURL()})`);
  style.setProperty('--sea-size', `${SEA_TEXELS * scale}px`);
  const mark = wordmarkTexels(set);
  const h1 = doc.querySelector('h1');
  if (mark && h1) {
    style.setProperty('--wordmark', `url(${textureCanvas(set, WORDMARK, doc).toDataURL()})`);
    style.setProperty('--wordmark-w', `${mark.w * scale}px`);
    style.setProperty('--wordmark-h', `${mark.h * scale}px`);
  }
  const raised = panelImage(doc, set, 'raised'), pressed = panelImage(doc, set, 'pressed');
  style.setProperty('--btn-raised', `url(${raised})`);
  style.setProperty('--btn-pressed', `url(${pressed})`);
  let btn = doc.getElementById('menu-buttons');
  if (!btn) {
    btn = doc.createElement('style');
    btn.id = 'menu-buttons';
    doc.head.appendChild(btn);
  }
  btn.textContent = `${buttonCss(BUTTON_SELECTOR, scale, raised, pressed)}\n${BUTTON_SELECTOR} { display: block; text-decoration: none; }`;
  h1?.classList.toggle('wordmark', Boolean(mark));
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
