// Mechanical HUD layout checks: pure functions over `hudLayout`'s result. Each problem is plain data,
// `{ check, elements, detail }`. Only the top-level rectangles are checked; slots, hearts and the
// timer's parts sit inside their parent row.

const ELEMENTS = ['crosshair', 'belt', 'hearts', 'timer', 'hands'];

const area = (r) => r.w * r.h;
const overlaps = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** Pairs of rectangles that overlap. Empty rectangles (an empty hands row) take no space and are skipped. */
export function overlapChecks(layout) {
  const named = ELEMENTS.map((name) => [name, layout.rects[name]]).filter(([, r]) => r && area(r) > 0);
  const found = [];
  for (let i = 0; i < named.length; i++) {
    for (let j = i + 1; j < named.length; j++) {
      const [an, a] = named[i], [bn, b] = named[j];
      if (overlaps(a, b)) found.push({ check: 'overlap', elements: [an, bn], detail: `${an} overlaps ${bn}` });
    }
  }
  return found;
}

/** Rectangles that extend past any edge of the viewport `{ w, h }`. */
export function offscreenChecks(layout, viewport) {
  const found = [];
  for (const name of ELEMENTS) {
    const r = layout.rects[name];
    if (!r || area(r) === 0) continue;
    const edges = [];
    if (r.x < 0) edges.push('left');
    if (r.y < 0) edges.push('top');
    if (r.x + r.w > viewport.w) edges.push('right');
    if (r.y + r.h > viewport.h) edges.push('bottom');
    if (edges.length) found.push({ check: 'offscreen', elements: [name], detail: `${name} extends past the ${edges.join(', ')} edge` });
  }
  return found;
}

/** All mechanical problems in `layout` (a `hudLayout` result) for `viewport`; empty when there are none. */
export function layoutChecks(layout, viewport) {
  return [...overlapChecks(layout), ...offscreenChecks(layout, viewport)];
}
