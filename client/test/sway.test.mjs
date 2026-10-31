import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { swayOffset, swayPhase, trunkLean, SWAY_AMPLITUDE, SWAY_WAVE, TRUNK_SWAY_AMPLITUDE } from '../sway.js';

test('sway: zero at weight 0, bounded by the amplitude', () => {
  for (const [x, z, p] of [[0, 0, 0], [13.5, -7, 2], [-100, 55, 5]]) {
    assert.ok(swayOffset(0, x, z, p).every((d) => d === 0));
    const [dx, dz] = swayOffset(1, x, z, p);
    assert.ok(Math.abs(dx) <= SWAY_AMPLITUDE && Math.abs(dz) <= SWAY_AMPLITUDE);
  }
});

test('sway: varies with time and world position, scales with weight', () => {
  assert.notDeepEqual(swayOffset(1, 3, 4, 0), swayOffset(1, 3, 4, 1));
  assert.notDeepEqual(swayOffset(1, 3, 4, 0), swayOffset(1, 9, 4, 0));
  const [fx] = swayOffset(1, 3, 4, 0.5), [hx] = swayOffset(0.5, 3, 4, 0.5);
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
  for (const [x, z, p] of [[0, 0, still], [13.5, -7, 2], [-100, 55, 5]]) {
    assert.ok(trunkLean(0, x, z, p).every((d) => d === 0));
    const mag = (w) => Math.hypot(...trunkLean(w, x, z, p));
    assert.ok(mag(0.25) < mag(0.5) && mag(0.5) < mag(1));
    const k = TRUNK_SWAY_AMPLITUDE / SWAY_AMPLITUDE;
    const [lx, lz] = trunkLean(1, x, z, p), [px, pz] = swayOffset(1, x, z, p);
    assert.ok(Math.abs(lx - px * k) < 1e-12 && Math.abs(lz - pz * k) < 1e-12);
  }
});

test('trunk lean: at weight 1 never exceeds half a texel (0.03 blocks)', () => {
  let max = 0;
  for (let x = -40; x <= 40; x += 0.5) {
    for (let z = -40; z <= 40; z += 0.5) {
      for (let p = 0; p < 2 * Math.PI; p += 0.25) max = Math.max(max, Math.hypot(...trunkLean(1, x, z, p)));
    }
  }
  assert.ok(max <= 0.03, `max lean ${max}`);
  assert.ok(Math.SQRT2 * TRUNK_SWAY_AMPLITUDE <= 0.03); // bound on any (x, z, phase)
});

// The vertex shader in scene.js, read as text (it needs WebGL to run).
const scene = readFileSync(new URL('../scene.js', import.meta.url), 'utf8');
const swayGlsl = new Function('SWAY_WAVE', `return ${scene.match(/const swayGlsl = (\(amplitude\) => `[^`]*`);/)[1]};`)(SWAY_WAVE);

/** Runs the GLSL of `swayGlsl(amplitude)` as JS for a vertex at world (x, z) with weight `top`: its [dx, dz]. */
function runGlsl(amplitude, top, x, z, swayPhase) {
  const js = swayGlsl(amplitude).replace('float k', 'const k')
    .replace(/w\.xz \+= (.*) \* vec2\((.*)\);$/, 'return vec2($2).map((d) => $1 * d);')
    .replace(/\b(sin|cos)\(/g, 'Math.$1(');
  return new Function('w', 'top', 'swayPhase', 'vec2', js)({ x, z }, top, swayPhase, (a, b) => [a, b]);
}

test('sway shader: cutout program emits the plant sway unchanged, opaque program the trunk lean', () => {
  const line = (a) => `float k = ${SWAY_WAVE.toFixed(3)};\n      w.xz += ${a} * top * vec2(sin(swayPhase + (w.x + 0.6 * w.z) * k), cos(swayPhase + (w.z - 0.6 * w.x) * k));`;
  assert.equal(swayGlsl(SWAY_AMPLITUDE), line(SWAY_AMPLITUDE.toFixed(3)));
  assert.equal(swayGlsl(TRUNK_SWAY_AMPLITUDE), line(TRUNK_SWAY_AMPLITUDE.toFixed(3)));
  assert.equal(+TRUNK_SWAY_AMPLITUDE.toFixed(3), TRUNK_SWAY_AMPLITUDE); // no rounding in the shader constant
  assert.match(scene, /\$\{cut \? `[^`]*mirrors swayOffset in sway\.js[^`]*\$\{swayGlsl\(SWAY_AMPLITUDE\)\}` : `[^`]*mirrors trunkLean in sway\.js[^`]*\$\{swayGlsl\(TRUNK_SWAY_AMPLITUDE\)\}`\}/);
});

test('sway shader: emitted wave matches swayOffset (cutout) and trunkLean (opaque)', () => {
  for (const [w, x, z, p] of [[1, 0, 0, 0], [0.5, 13.5, -7, 2], [0.25, -100, 55, 5], [0, 3, 4, 1]]) {
    const cut = runGlsl(SWAY_AMPLITUDE, w, x, z, p), plant = swayOffset(w, x, z, p);
    const solid = runGlsl(TRUNK_SWAY_AMPLITUDE, w, x, z, p), lean = trunkLean(w, x, z, p);
    // SWAY_WAVE is written to 3 places in the shader
    for (let i = 0; i < 2; i++) {
      assert.ok(Math.abs(cut[i] - plant[i]) < 1e-3, `cutout ${cut} vs ${plant}`);
      assert.ok(Math.abs(solid[i] - lean[i]) < 1e-3, `opaque ${solid} vs ${lean}`);
    }
  }
});

test('sway shader: every program gets swayPhase each frame, 0 under reduced motion', () => {
  const bind = scene.match(/const bind = \(prog, a\) => \{[\s\S]*?\n {2}\};/)[0];
  assert.match(bind, /\n {4}gl\.uniform1f\(prog\.u\('swayPhase'\), swayPhase\(now, reducedMotion\.matches\)\);/);
  assert.match(scene, /bind\(solidProg, 1\); drawParts\(solidProg, meshes\.opaque\);/);
  assert.equal(swayPhase(5000, true), 0);
  assert.ok(runGlsl(TRUNK_SWAY_AMPLITUDE, 1, 7, 9, swayPhase(5000, true)).every((d, i) => Math.abs(d - trunkLean(1, 7, 9, 0)[i]) < 1e-3));
});
