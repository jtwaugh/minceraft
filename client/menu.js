// Root menu page logic (docs/design/root-menu.md section 1). The link-building
// is DOM-free so node tests can cover it; the DOM part only runs in a browser.
import { ALPHA, menuEntries } from './scenes.js';

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
    h.appendChild(link(doc, entry.href, entry.name.replace(/ \(alpha\)$/, '')));
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

if (typeof document !== 'undefined') {
  const root = document.getElementById('menu');
  if (root) renderMenu(document, root, menuModel(ALPHA, menuDebug(location.search)));
}
