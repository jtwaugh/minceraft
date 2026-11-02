// The fishing bobber and its line, with no GL: the bobber is a small box
// textured with the `bobber` tile and sunk a little while the bite window is
// open; the line is a thin box stretched from the rod tip to the bobber, so it
// draws through the same model drawer as everything else.

import { CLEAR_EDGES } from './models.js';

export const BOBBER_TEXTURES = ['bobber'];

export const BOBBER_SIZE = 0.25; // edge of the bobber's cube, in blocks
export const DIP_DEPTH = 0.12; // how far a dipping bobber sits lower, in blocks
export const LINE_THICKNESS = 0.02; // edge of the line's square section, in blocks
export const LINE_TINT = [0.9, 0.9, 0.9];

/** The bobber cube centred on the origin; only its four sides show the tile. */
export function bobberBoxes() {
  const h = BOBBER_SIZE / 2;
  const side = 'bobber';
  return [{ lo: [-h, -h, -h], size: [BOBBER_SIZE, BOBBER_SIZE, BOBBER_SIZE], faces: { front: side, back: side, ...CLEAR_EDGES } }];
}

/** The line's unit box: 1 long along +z from the origin, centred on x and y (scaled by lineMatrix). */
export function lineBoxes() {
  return [{ lo: [-0.5, -0.5, 0], size: [1, 1, 1], faces: {} }];
}

/** Where the bobber's centre is drawn: the sim position, lowered while dipping. */
export function bobberCentre(bobber) {
  const [x, y, z] = bobber.pos;
  return [x, bobber.dipping ? y - DIP_DEPTH : y, z];
}

/** Column-major model matrix moving the origin-centred bobber cube to `centre`. */
export function bobberMatrix(centre) {
  return new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, centre[0], centre[1], centre[2], 1]);
}

/**
 * Column-major model matrix taking lineBoxes' unit box onto the segment from
 * `a` to `b` with a LINE_THICKNESS square section, or null if they coincide.
 */
export function lineMatrix(a, b) {
  const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const len = Math.hypot(...d);
  if (len < 1e-6) return null;
  const z = d.map((v) => v / len);
  const ref = Math.abs(z[1]) > 0.99 ? [1, 0, 0] : [0, 1, 0];
  const cross = (p, q) => [p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]];
  let x = cross(ref, z);
  const xl = Math.hypot(...x);
  x = x.map((v) => v / xl);
  const y = cross(z, x);
  const t = LINE_THICKNESS;
  return new Float32Array([
    x[0] * t, x[1] * t, x[2] * t, 0,
    y[0] * t, y[1] * t, y[2] * t, 0,
    d[0], d[1], d[2], 0,
    a[0], a[1], a[2], 1,
  ]);
}

/**
 * What to draw for `bobber` (readBobber's result, or null) and the rod `tip`:
 * null with no bobber, else { bobber: model matrix, line: model matrix or null,
 * lineTint }. The line ends at the drawn (possibly dipped) bobber centre.
 */
export function bobberDraw(bobber, tip) {
  if (!bobber) return null;
  const centre = bobberCentre(bobber);
  return { bobber: bobberMatrix(centre), line: lineMatrix(tip, centre), lineTint: LINE_TINT };
}
