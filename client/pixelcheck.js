// Palette check over a rendered RGBA buffer (docs/design/play-critic-perf-render.md section 5, Phase F).
// On flat solid geometry every pixel should be a palette colour times a known shade; anything else is a
// seam or a stray colour. Pure functions, no browser: the runner supplies the pixels.
import { AO_SHADE, FACE_SHADE } from '../viewer/mesh.js';

/** Largest per-channel difference from palette colour times shade that still counts as that colour. */
export const TOLERANCE = 3;
/** A pixel whose brightest channel is below this, and which is not palette times shade, is near-black. */
export const NEAR_BLACK = 24;
/** Pixels added on every side of a flagged region's bounding box to make its crop. */
export const CROP_PADDING = 4;
/** Most crops returned; the model only ever looks at this many. */
export const MAX_CROPS = 8;

/** Shade multipliers of the fixed render: every face shade times every AO level. */
export const SHADES = [...new Set(FACE_SHADE.flatMap((f) => AO_SHADE.map((a) => f * a)))];

const hexRgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));

/** Palette as `[r, g, b]` triples, from `viewer/textures.json`'s `palette` (object or array of hex strings or triples). */
export function paletteColors(palette) {
  return Object.values(palette).map((c) => (typeof c === 'string' ? hexRgb(c) : c));
}

const intersects = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

function union(a, b) {
  const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y, n: a.n + b.n };
}

/** Bounding boxes (with pixel counts `n`) of the 8-connected regions of set pixels in `flag`. */
function regions(flag, width, height) {
  const boxes = [], stack = [];
  for (let start = 0; start < flag.length; start++) {
    if (flag[start] !== 1) continue;
    let x0 = width, y0 = height, x1 = -1, y1 = -1, n = 0;
    flag[start] = 2;
    stack.push(start);
    while (stack.length) {
      const i = stack.pop(), x = i % width, y = (i - x) / width;
      n++;
      if (x < x0) x0 = x;
      if (x > x1) x1 = x;
      if (y < y0) y0 = y;
      if (y > y1) y1 = y;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const nx = x + dx, ny = y + dy;
          if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
          const j = ny * width + nx;
          if (flag[j] === 1) { flag[j] = 2; stack.push(j); }
        }
      }
    }
    boxes.push({ x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1, n });
  }
  return boxes;
}

/** Pads each box, clamps it to the image, merges overlapping ones, and keeps the `MAX_CROPS` covering the most pixels. */
function crops(boxes, width, height) {
  let list = boxes.map((b) => {
    const x = Math.max(0, b.x - CROP_PADDING), y = Math.max(0, b.y - CROP_PADDING);
    const x1 = Math.min(width, b.x + b.w + CROP_PADDING), y1 = Math.min(height, b.y + b.h + CROP_PADDING);
    return { x, y, w: x1 - x, h: y1 - y, n: b.n };
  });
  for (let merged = true; merged;) {
    merged = false;
    for (let i = 0; i < list.length && !merged; i++) {
      for (let j = i + 1; j < list.length; j++) {
        if (!intersects(list[i], list[j])) continue;
        list[i] = union(list[i], list[j]);
        list.splice(j, 1);
        merged = true;
        break;
      }
    }
  }
  list.sort((a, b) => b.n - a.n || a.y - b.y || a.x - b.x);
  return list.slice(0, MAX_CROPS).map(({ x, y, w, h }) => ({ x, y, w, h }));
}

/**
 * `{ seams, offPalette, nearBlack, crops }` for an image `{ data, width, height }` (RGBA bytes).
 * Options: `palette` (see `paletteColors`; required), `shades` (default `SHADES`), and either `skip`
 * (a width*height array, nonzero = not solid geometry) or `sky` (an `[r, g, b]` colour to ignore).
 * `seams` is `offPalette + nearBlack`; a near-black pixel is not counted as off-palette.
 */
export function pixelCheck({ data, width, height }, { palette, shades = SHADES, skip = null, sky = null }) {
  const colors = paletteColors(palette);
  const allowed = colors.flatMap((c) => shades.map((s) => c.map((v) => v * s)));
  const verdicts = new Map();
  const classify = (r, g, b) => {
    const key = (r << 16) | (g << 8) | b;
    let v = verdicts.get(key);
    if (v === undefined) {
      v = allowed.some((c) => Math.abs(c[0] - r) <= TOLERANCE && Math.abs(c[1] - g) <= TOLERANCE && Math.abs(c[2] - b) <= TOLERANCE)
        ? 0 : Math.max(r, g, b) < NEAR_BLACK ? 2 : 1;
      verdicts.set(key, v);
    }
    return v;
  };
  const flag = new Uint8Array(width * height);
  let offPalette = 0, nearBlack = 0;
  for (let i = 0; i < width * height; i++) {
    if (skip && skip[i]) continue;
    const r = data[i * 4], g = data[i * 4 + 1], b = data[i * 4 + 2];
    if (sky && r === sky[0] && g === sky[1] && b === sky[2]) continue;
    const v = classify(r, g, b);
    if (v === 1) offPalette++;
    else if (v === 2) nearBlack++;
    if (v) flag[i] = 1;
  }
  return { seams: offPalette + nearBlack, offPalette, nearBlack, crops: crops(regions(flag, width, height), width, height) };
}
