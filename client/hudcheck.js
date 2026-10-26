// Mechanical HUD layout checks: pure functions over `hudLayout`'s result. Each problem is plain data,
// `{ check, elements, detail }`. Only the top-level rectangles are checked; slots, hearts and the
// timer's parts sit inside their parent row.

/** Thresholds (docs/design/play-critic-agent.md section 3, decided in #970 and #971); to be tuned later. */
export const MIN_SCALE = 2;
/** Space kept clear around the crosshair on every side, in CSS pixels. */
export const CROSSHAIR_CLEAR_MARGIN = 8;
/** Largest gap between the hands row and the belt, in texels (multiplied by the HUD scale). */
export const MAX_HANDS_BELT_GAP = 16;

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

/** HUD scale below `MIN_SCALE`: a finding, or advisory (`severity`) on a portrait viewport (height greater than width). */
export function scaleChecks(layout, viewport) {
  if (!(layout.scale < MIN_SCALE)) return [];
  const severity = viewport.h > viewport.w ? 'advisory' : 'finding';
  return [{ check: 'scale', severity, elements: [], detail: `HUD scale ${layout.scale} is below ${MIN_SCALE}` }];
}

/** Elements other than the crosshair that enter the space `CROSSHAIR_CLEAR_MARGIN` around it. */
export function clearZoneChecks(layout) {
  const c = layout.rects.crosshair;
  if (!c) return [];
  const m = CROSSHAIR_CLEAR_MARGIN;
  const zone = { x: c.x - m, y: c.y - m, w: c.w + 2 * m, h: c.h + 2 * m };
  return ELEMENTS.filter((name) => name !== 'crosshair' && layout.rects[name] && area(layout.rects[name]) > 0 && overlaps(zone, layout.rects[name]))
    .map((name) => ({ check: 'clear-zone', elements: [name], detail: `${name} is inside the crosshair clear zone` }));
}

/** The hands row sitting more than `MAX_HANDS_BELT_GAP` texels above the belt. Skipped when the hands are empty. */
export function handsDistanceChecks(layout) {
  const { hands, belt } = layout.rects;
  if (!hands || !belt || area(hands) === 0) return [];
  const gap = belt.y - (hands.y + hands.h), limit = MAX_HANDS_BELT_GAP * (layout.scale ?? 1);
  if (gap <= limit) return [];
  return [{ check: 'distance', elements: ['hands', 'belt'], detail: `hands are ${gap}px above the belt, more than ${limit}px` }];
}

/** All mechanical problems in `layout` (a `hudLayout` result) for `viewport`; empty when there are none. */
export function layoutChecks(layout, viewport) {
  return [...overlapChecks(layout), ...offscreenChecks(layout, viewport), ...scaleChecks(layout, viewport), ...clearZoneChecks(layout), ...handsDistanceChecks(layout)];
}
