import test from 'node:test';
import assert from 'node:assert/strict';
import { meshBlocks, B, H, VERTEX_BYTES } from '../mesh.js';

const ctx = { COLORS: [], faceLayer: new Uint8Array(256 * 3 * 2) };
const tints = (n) => ({ grass: Array(n * n).fill([1, 1, 1]), leaf: Array(n * n).fill([1, 1, 1]) });

// Column world (wx, wz) gets stone up to `floor` (exclusive), then water for `depth(wx, wz)` blocks.
function area(depth, origin, size, pad, floor = 2) {
  const w = size + 2 * pad, blocks = new Uint8Array(w * w * H);
  for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
    const d = depth(origin[0] + x - pad, origin[1] + z - pad), i = (z * w + x) * H;
    for (let y = 0; y < floor; y++) blocks[i + y] = B.stone;
    for (let y = floor; y < floor + d; y++) blocks[i + y] = B.water;
  }
  return blocks;
}
const mesh = (blocks, size, pad, origin = [0, 0]) => meshBlocks(ctx, blocks, tints(size), size, pad, { origin });
// Distinct byte-15 values per vertex of a mesh part.
const depths = (part) => {
  const dv = new DataView(part.verts), out = new Set();
  for (let i = 0; i < part.faces * 4; i++) out.add(dv.getUint8(i * VERTEX_BYTES + 15));
  return out;
};

test('single-deep water carries depth 1', () => {
  const m = mesh(area(() => 1, [0, 0], 2, 0), 2, 0);
  assert.ok(m.water.faces > 0);
  assert.deepEqual([...depths(m.water)], [1]);
});

test('multi-deep water: top and side faces carry the depth of their block', () => {
  const m = mesh(area(() => 5, [0, 0], 2, 0), 2, 0);
  // top block is 5 deep; side faces of lower blocks are culled by pad 0 air only at the edge, so all depths 1..5 appear
  assert.deepEqual([...depths(m.water)].sort(), [1, 2, 3, 4, 5]);
  const tops = (() => {
    const dv = new DataView(m.water.verts), s = new Set();
    for (let i = 0; i < m.water.faces * 4; i++) {
      const o = i * VERTEX_BYTES;
      if (dv.getInt16(o + 2, true) === (2 + 5) * 16) s.add(dv.getUint8(o + 15));
    }
    return s;
  })();
  assert.deepEqual([...tops], [5]);
});

test('a full-height column reaches 127 (the cap of 255 is above the world height)', () => {
  const m = mesh(area(() => H, [0, 0], 2, 0, 1), 2, 0);
  assert.equal(Math.max(...depths(m.water)), H - 1);
});

test('non-water meshes have byte 15 = 0', () => {
  const m = mesh(area(() => 3, [0, 0], 2, 0), 2, 0);
  assert.ok(m.opaque.faces > 0);
  assert.deepEqual([...depths(m.opaque)], [0]);
  assert.deepEqual([...depths(m.cutout)], []);
  assert.deepEqual([...depths(m.ice)], []);
});

test('overlapping regions agree on byte 15', () => {
  const depth = (x, z) => 1 + ((x * 7 + z * 3) & 7);
  const a = mesh(area(depth, [0, 0], 4, 1), 4, 1, [0, 0]);
  const b = mesh(area(depth, [2, 0], 4, 1), 4, 1, [2, 0]);
  // Quads lying wholly within the shared columns 2-3, keyed by their vertices.
  const key = (m, ox) => {
    const dv = new DataView(m.water.verts), s = new Map();
    for (let q = 0; q < m.water.faces; q++) {
      const vs = [0, 1, 2, 3].map((j) => {
        const o = (q * 4 + j) * VERTEX_BYTES;
        return { p: [dv.getInt16(o, true) / 16 + ox, dv.getInt16(o + 2, true), dv.getInt16(o + 4, true) / 16], d: dv.getUint8(o + 15) };
      });
      if (vs.some((v) => v.p[0] < 2 || v.p[0] > 4 || v.p[2] < 0 || v.p[2] > 4)) continue;
      if (vs.every((v) => v.p[0] === 2 || v.p[0] === 4) && vs[0].p[0] === vs[1].p[0] && vs.every((v) => v.p[0] === vs[0].p[0])) continue;
      s.set(vs.map((v) => v.p.join()).sort().join(';'), vs.map((v) => v.d).join());
    }
    return s;
  };
  const ka = key(a, 0), kb = key(b, 2);
  assert.ok(ka.size > 0);
  for (const [k, v] of ka) if (kb.has(k)) assert.equal(kb.get(k), v);
});
