import test from 'node:test';
import assert from 'node:assert/strict';
import { NAMES, TINTED, VARIANTS, REQUIRED, variantName } from '../../viewer/tilelist.js';

test('names are unique and required tiles are listed', () => {
  assert.equal(new Set(NAMES).size, NAMES.length);
  for (const n of REQUIRED) assert.ok(NAMES.includes(n), n);
});

test('variant runs are contiguous and weights sum to 1', () => {
  for (const [base, { count, weights }] of Object.entries(VARIANTS)) {
    const at = NAMES.indexOf(base);
    assert.ok(at >= 0, base);
    assert.equal(weights.length, count);
    assert.ok(Math.abs(weights.reduce((a, b) => a + b, 0) - 1) < 1e-9, base);
    for (let i = 1; i < count; i++) {
      const name = variantName(base, i);
      assert.equal(NAMES[at + i], name);
      assert.equal(TINTED[name], TINTED[base], `${name} tint`);
    }
  }
});

test('variant names follow the base: _1 after a plain base, _0 runs count on', () => {
  assert.deepEqual([0, 1, 2].map((i) => variantName('grass_top', i)), ['grass_top', 'grass_top_1', 'grass_top_2']);
  assert.deepEqual([0, 1, 2].map((i) => variantName('floe_ice_0', i)), ['floe_ice_0', 'floe_ice_1', 'floe_ice_2']);
});
