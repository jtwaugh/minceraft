import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { FIXED_WIND, SWAY_FLOOR, swayOffset, swayPhase, trunkLean, SWAY_AMPLITUDE, SWAY_WAVE, TRUNK_SWAY_AMPLITUDE } from '../sway.js';

test('sway: zero at weight 0, bounded by the amplitude', () => {
  for (const [x, z, p] of [[0, 0, 0], [13.5, -7, 2], [-100, 55, 5]]) {
    assert.ok(swayOffset(0, x, z, p, FIXED_WIND).every((d) => d === 0));
    const [dx, dz] = swayOffset(1, x, z, p, FIXED_WIND);
    assert.ok(Math.abs(dx) <= SWAY_AMPLITUDE && Math.abs(dz) <= SWAY_AMPLITUDE);
  }
});

test('sway: varies with time and world position, scales with weight', () => {
  assert.notDeepEqual(swayOffset(1, 3, 4, 0, FIXED_WIND), swayOffset(1, 3, 4, 1, FIXED_WIND));
  assert.notDeepEqual(swayOffset(1, 3, 4, 0, FIXED_WIND), swayOffset(1, 9, 4, 0, FIXED_WIND));
  const [fx] = swayOffset(1, 3, 4, 0.5, FIXED_WIND), [hx] = swayOffset(0.5, 3, 4, 0.5, FIXED_WIND);
  assert.ok(Math.abs(hx - fx / 2) < 1e-12);
});

test('sway phase: wraps, and is still when motion is reduced', () => {
  assert.equal(swayPhase(123456, true), 0);
  const p = swayPhase(1e9, false);
  assert.ok(p >= 0 && p < 2 * Math.PI);
});

test('trunk lean: zero at weight 0, grows with weight, same wave as plants', () => {
  assert.ok(TRUNK_SWAY_AMPLITUDE > 0 && TRUNK_SWAY_AMPLITUDE < SWAY_AMPLITUDE);
  const still = swayPhase(123456, true);
  for (const [x, z, p] of [[1, 1, still + 1], [13.5, -7, 2], [-100, 55, 5]]) {
    assert.ok(trunkLean(0, x, z, p, FIXED_WIND).every((d) => d === 0));
    const mag = (w) => Math.hypot(...trunkLean(w, x, z, p, FIXED_WIND));
    assert.ok(mag(0.25) < mag(0.5) && mag(0.5) < mag(1));
    const k = TRUNK_SWAY_AMPLITUDE / SWAY_AMPLITUDE;
    const [lx, lz] = trunkLean(1, x, z, p, FIXED_WIND), [px, pz] = swayOffset(1, x, z, p, FIXED_WIND);
    assert.ok(Math.abs(lx - px * k) < 1e-12 && Math.abs(lz - pz * k) < 1e-12);
  }
});

test('trunk lean: at weight 1 never exceeds half a texel (0.03 blocks)', () => {
  let max = 0;
  for (let x = -40; x <= 40; x += 0.5) {
    for (let z = -40; z <= 40; z += 0.5) {
      for (let p = 0; p < 2 * Math.PI; p += 0.25) max = Math.max(max, Math.hypot(...trunkLean(1, x, z, p, FIXED_WIND)));
    }
  }
  assert.ok(max <= 0.03, `max lean ${max}`);
  assert.ok(Math.SQRT2 * TRUNK_SWAY_AMPLITUDE <= 0.03); // bound on any (x, z, phase)
});

