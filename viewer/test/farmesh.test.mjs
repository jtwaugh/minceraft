import test from 'node:test';
import assert from 'node:assert/strict';
import { farMesh, FAR_SEA } from '../farmesh.js';

function makeGrid(x0, z0, nx, nz, step = 16) {
  const top = new Uint8Array(nx * nz), biome = new Uint8Array(nx * nz);
  for (let k = 0; k < nz; k++) for (let i = 0; i < nx; i++) {
    const wx = x0 + i * step, wz = z0 + k * step;
    top[k * nx + i] = 40 + ((wx * 7 + wz * 13) / step & 63) + (wx > 100 ? 40 : 0);
    biome[k * nx + i] = (i + k) & 3;
  }
  return { x0, z0, step, nx, nz, top, biome };
}

test('quad and index counts', () => {
  const m = farMesh(makeGrid(0, 0, 5, 4));
  assert.equal(m.surfaceQuads, 4 * 3);
  assert.equal(m.skirtQuads, 2 * 4 + 2 * 3);
  assert.equal(m.indices.length, (m.surfaceQuads + m.skirtQuads) * 6);
  assert.equal(m.positions.length / 3, m.biome.length);
  assert.ok(m.indices.every((i) => i < m.biome.length));
});

test('skirt bottoms are below every surface vertex', () => {
  const g = makeGrid(0, 0, 5, 4);
  const m = farMesh(g);
  const n = g.nx * g.nz;
  let lo = Infinity;
  for (let v = 0; v < n; v++) lo = Math.min(lo, m.positions[v * 3 + 1]);
  for (let v = n; v < m.positions.length / 3; v++) assert.ok(m.positions[v * 3 + 1] < lo);
});

test('origin offset, survey heights and flat sea', () => {
  const g = makeGrid(32, -48, 4, 4);
  g.top[5] = 30;
  const m = farMesh(g, { x: 10, y: 5, z: -8 });
  for (let c = 0; c < 16; c++) {
    assert.equal(m.positions[c * 3], g.x0 + (c % 4) * 16 - 10);
    assert.equal(m.positions[c * 3 + 2], g.z0 + Math.floor(c / 4) * 16 + 8);
    assert.equal(m.positions[c * 3 + 1], Math.max(FAR_SEA, g.top[c]) - 5);
    assert.equal(m.top[c], g.top[c]);
    assert.equal(m.biome[c], g.biome[c]);
  }
  assert.equal(m.positions[5 * 3 + 1], FAR_SEA - 5);
});

test('overlapping grids agree on a shared world column', () => {
  const a = farMesh(makeGrid(0, 0, 6, 6)), b = farMesh(makeGrid(32, 32, 6, 6));
  // world (48, 48): a index (3,3), b index (1,1)
  assert.equal(a.positions[(3 * 6 + 3) * 3 + 1], b.positions[(1 * 6 + 1) * 3 + 1]);
  assert.equal(a.positions[(3 * 6 + 3) * 3], b.positions[(1 * 6 + 1) * 3]);
});
