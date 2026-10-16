import { test } from 'node:test';
import assert from 'node:assert/strict';
import { samplerParams, maxLod, createSampler, CUTOUT_MAX_LOD } from '../samplers.js';

const get = (pass, name) => Object.fromEntries(samplerParams(pass))[name];

test('cutout clamps on both axes, opaque and water repeat', () => {
  for (const axis of ['TEXTURE_WRAP_S', 'TEXTURE_WRAP_T']) {
    assert.equal(get('cutout', axis), 'CLAMP_TO_EDGE');
    assert.equal(get('opaque', axis), 'REPEAT');
    assert.equal(get('water', axis), 'REPEAT');
  }
});

test('both samplers keep nearest mag and trilinear min', () => {
  for (const pass of ['opaque', 'water', 'cutout']) {
    assert.equal(get(pass, 'TEXTURE_MAG_FILTER'), 'NEAREST');
    assert.equal(get(pass, 'TEXTURE_MIN_FILTER'), 'LINEAR_MIPMAP_LINEAR');
  }
});

test('max LOD is set only on cutout, at the 4x4 level', () => {
  assert.equal(maxLod('cutout'), 2);
  assert.equal(CUTOUT_MAX_LOD, 2);
  assert.equal(maxLod('opaque'), null);
  assert.equal(maxLod('water'), null);
});

test('createSampler applies the table to a context', () => {
  const calls = [];
  const gl = new Proxy({
    createSampler: () => 'S',
    samplerParameteri: (s, p, v) => calls.push(['i', p, v]),
    samplerParameterf: (s, p, v) => calls.push(['f', p, v]),
    getExtension: () => null,
  }, { get: (t, k) => (k in t ? t[k] : String(k)) });
  createSampler(gl, 'cutout');
  assert.deepEqual(calls.filter((c) => c[1] === 'TEXTURE_WRAP_S'), [['i', 'TEXTURE_WRAP_S', 'CLAMP_TO_EDGE']]);
  assert.deepEqual(calls.filter((c) => c[1] === 'TEXTURE_MAX_LOD'), [['f', 'TEXTURE_MAX_LOD', 2]]);
  calls.length = 0;
  createSampler(gl, 'opaque');
  assert.equal(calls.some((c) => c[1] === 'TEXTURE_MAX_LOD'), false);
});