// The vertex shader in scene.js, read as text (it needs WebGL to run).
const scene = readFileSync(new URL('../scene.js', import.meta.url), 'utf8');
const swayGlsl = new Function('SWAY_WAVE', 'SWAY_FLOOR', `return ${scene.match(/const swayGlsl = (\(amplitude\) => `[^`]*`);/)[1]};`)(SWAY_WAVE, SWAY_FLOOR);

/** Runs the GLSL of `swayGlsl(amplitude)` as JS for a vertex at world (x, z) with weight `top`: its [dx, dz]. */
function runGlsl(amplitude, top, x, z, swayPhase, wind) {
  const js = swayGlsl(amplitude).replace('float k', 'const k').replace('vec2 wd', 'const wd')
    .replace(/w\.xz \+= (.*) \* top \* sin\(swayPhase \+ dot\(w\.xz, wd\) \* k\) \* wd;$/,
      'const a = $1 * top * Math.sin(swayPhase + (w.x * wd[0] + w.z * wd[1]) * k); return [a * wd[0], a * wd[1]];');
  return new Function('w', 'top', 'swayPhase', 'windDir', 'windStrength', js)({ x, z }, top, swayPhase, wind.dir, wind.strength);
}

const WINDS = [FIXED_WIND, { dir: [1, 0], strength: 0 }, { dir: [-0.6, 0.8], strength: 0.4 }, { dir: [0, 0], strength: 1 }];

test('sway shader: cutout and opaque programs share one wave built from the sway.js constants', () => {
  assert.match(swayGlsl(SWAY_AMPLITUDE), new RegExp(`float k = ${SWAY_WAVE.toFixed(3)};`));
  assert.match(swayGlsl(SWAY_AMPLITUDE), new RegExp(`${SWAY_FLOOR.toFixed(3)} \\+ ${(1 - SWAY_FLOOR).toFixed(3)} \\* windStrength`));
  assert.match(swayGlsl(SWAY_AMPLITUDE), /vec2 wd = windDir;/);
  assert.equal(scene.match(/sin\(swayPhase/g).length, 1); // one wave, no second one
  assert.match(scene, /\$\{cut \? `[^`]*mirrors swayOffset in sway\.js[^`]*\$\{swayGlsl\(SWAY_AMPLITUDE\)\}` : `[^`]*mirrors trunkLean in sway\.js[^`]*\$\{swayGlsl\(TRUNK_SWAY_AMPLITUDE\)\}`\}/);
});

test('sway shader: emitted wave matches swayOffset (cutout) and trunkLean (opaque)', () => {
  for (const wind of WINDS) {
    for (const [w, x, z, p] of [[1, 0, 0, 0], [0.5, 13.5, -7, 2], [0.25, -100, 55, 5], [0, 3, 4, 1]]) {
      const cut = runGlsl(SWAY_AMPLITUDE, w, x, z, p, wind), plant = swayOffset(w, x, z, p, wind);
      const solid = runGlsl(TRUNK_SWAY_AMPLITUDE, w, x, z, p, wind), lean = trunkLean(w, x, z, p, wind);
      for (let i = 0; i < 2; i++) {
        assert.ok(Math.abs(cut[i] - plant[i]) < 1e-4, `cutout ${cut} vs ${plant}`);
        assert.ok(Math.abs(solid[i] - lean[i]) < 1e-4, `opaque ${solid} vs ${lean}`);
      }
    }
  }
});

test('sway shader: every program gets the wind uniforms each frame; reduced motion zeroes dir and phase', () => {
  const bind = scene.match(/const bind = \(prog, a\) => \{[\s\S]*?\n {2}\};/)[0];
  assert.match(bind, /uniform1f\(prog\.u\('swayPhase'\), wind\.phase\)/);
  assert.match(bind, /uniform2fv\(prog\.u\('windDir'\), wind\.dir\)/);
  assert.match(bind, /uniform1f\(prog\.u\('windStrength'\), wind\.strength\)/);
  assert.match(scene, /const wind = windState\.frame\(eye\[0\], eye\[2\]/); // one query per frame, at the eye
  assert.match(scene, /if \(reducedMotion\.matches\) wind\.dir = \[0, 0\];/);
  assert.match(scene, /bind\(solidProg, 1\); drawParts\(solidProg, meshes\.opaque\);/);
  // zero dir and phase: no motion at any weight
  assert.ok(runGlsl(SWAY_AMPLITUDE, 1, 7, 9, 0, { dir: [0, 0], strength: 1 }).every((d) => d === 0));
  assert.equal(swayPhase(5000, true), 0);
});

const wind = (dir, strength) => ({ dir, strength });
const EAST = wind([1, 0], 1), WEST = wind([-1, 0], 1);

test('sway: leans along dir, and reversing dir reverses the lean', () => {
  for (const [x, z, p] of [[0, 0, 1], [13.5, -7, 2], [-100, 55, 5]]) {
    const [ex, ez] = swayOffset(1, x, z, p, EAST), [wx, wz] = swayOffset(1, x, z, p, WEST);
    assert.equal(Math.abs(ez), 0);
    const [nx, nz] = swayOffset(1, x, z, p, wind([0, 1], 1));
    assert.equal(Math.abs(nx), 0);
    assert.ok(Math.abs(nz) <= SWAY_AMPLITUDE);
  }
  // at the origin the wave is sin(phase) either way, so the lean is exactly opposite
  const [ex] = swayOffset(1, 0, 0, 1, EAST), [wx] = swayOffset(1, 0, 0, 1, WEST);
  assert.ok(ex > 0 && Math.abs(ex + wx) < 1e-12);
});

test('sway: the wave travels along dir (coordinate is position dotted with dir)', () => {
  const d = [0.6, 0.8], w = wind(d, 1);
  const base = swayOffset(1, 0, 0, 0.3, w);
  // moving perpendicular to dir keeps the wave coordinate
  const side = swayOffset(1, 0.8 * 5, -0.6 * 5, 0.3, w);
  assert.ok(Math.abs(base[0] - side[0]) < 1e-12 && Math.abs(base[1] - side[1]) < 1e-12);
  // moving one wavelength along dir does too
  const L = 2 * Math.PI / SWAY_WAVE, far = swayOffset(1, d[0] * L, d[1] * L, 0.3, w);
  assert.ok(Math.abs(base[0] - far[0]) < 1e-9 && Math.abs(base[1] - far[1]) < 1e-9);
});

test('sway: strength scales the amplitude from the floor; zero dir is still', () => {
  const peak = (s) => Math.hypot(...swayOffset(1, 0, 0, Math.PI / 2, wind([1, 0], s)));
  assert.ok(Math.abs(peak(1) - SWAY_AMPLITUDE) < 1e-12);
  assert.ok(Math.abs(peak(0) - SWAY_AMPLITUDE * SWAY_FLOOR) < 1e-12);
  assert.ok(peak(0.25) < peak(0.5) && peak(0.5) < peak(1));
  assert.ok(swayOffset(1, 3, 4, 1, wind([0, 0], 1)).every((d) => d === 0));
});

test('sway: weight 0 stays planted and offsets stay bounded for any wind', () => {
  for (const d of [[1, 0], [0, -1], [0.6, 0.8], [0, 0]]) {
    for (const s of [0, 0.5, 1]) {
      for (let p = 0; p < 7; p += 0.5) {
        assert.ok(swayOffset(0, 5, 9, p, wind(d, s)).every((v) => v === 0));
        assert.ok(Math.hypot(...swayOffset(1, 5, 9, p, wind(d, s))) <= SWAY_AMPLITUDE + 1e-12);
        assert.ok(Math.hypot(...trunkLean(1, 5, 9, p, wind(d, s))) <= TRUNK_SWAY_AMPLITUDE + 1e-12);
      }
    }
  }
});
