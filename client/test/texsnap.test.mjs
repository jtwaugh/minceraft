import { test } from 'node:test';
import assert from 'node:assert/strict';
import { snapTexel, SNAP_GLSL, SNAP_SAMPLE_GLSL, TEX_SIZE } from '../texsnap.js';

const TILE = TEX_SIZE;
const FWS = [0.05, 0.1, 0.25, 0.5, 1, 2, 4];
const texel = (px, fw) => Math.floor(snapTexel(px, fw));

test("a face's far edge never snaps into the next tile", () => {
  for (const fw of FWS) {
    for (const d of [0.5, 0.25, 0.1, 0.03, 0.01, 0.001]) {
      const t = texel(TILE - d, fw);
      assert.ok(t >= 0 && t <= 15, `px ${TILE - d}, fw ${fw}: texel ${t}`);
    }
  }
});

test("a face's near edge stays in its tile", () => {
  for (const fw of FWS) {
    for (const px of [0, 0.001, 0.03, 0.25]) assert.equal(texel(px, fw), 0, `px ${px}, fw ${fw}`);
  }
});

test('inside a merged quad the snap crosses into the next block', () => {
  for (const fw of [0.05, 0.1, 0.25, 0.5]) {
    assert.equal(texel(TILE + 0.01, fw), TILE, `fw ${fw}`);
    assert.equal(texel(2 * TILE - 0.01, fw), 2 * TILE - 1, `fw ${fw}`);
  }
});

test('magnified, every fragment samples the texel it lies in', () => {
  for (const fw of [0.05, 0.1, 0.25, 0.5, 1]) {
    for (let px = 0; px < 2 * TILE; px += 0.0625 + 0.003) {
      assert.equal(texel(px, fw), Math.floor(px), `px ${px}, fw ${fw}`);
    }
  }
});

test('away from boundaries the snap lands on texel centres', () => {
  assert.equal(snapTexel(3.5, 0.1), 3.5);
  assert.equal(snapTexel(3.2, 0.1), 3.5);
  assert.equal(snapTexel(3.8, 0.1), 3.5);
});

test('the GLSL snippet defines snapTexel', () => {
  assert.match(SNAP_GLSL, /vec2 snapTexel\(vec2 px, vec2 fw\)/);
});

// View angles from grazing (fw large) to head-on close up (fw small), and the
// UV extents a single-block face can have along one axis (full, slab, thin).
const SWEEP_FWS = [0.05, 0.08, 0.125, 0.2, 0.3, 0.5, 0.75, 1, 1.5, 2, 3, 4];
const EXTENTS = [1, 2, 4, 8, 12, 15, 16];
const EDGE = [0, 1e-4, 0.001, 0.01, 0.05, 0.2];

// Every (fw, extent, edge) where `snap` puts a single-block face's texel
// outside its tile, at the near edge (px from 0) or the far edge (px up to extent).
function violations(snap) {
  const out = [];
  for (const fw of SWEEP_FWS) {
    for (const extent of EXTENTS) {
      for (const d of EDGE) {
        for (const px of [d, extent - Math.max(d, 1e-4)]) {
          const t = Math.floor(snap(px, fw));
          if (t < 0 || t > TILE - 1) out.push({ fw, extent, px, t });
        }
      }
    }
  }
  return out;
}

test('a single-block face stays inside its tile at every view angle and extent', () => {
  assert.deepEqual(violations(snapTexel), []);
});

test('the sweep reports the old snap formula, which reaches texel 16 at a far edge', () => {
  const old = (px, fw) => {
    const f = px - Math.floor(px);
    return Math.floor(px) + 0.5 + Math.min(Math.max((f - 0.5) / fw, -0.5), 0.5);
  };
  const bad = violations(old);
  assert.ok(bad.length > 0, 'old formula passed the sweep');
  assert.ok(bad.some((v) => v.extent === TILE && v.t === TILE), JSON.stringify(bad.slice(0, 3)));
});

test('the shader sampling lines are built from TEX_SIZE and call snapTexel with fwidth(px)', () => {
  assert.equal(TEX_SIZE, 16);
  assert.ok(SNAP_SAMPLE_GLSL.includes(`vuv.xy * ${TEX_SIZE}.0`), SNAP_SAMPLE_GLSL);
  assert.ok(SNAP_SAMPLE_GLSL.includes(`snapTexel(px, fwidth(px)) / ${TEX_SIZE}.0`), SNAP_SAMPLE_GLSL);
});
