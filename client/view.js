// View geometry and the above/under-water look, apart from WebGL so it can be
// tested in Node (client/test/view.test.mjs).
//
// Near plane. The player is 0.6 wide and its eye sits 1.62 up a 1.8 box
// (data/player.toml), so the eye can be 0.3 from a wall and 0.18 below a
// ceiling. The near plane's corners reach further from the eye than its
// distance: at 70 degrees and 16:9 a 0.3 near plane has corners about 0.52
// away, which cross into a block the player is pressed against and show the
// far side through its culled faces. At 0.05 the corners stay within about
// 0.09 of the eye at 16:9 and 0.10 at 21:9.
//
// Far plane. Depth precision goes with far / near, so far follows the fog
// (solid at FOG_END x fogFar) instead of a fixed 1000.

import { fogAmount } from '../viewer/fog.js';

export const NEAR = 0.05;

/** Fog starts and is solid at these multiples of `fogFar` above water. */
export const FOG_START = 0.55;
export const FOG_END = 1.15;

/** Far clip plane for a scene whose fog is solid at FOG_END x `fogFar`. */
export const farPlane = (fogFar) => fogFar * 1.2;

/** Under water: fog from the eye, solid at this distance in blocks. */
export const UNDERWATER_FOG_END = 18;

/** Unit direction to the sun, the same as the viewer's (low in the north-west). */
export const SUN_DIR = (() => { const v = [-0.7, 0.16, -0.7], l = Math.hypot(...v); return v.map((c) => c / l); })();

/**
 * Fog colour, fog range and colour multiplier for this frame.
 * `water` is the water block's colour from the legend, 0..255 per channel.
 */
export function look({ underwater, skyColor, fogFar, water }) {
  if (!underwater) {
    return { fogColor: skyColor, fogStart: fogFar * FOG_START, fogEnd: fogFar * FOG_END, fogFar: fogFar * FOG_END, tint: [1, 1, 1], sky: true };
  }
  const w = water.map((v) => v / 255);
  const peak = Math.max(...w, 1e-6);
  return {
    // the water colour, darker, for what fades out in the distance
    fogColor: w.map((v) => v * 0.55),
    fogStart: 0,
    fogFar: 0, // 0: linear ramp, not the viewer's curve
    fogEnd: UNDERWATER_FOG_END,
    // half way from white to the water's hue at full brightness
    tint: w.map((v) => 0.5 + 0.5 * (v / peak)),
    sky: false, // flat water fog, no sky pass (decided in #620)
  };
}

/**
 * Inputs of the sky pass and of the fog's sky colour: `base` is the colour to
 * pass as `skyColor`'s base (the fog colour), or null when the sky pass is
 * skipped and fog stays flat (under water). `right`, `up` and `fwd` are the
 * camera basis, `sunDir` is the supplied sun direction (default `SUN_DIR`) and
 * `scale` is [tan(fovY / 2) * aspect, tan(fovY / 2)].
 */
export function skyInputs(v, yaw, pitch, fovY, aspect, sunDir = SUN_DIR) {
  if (!v.sky) return null;
  const { r, u, f } = basis(yaw, pitch);
  const t = Math.tan(fovY / 2);
  return { base: v.fogColor, sunDir, right: r, up: u, fwd: f, scale: [t * aspect, t] };
}

/** Fog amount 0..1 at eye distance `dist`: mirrors the fragment shader. */
export function fogAt(v, dist) {
  if (v.fogFar > 0) return fogAmount(dist, v.fogFar);
  return Math.min(Math.max((dist - v.fogStart) / (v.fogEnd - v.fogStart), 0), 1);
}

/**
 * Forward, right and up for a look, the conventions of crates/game/src/camera.rs:
 * +Y up, yaw 0 faces -Z, increasing yaw turns right, positive pitch looks up.
 */
export function basis(yaw, pitch) {
  const [sy, cy, sp, cp] = [Math.sin(yaw), Math.cos(yaw), Math.sin(pitch), Math.cos(pitch)];
  const f = [sy * cp, sp, -cy * cp];
  const r = [cy, 0, sy];
  const u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
  return { f, r, u };
}

/** The four world-space corners of the near plane. `fovY` in radians. */
export function nearPlaneCorners(eye, yaw, pitch, fovY, aspect, near = NEAR) {
  const { f, r, u } = basis(yaw, pitch);
  const hh = near * Math.tan(fovY / 2), hw = hh * aspect;
  const out = [];
  for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
    out.push([0, 1, 2].map((i) => eye[i] + f[i] * near + r[i] * a * hw + u[i] * b * hh));
  }
  return out;
}

/** The daylight multiplier for terrain and plants this frame: the controller's, or 1 with no day state. */
export const daylightFactor = (day) => day?.daylight ?? 1;
