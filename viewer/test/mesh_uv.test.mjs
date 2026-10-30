import test from 'node:test';
import assert from 'node:assert/strict';
import { meshBlocks as mesh, B, H, VERTEX_BYTES } from '../mesh.js';
const meshBlocks = (c, b, t, s, p = 0, o = {}) => mesh(c, b, t, s, p, { greedy: false, ...o });

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// A flat area of `top` over stone at y 0..59, with `size` columns and `pad` ring.
function flat(top, size, pad, ground = 60) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let c = 0; c < w * w; c++) for (let y = 0; y < ground; y++) blocks[c * H + y] = y === ground - 1 ? top : B.stone;
  return blocks;
}

// UVs of upward faces at ground level, keyed "x,z" (world), as a string of the 4 corners.
function topUVs(m, origin, ground = 60) {
  const dv = new DataView(m.opaque.verts), out = new Map();
  for (let q = 0; q < m.opaque.faces; q++) {
    const o = q * 4 * VERTEX_BYTES;
    const ys = [0, 1, 2, 3].map((j) => dv.getInt16(o + j * VERTEX_BYTES + 2, true) / 16);
    if (!ys.every((y) => y === ground)) continue;
    const x = dv.getInt16(o, true) / 16, z = dv.getInt16(o + 4, true) / 16;
    const uv = [0, 1, 2, 3].map((j) => { const a = o + j * VERTEX_BYTES; return `${dv.getUint8(a + 6)}:${dv.getUint8(a + 7)}`; });
    // corner 0 is the block's min corner; positions are local, so add the origin
    out.set(`${Math.round(x) + origin[0]},${Math.round(z) + origin[1]}`, uv.join(','));
  }
  return out;
}

test('the same world block gets identical UVs from different origins and areas', () => {
  const a = meshBlocks(ctx, flat(B.grass, 8, 1), tints(8), 8, 1, { origin: [100, -40] });
  // second mesh starts 3 columns later and is smaller
  const b = meshBlocks(ctx, flat(B.grass, 4, 1), tints(4), 4, 1, { origin: [103, -40] });
  const ua = topUVs(a, [100, -40]), ub = topUVs(b, [103, -40]);
  assert.equal(ub.size, 16);
  for (const [k, v] of ub) assert.equal(ua.get(k), v, k);
});

test('tops of ground blocks use several variants, dirt uses one', () => {
  for (const top of [B.grass, B.sand, B.gravel, B.stone, B.snow_layer]) {
    const m = meshBlocks(ctx, flat(top, 16, 0), tints(16), 16, 0);
    const kinds = new Set([...topUVs(m, [0, 0], top === B.snow_layer ? 59.125 : 60).values()]);
    assert.ok(kinds.size > 1, `block ${top}`);
  }
  const d = meshBlocks(ctx, flat(B.dirt, 16, 0), tints(16), 16, 0);
  assert.equal(new Set(topUVs(d, [0, 0]).values()).size, 1);
});

test('side faces never flip vertically', () => {
  const blocks = flat(B.grass, 8, 0);
  const m = meshBlocks(ctx, blocks, tints(8), 8, 0);
  const dv = new DataView(m.opaque.verts);
  for (let q = 0; q < m.opaque.faces; q++) {
    const o = q * 4 * VERTEX_BYTES;
    const ys = [0, 1, 2, 3].map((j) => dv.getInt16(o + j * VERTEX_BYTES + 2, true));
    if (new Set(ys).size !== 2) continue; // not a vertical face
    for (let j = 0; j < 4; j++) {
      const a = o + j * VERTEX_BYTES, high = ys[j] === Math.max(...ys);
      assert.equal(dv.getUint8(a + 7), high ? 0 : 16);
    }
  }
});

test('face counts do not depend on the origin', () => {
  const a = meshBlocks(ctx, flat(B.grass, 8, 1), tints(8), 8, 1);
  const b = meshBlocks(ctx, flat(B.grass, 8, 1), tints(8), 8, 1, { origin: [999, -7] });
  assert.equal(a.opaque.faces, b.opaque.faces);
});
