// ?pixel=1: a fixed render mode so a frame can be reproduced exactly and checked
// against the palette (docs/design/play-critic-perf-render.md Phase F). Parsing and
// the forced values live here, apart from WebGL, so Node can test them; scene.js
// only applies them.
//
//   ?pixel=1&pose=x,y,z,yaw,pitch&view=WxH
//
// pose is the eye position in blocks, then yaw and pitch in radians; view is the
// canvas size in pixels.

import { SUN_DIR } from './view.js';

export const DEFAULT_POSE = [32, 90, 32, 0, -0.3];
export const DEFAULT_VIEW = [640, 360];
export const MAX_VIEW = 4096;

/** What pixel mode forces, by uniform. Fog never mixes in; the tint attribute and tintTex add nothing. */
export const PIXEL_UNIFORMS = {
  fogStart: 1e9, // linear ramp (fogFar 0) that is 0 at every distance
  fogEnd: 2e9,
  fogFar: 0,
  tint: [1, 1, 1],
  daylight: 1,
  biomeTint: 0, // scales the tintTex x tint-attribute term in the fragment shader; 1 in normal rendering
  fogColor: [0.6, 0.75, 1], // the sky pass's base colour, fixed
  sunDir: SUN_DIR,
};

const numbers = (text, n) => {
  const parts = text.split(',');
  if (parts.length !== n || parts.some((p) => p.trim() === '')) return null;
  const v = parts.map(Number);
  return v.every(Number.isFinite) ? v : null;
};

/**
 * Parse a query string. Returns null without `pixel=1`, else
 * `{ pose, width, height }`; throws on a malformed `pose` or `view`.
 */
export function parsePixelMode(search) {
  const qs = new URLSearchParams(search);
  if (qs.get('pixel') !== '1') return null;
  let pose = DEFAULT_POSE;
  if (qs.has('pose')) {
    pose = numbers(qs.get('pose'), 5);
    if (!pose) throw new Error(`bad pose "${qs.get('pose')}": expected x,y,z,yaw,pitch`);
  }
  let [width, height] = DEFAULT_VIEW;
  if (qs.has('view')) {
    const m = /^(\d+)x(\d+)$/.exec(qs.get('view'));
    if (!m || +m[1] < 1 || +m[2] < 1 || +m[1] > MAX_VIEW || +m[2] > MAX_VIEW) throw new Error(`bad view "${qs.get('view')}": expected WxH, 1 to ${MAX_VIEW}`);
    width = +m[1]; height = +m[2];
  }
  return { pose, width, height };
}
