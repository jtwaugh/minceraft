import test from 'node:test';
import assert from 'node:assert/strict';
import { NAMES, TINTED, VARIANTS, REQUIRED } from '../../viewer/tilelist.js';

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
      assert.equal(NAMES[at + i], `${base}_${i}`);
      assert.equal(TINTED[`${base}_${i}`], TINTED[base], `${base}_${i} tint`);
    }
  }
});
