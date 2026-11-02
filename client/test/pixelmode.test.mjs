import test from 'node:test';
import assert from 'node:assert/strict';
import { parsePixelMode, DEFAULT_POSE, DEFAULT_VIEW, PIXEL_UNIFORMS } from '../pixelmode.js';

test('no pixel=1, no pixel mode', () => {
  assert.equal(parsePixelMode(''), null);
  assert.equal(parsePixelMode('?pixel=0'), null);
  assert.equal(parsePixelMode('?pose=1,2,3,0,0'), null);
});

test('defaults fill in', () => {
  assert.deepEqual(parsePixelMode('?pixel=1'), { pose: DEFAULT_POSE, width: DEFAULT_VIEW[0], height: DEFAULT_VIEW[1] });
});

test('overrides', () => {
  assert.deepEqual(parsePixelMode('?seed=3&pixel=1&pose=1,2.5,-3,0.5,-0.25&view=320x200'), { pose: [1, 2.5, -3, 0.5, -0.25], width: 320, height: 200 });
});

test('malformed pose or view is rejected', () => {
  for (const bad of ['pose=1,2,3', 'pose=1,2,3,4,x', 'pose=1,2,,4,5', 'pose=', 'view=640', 'view=0x10', 'view=axb', 'view=9999x10', 'view=-1x5']) {
    assert.throws(() => parsePixelMode(`?pixel=1&${bad}`), /bad (pose|view)/, bad);
  }
});

test('forced uniforms switch fog, tint and daylight off', () => {
  const u = PIXEL_UNIFORMS;
  assert.equal(u.fogFar, 0);
  assert.ok((0 - u.fogStart) / (u.fogEnd - u.fogStart) <= 0 && (1000 - u.fogStart) / (u.fogEnd - u.fogStart) < 0);
  assert.deepEqual(u.tint, [1, 1, 1]);
  assert.equal(u.daylight, 1);
  assert.equal(u.biomeTint, 0);
});
