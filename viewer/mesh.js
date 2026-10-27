// Block meshing shared by the 3D view (main thread) and the flyover chunk
// workers. Produces quads (4 vertices each) in a compact 16-byte vertex
// format, drawn with one shared index buffer (quadIndices):
//
//   bytes 0-5   position x, y, z  int16, in 1/16 block (POS_SCALE)
//   bytes 6-7   uv                uint8, in 1/16 (UV_SCALE)
//   byte  8     texture layer     uint8
//   byte  9     shade             uint8, shade * 128 (SHADE_SCALE)
//   bytes 10-12 tint rgb          uint8, normalized
//   byte  13    ambient occlusion level 0-3 (AO_SHADE): the opaque blocks among
//               the corner's two side neighbours and diagonal on the open side
//               of full-cube faces; 0 elsewhere. The viewer shader reads it.
//   byte  14    plant top weight  uint8: 255 on the top vertices of cross
//               plants, 0 at their base and on every other face (the client
//               fades and sways with it; the viewer ignores it)
//   byte  15    padding
//
// Positions are local to the meshed area: x and z from 0 to size, y the
// world y. The renderer adds a per-mesh offset.

export const H = 128;
export const VERTEX_BYTES = 16;
export const POS_SCALE = 1 / 16, UV_SCALE = 1 / 16, SHADE_SCALE = 1 / 128;
// Shade multiplier per AO level (vertex byte 13).
export const AO_SHADE = [1.0, 0.82, 0.68, 0.55];

// Block ids (data/blocks.toml); names and colours come from legend().
export const B = {
  air: 0, stone: 1, dirt: 2, grass: 3, sand: 4, gravel: 5, water: 6, ice: 7,
  oak_log: 8, birch_log: 9, spruce_log: 10, oak_leaves: 11, birch_leaves: 12, spruce_leaves: 13,
  tall_grass: 14, fern: 15, dead_bush: 16, dandelion: 17, rose: 18, sugar_cane: 19,
  cactus: 20, brown_mushroom: 21, red_mushroom: 22, pumpkin: 23, snow_layer: 24,
  raft_planks: 25, butcher_block: 26, griddle: 27, crate: 28,
  farmland_dry: 29, farmland_wet: 30,
  wheat_0: 31, wheat_1: 32, wheat_2: 33, wheat_3: 34, wheat_4: 35, wheat_5: 36, wheat_6: 37, wheat_7: 38,
  planter_dirt: 39, planter_farmland_dry: 40, planter_farmland_wet: 41,
};

const flags = (ids) => { const a = new Uint8Array(256); for (const b of ids) a[b] = 1; return a; };
// Full cubes that hide the faces of whatever is next to them.
const SOLID = [B.stone, B.dirt, B.grass, B.sand, B.gravel, B.oak_log, B.birch_log, B.spruce_log, B.pumpkin,
  B.raft_planks, B.butcher_block, B.griddle, B.crate, B.farmland_dry, B.farmland_wet,
  B.planter_dirt, B.planter_farmland_dry, B.planter_farmland_wet];
export const OPAQUE = flags(SOLID);
export const LEAVES = flags([B.oak_leaves, B.birch_leaves, B.spruce_leaves]);
// With fast leaves, leaves count as solid too.
const OPAQUE_FAST = flags([...SOLID, B.oak_leaves, B.birch_leaves, B.spruce_leaves]);
// Drawn as two crossed quads.
export const CROSS = flags([B.tall_grass, B.fern, B.dead_bush, B.dandelion, B.rose, B.sugar_cane, B.brown_mushroom, B.red_mushroom,
  B.wheat_0, B.wheat_1, B.wheat_2, B.wheat_3, B.wheat_4, B.wheat_5, B.wheat_6, B.wheat_7]);

/**
 * Meshing context: names, colours and biomes from the legend() JSON, plus
 * `faceLayer`, the texture layer of every block face (see faceLayers). The
 * page owns the texture array, so it builds the table and hands it over.
 * `variants` (see variantTable) lets grass tops and cross plants pick one of
 * several tiles per block; without it every block uses its base layer.
 */
export function legendContext(json, faceLayer = null, variants = null) {
  const legend = JSON.parse(json);
  const COLORS = [], NAMES = [], BIOMES = [];
  for (const b of legend.blocks) { COLORS[b.id] = b.color; NAMES[b.id] = b.name; }
  for (const b of legend.biomes) BIOMES[b.id] = b;
  return { legend, COLORS, NAMES, BIOMES, faceLayer, variants };
}

/**
 * Face-to-layer table from a tileFor(block, dy) function: entry
 * block * 3 + (0 top, 1 side, 2 bottom). Entries from 768 on are the same
 * for fast leaves (tileFor(block, dy, true)).
 */
export function faceLayers(tileFor) {
  const t = new Uint8Array(256 * 3 * 2);
  for (let k = 0; k < 2; k++) for (let b = 0; b < 256; b++) {
    const o = k * 768 + b * 3;
    t[o] = tileFor(b, 1, !!k); t[o + 1] = tileFor(b, 0, !!k); t[o + 2] = tileFor(b, -1, !!k);
  }
  return t;
}
/**
 * Per-layer variant table from the page's layer names and tilelist's VARIANTS:
 * { [base layer]: { count, cum } }, `cum` the cumulative weights (last is 1).
 * Variant i of a base lives at layer base + i (the tile list keeps runs contiguous).
 */
export function variantTable(layerNames, variants) {
  const out = {};
  for (const [base, { count, weights }] of Object.entries(variants)) {
    const layer = layerNames.indexOf(base);
    if (layer < 0) continue;
    let sum = 0;
    out[layer] = { count, cum: weights.map((w) => (sum += w)) };
  }
  return out;
}
const layerOf = (ctx, b, dy, fast = false) => ctx.faceLayer[(fast ? 768 : 0) + b * 3 + (dy > 0 ? 0 : dy < 0 ? 2 : 1)];

/**
 * Indices for `quads` quads of 4 vertices (two triangles each). Every mesh
 * uses the same pattern, so the renderer keeps one buffer of these for all.
 */
export function quadIndices(quads) {
  const idx = new Uint32Array(quads * 6);
  for (let q = 0; q < quads; q++) {
    const v = q * 4, o = q * 6;
    idx[o] = v; idx[o + 1] = v + 1; idx[o + 2] = v + 2; idx[o + 3] = v; idx[o + 4] = v + 2; idx[o + 5] = v + 3;
  }
  return idx;
}

export const lerp3 = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
// Biome ramps have three stops at 0, 0.5 and 1.
export const ramp3 = (stops, v) => v < 0.5 ? lerp3(stops[0], stops[1], v * 2) : lerp3(stops[1], stops[2], (v - 0.5) * 2);

/**
 * Box blur of `n` x n interleaved 3-channel integer data, window radius `r`
 * (clipped at the edges), as two separable running-sum passes. Integer
 * input keeps the sums exact, so a column's value depends only on its window.
 * Returns the window sums (not yet divided).
 */
function boxSum(src, n, r) {
  const pass = (a, stride, step) => {
    const out = new Float64Array(a.length);
    for (let line = 0; line < n; line++) {
      const base = line * stride;
      for (let ch = 0; ch < 3; ch++) {
        let s = 0;
        for (let i = 0; i < Math.min(r, n - 1) + 1; i++) s += a[base + i * step + ch];
        for (let i = 0; i < n; i++) {
          out[base + i * step + ch] = s;
          if (i + r + 1 < n) s += a[base + (i + r + 1) * step + ch];
          if (i - r >= 0) s -= a[base + (i - r) * step + ch];
        }
      }
    }
    return out;
  };
  return pass(pass(src, 3 * n, 3), 3, 3 * n);
}

/**
 * Per-column climate tints for a size x size area at world (wx, wz): grass
 * leaf and water ramps by humidity, box-blurred over a (2R+1)-wide window so they
 * don't jump at biome borders. `R` defaults to the legend's blend radius.
 * `wasm` needs biome_map, humidity_map and temperature_map.
 *
 * Also returns `padded`: the blurred grass and leaf tints (Float32Array,
 * 3 per column) for the area plus a ring of `pad` = R + 1 columns, `n` wide.
 * Columns within one of the area are exact; the outermost ring is not.
 *
 * And `corners`: the tint of each of the (size + 1)^2 block corners, the mean
 * of the 4 columns that meet there (`grass`, `leaf` and `water`, Float32Array, 3 per
 * corner, row z * (size + 1) + x). Edge corners use the padded ring, so
 * areas side by side agree on the corners they share.
 */
export function columnTints(wasm, ctx, seed, wx, wz, size, R = ctx.legend.blend.radius) {
  const pad = R + 1, n = size + 2 * pad;
  const bigBio = wasm.biome_map(seed, wx - pad, wz - pad, n, n);
  const hum = wasm.humidity_map(seed, wx - pad, wz - pad, n, n);
  const tmp = wasm.temperature_map(seed, wx, wz, size, size);
  const bio = new Uint8Array(size * size);
  const rawG = new Float64Array(n * n * 3), rawL = new Float64Array(n * n * 3), rawW = new Float64Array(n * n * 3);
  // Whole quarter-steps of 0-255 when blurring, so the sums are exact.
  const q = R > 0 ? (v) => Math.round(v * 4) : (v) => v / 255;
  for (let c = 0; c < n * n; c++) {
    const bi = ctx.BIOMES[bigBio[c]];
    const g = ramp3(bi.grass, hum[c]), l = ramp3(bi.leaf, hum[c]), w = ramp3(bi.water, hum[c]);
    for (let k = 0; k < 3; k++) { rawG[c * 3 + k] = q(g[k]); rawL[c * 3 + k] = q(l[k]); rawW[c * 3 + k] = q(w[k]); }
  }
  let padG = rawG, padL = rawL, padW = rawW;
  if (R > 0) {
    const area = (2 * R + 1) * (2 * R + 1), scale = 4 * 255 * area;
    const norm = (a) => Float32Array.from(boxSum(a, n, R), (v) => v / scale);
    padG = norm(rawG); padL = norm(rawL); padW = norm(rawW);
  }
  const grass = [], leaf = [], water = [];
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    const p = ((z + pad) * n + x + pad) * 3;
    bio[z * size + x] = bigBio[(z + pad) * n + x + pad];
    grass.push([padG[p], padG[p + 1], padG[p + 2]]);
    leaf.push([padL[p], padL[p + 1], padL[p + 2]]);
    water.push([padW[p], padW[p + 1], padW[p + 2]]);
  }
  const cn = size + 1;
  const corner = (a) => {
    const out = new Float32Array(cn * cn * 3);
    for (let z = 0; z < cn; z++) for (let x = 0; x < cn; x++) for (let k = 0; k < 3; k++) {
      const p = ((z + pad - 1) * n + x + pad - 1) * 3 + k;
      out[(z * cn + x) * 3 + k] = (a[p] + a[p + 3] + a[p + n * 3] + a[p + n * 3 + 3]) / 4;
    }
    return out;
  };
  const mid = (size >> 1) * size + (size >> 1);
  return {
    corners: { n: cn, grass: corner(padG), leaf: corner(padL), water: corner(padW) },
    grass, leaf, water, bio, tmp, centre: { biome: ctx.BIOMES[bio[mid]], t: tmp[mid] },
    padded: { pad, n, grass: padG, leaf: padL, water: padW },
  };
}

const WHITE = [1, 1, 1];
function tintFor(ctx, b, grass, leaf, water) {
  switch (b) {
    case B.grass: case B.tall_grass: return grass;
    case B.fern: return grass.map((v) => v * 0.88);
    case B.water: return water ?? WHITE;
    case B.oak_leaves: return leaf;
    case B.birch_leaves: return lerp3(leaf, ctx.COLORS[B.birch_leaves].map((v) => v / 255), 0.55);
    case B.spruce_leaves: return lerp3(leaf, ctx.COLORS[B.spruce_leaves].map((v) => v / 255), 0.7);
    default: return WHITE;
  }
}

// Faces: [normal, 4 corner offsets, shade]
const FACES = [
  { d: [0, 1, 0], c: [[0,1,0],[0,1,1],[1,1,1],[1,1,0]], s: 1.0 },   // top
  { d: [0, -1, 0], c: [[0,0,0],[1,0,0],[1,0,1],[0,0,1]], s: 0.5 },  // bottom
  { d: [1, 0, 0], c: [[1,0,0],[1,1,0],[1,1,1],[1,0,1]], s: 0.8 },   // +x
  { d: [-1, 0, 0], c: [[0,0,0],[0,0,1],[0,1,1],[0,1,0]], s: 0.65 }, // -x
  { d: [0, 0, 1], c: [[0,0,1],[1,0,1],[1,1,1],[0,1,1]], s: 0.9 },   // +z
  { d: [0, 0, -1], c: [[0,0,0],[0,1,0],[1,1,0],[1,0,0]], s: 0.6 },  // -z
];

// Texture coords for a face corner: top/bottom map x,z; sides map the
// horizontal axis to u and put the tile's top row at the top of the block.
// `v` is the block's variant (0-7; 0 leaves the tile as drawn). Tops use all eight
// rotations and mirrors (bit 0 swaps u and v, bit 1 flips u, bit 2 flips v);
// sides only mirror horizontally (bit 1), so they never turn or flip upside
// down. Bottoms stay as they are.
function faceUV(f, [a, b, c], v) {
  if (f.d[1] > 0) {
    let u = a, w = c;
    if (v & 1) { u = c; w = a; }
    return [v & 2 ? 1 - u : u, v & 4 ? 1 - w : w];
  }
  if (f.d[1] < 0) return [a, c];
  const u = f.d[0] !== 0 ? c : a;
  return [v & 2 ? 1 - u : u, 1 - b];
}

// Blocks whose tiles vary from block to block.
const VARIES = flags([B.grass, B.sand, B.gravel, B.stone, B.snow_layer]);
// Variant 0-7 from the world coordinate alone (an integer hash), so it
// doesn't depend on the chunk, pad or area being meshed.
function hashAt(x, y, z, salt) {
  let h = Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1) ^ Math.imul(z, 0x9e3779b1) ^ salt;
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b);
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
  return h ^ (h >>> 16);
}
const variantAt = (x, y, z) => hashAt(x, y, z, 0) & 7;
// Cross-plant shift in x and z, in whole 1/16s from -2 to 2 (salted apart from
// the variant hash). The quads' vertices quantise to 2/16 and 14/16, so this
// is the widest range that keeps them inside their column.
const plantOffset = (x, y, z) => {
  const h = hashAt(x, y, z, 0x2c1b3c6d);
  return [((h >>> 4) % 5 - 2) / 16, ((h >>> 12) % 5 - 2) / 16];
};
// Cross-plant shape from the world coordinate alone (own salt): mirrored
// texture, height in 12/16..16/16 (used for tall grass and ferns only) and
// brightness jitter of about +-6%.
const plantShape = (x, y, z) => {
  const h = hashAt(x, y, z, 0x7f4a7c15);
  return { mirror: (h & 1) === 1, height: (12 + ((h >>> 1) & 0xff) % 5) / 16, bright: 1 + (((h >>> 9) & 0xff) / 255 - 0.5) * 0.12 };
};
const PLANT_BASE_DARK = 0.8, clamp01 = (v) => Math.min(1, Math.max(0, v));
// `layer` moved to its tile variant, picked by the world coordinate alone with
// a salt of its own (independent of variantAt). Layers without variants stay.
function pickLayer(ctx, layer, x, y, z) {
  const v = ctx.variants?.[layer];
  if (!v) return layer;
  const r = (hashAt(x, y, z, 0x5bd1e995) >>> 8) / 0x1000000;
  let i = 0;
  while (i < v.count - 1 && r >= v.cum[i]) i++;
  return layer + i;
}

const CACTUS_INSET = 1 / 16, SNOW_H = 1 / 8;

// Growable quad buffer in the packed vertex format.
class Quads {
  constructor() { this.buf = new ArrayBuffer(VERTEX_BYTES * 4 * 1024); this.n = 0; this.view(); }
  view() { this.i16 = new Int16Array(this.buf); this.u8 = new Uint8Array(this.buf); }
  // `tint` is one [r, g, b] (0-1) for all four vertices, or four of them.
  quad(x, y, z, p, uv, layer, k, tint, top = null, ao = null) {
    if ((this.n + 4) * VERTEX_BYTES > this.buf.byteLength) {
      const nb = new ArrayBuffer(this.buf.byteLength * 2);
      new Uint8Array(nb).set(this.u8); this.buf = nb; this.view();
    }
    const sh = Math.round(k * 128), per = Array.isArray(tint[0]);
    for (let j = 0; j < 4; j++) {
      const t = per ? tint[j] : tint;
      const r = Math.round(t[0] * 255), g = Math.round(t[1] * 255), b = Math.round(t[2] * 255);
      const o = (this.n + j) * VERTEX_BYTES;
      this.i16[o >> 1] = Math.round((x + p[j][0]) * 16);
      this.i16[(o >> 1) + 1] = Math.round((y + p[j][1]) * 16);
      this.i16[(o >> 1) + 2] = Math.round((z + p[j][2]) * 16);
      this.u8[o + 6] = Math.round(uv[j][0] * 16); this.u8[o + 7] = Math.round(uv[j][1] * 16);
      this.u8[o + 8] = layer; this.u8[o + 9] = sh;
      this.u8[o + 10] = r; this.u8[o + 11] = g; this.u8[o + 12] = b;
      this.u8[o + 13] = ao ? ao[j] : 0;
      this.u8[o + 14] = top ? top[j] : 0;
    }
    this.n += 4;
  }
  done() { return { verts: this.buf.slice(0, this.n * VERTEX_BYTES), faces: this.n / 4 }; }
}

/**
 * Mesh a region of blocks. `blocks` covers (size + 2 * pad)^2 columns
 * (layout ((z * w) + x) * 128 + y); only the inner size x size columns are
 * meshed, and the pad ring is used for face culling, so meshes of adjacent
 * areas meet without seams. With pad 0 the outside counts as air (the 3D
 * view's free-standing block of terrain). `tints` are for the inner columns.
 * When they carry `corners` (see `columnTints`), tinted blocks colour each
 * vertex from the corner grid, so tops, sides and neighbouring blocks agree
 * where they meet (cross plants use the nearest corner of their own column);
 * otherwise a face takes its column's tint.
 *
 * Solid ground is clipped 8 blocks under the lowest surface. Returns
 * { opaque, cutout, water, ice } (each { verts, faces }), and the mean and
 * lowest surface heights. `opaque` has no transparent texels, so it can be
 * drawn without discard; `cutout` is leaves and plants.
 *
 * `origin` is the world (x, z) of the area's first column; it seeds the
 * per-block texture variants of ground blocks (grass, sand, gravel, stone,
 * snow layer), which depend only on the world coordinate.
 *
 * `fast`: fast leaves, for terrain seen from afar. Leaves hide each other
 * and the faces next to them like solid blocks, and use the hole-free
 * leaves_fast tile, so they move to `opaque`. In a forest that's about half
 * the faces.
 */
export function meshBlocks(ctx, blocks, tints, size, pad = 0, { fast = false, origin = [0, 0] } = {}) {
  const SOLID_N = fast ? OPAQUE_FAST : OPAQUE;
  const w = size + 2 * pad;
  const col = (x, z) => ((z + pad) * w + x + pad) * H;
  let minY = H, sumY = 0;
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    const base = col(x, z);
    let y = H - 1;
    while (y > 0 && (blocks[base + y] === B.air || blocks[base + y] === B.water || blocks[base + y] === B.ice)) y--;
    minY = Math.min(minY, y); sumY += y;
  }
  const y0 = Math.max(0, minY - 8);
  const at = (x, y, z) => (x < -pad || z < -pad || x >= size + pad || z >= size + pad || y < y0 || y >= H)
    ? 0 : blocks[col(x, z) + y];
  const out = [new Quads(), new Quads(), new Quads(), new Quads()]; // opaque, water, ice, cutout
  // One face of the box lo..hi (block-local) for face f.
  const boxFace = (L, x, y, z, f, lo, hi, layer, tint, vr = 0, ao = null) => {
    let p = f.c.map((c) => c.map((v, j) => lo[j] + v * (hi[j] - lo[j])));
    let uv = p.map((q) => faceUV(f, q, vr));
    if (typeof tint === 'function') tint = p.map((q) => tint(Math.round(q[0]), Math.round(q[2])));
    if (ao && ao[0] + ao[2] < ao[1] + ao[3]) {
      // flip the diagonal: rotate the vertices so the shared index pattern splits the other way
      const rot = (a) => [a[1], a[2], a[3], a[0]];
      p = rot(p); uv = rot(uv); ao = rot(ao);
      if (Array.isArray(tint[0])) tint = rot(tint);
    }
    out[L].quad(x, y, z, p, uv, layer, f.s * (0.75 + y / 256), tint, null, ao);
  };
  // AO level 0-3 of each corner of face f of the block at (x, y, z): the
  // opaque blocks among the two side neighbours and the diagonal, on the
  // face's open side.
  const faceAO = (x, y, z, f) => {
    const ax = f.d[0] !== 0 ? 1 : 0, ay = f.d[1] !== 0 ? 1 : 0;
    // the two tangent axes of the face
    const [a, b] = ax ? [1, 2] : ay ? [0, 2] : [0, 1];
    const o = [x + f.d[0], y + f.d[1], z + f.d[2]];
    return f.c.map((c) => {
      const sa = c[a] ? 1 : -1, sb = c[b] ? 1 : -1;
      const q = (da, db) => { const r = o.slice(); r[a] += da; r[b] += db; return SOLID_N[at(r[0], r[1], r[2])]; };
      return q(sa, 0) + q(0, sb) + q(sa, sb);
    });
  };
  const FULL_LO = [0, 0, 0], FULL_HI = [1, 1, 1];
  const CACTUS_LO = [CACTUS_INSET, 0, CACTUS_INSET], CACTUS_HI = [1 - CACTUS_INSET, 1, 1 - CACTUS_INSET];
  const SNOW_HI = [1, SNOW_H, 1];
  const X1 = [[0.15, 1, 0.15], [0.85, 1, 0.85], [0.85, 0, 0.85], [0.15, 0, 0.15]];
  const X2 = [[0.85, 1, 0.15], [0.15, 1, 0.85], [0.15, 0, 0.85], [0.85, 0, 0.15]];
  const XTOP = [255, 255, 0, 0]; // vertex order of X1/X2: top, top, base, base
  const XUV = [[0, 0], [1, 0], [1, 1], [0, 1]];
  for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
    const c = z * size + x, base = col(x, z);
    for (let y = y0; y < H; y++) {
      const b = blocks[base + y];
      if (b === B.air) continue;
      const flat = tintFor(ctx, b, tints.grass[c], tints.leaf[c], tints.water?.[c]);
      // Per-vertex tint by corner offset (0 or 1), for blocks that have one.
      const tint = flat === WHITE || !tints.corners ? flat : (ox, oz) => {
        const k = ((z + oz) * tints.corners.n + x + ox) * 3, g = tints.corners.grass, l = tints.corners.leaf, w = tints.corners.water;
        return tintFor(ctx, b, [g[k], g[k + 1], g[k + 2]], [l[k], l[k + 1], l[k + 2]], w && [w[k], w[k + 1], w[k + 2]]);
      };
      const vr = VARIES[b] ? variantAt(origin[0] + x, y, origin[1] + z) : 0;
      if (CROSS[b]) {
        // two crossed quads, wound both ways so they show from either side
        const layer = pickLayer(ctx, layerOf(ctx, b, 0), origin[0] + x, y, origin[1] + z), k = 0.9 * (0.75 + y / 256);
        const [dx, dz] = plantOffset(origin[0] + x, y, origin[1] + z);
        const shape = plantShape(origin[0] + x, y, z + origin[1]);
        const tall = b === B.tall_grass || b === B.fern, hgt = tall ? shape.height : 1;
        const uvs = XUV.map(([u, v]) => [shape.mirror ? 1 - u : u, v === 0 ? 1 - hgt : v]);
        for (const q of [X1, X2]) {
          const t = typeof tint === 'function' ? q.map((v) => tint(Math.round(v[0]), Math.round(v[2]))) : q.map(() => tint);
          const tj = t.map((c, j) => c.map((v) => clamp01(v * shape.bright) * (XTOP[j] ? 1 : PLANT_BASE_DARK)));
          const qs = q.map((v) => [v[0], v[1] * hgt, v[2]]);
          const flip = (a) => [a[1], a[0], a[3], a[2]];
          out[3].quad(x + dx, y, z + dz, qs, uvs, layer, k, tj, XTOP);
          out[3].quad(x + dx, y, z + dz, flip(qs), flip(uvs), layer, k, flip(tj), flip(XTOP));
        }
        continue;
      }
      for (const f of FACES) {
        if (y === y0 && f.d[1] < 0) continue;
        const n = at(x + f.d[0], y + f.d[1], z + f.d[2]);
        let layer = layerOf(ctx, b, f.d[1], fast);
        if (b === B.grass && f.d[1] > 0) layer = pickLayer(ctx, layer, origin[0] + x, y, origin[1] + z);
        if (b === B.water) {
          // water: only next to air, plants or leaves (not water, ice or solids)
          if (n === B.water || n === B.ice || SOLID_N[n]) continue;
          boxFace(1, x, y, z, f, FULL_LO, FULL_HI, layer, tint);
        } else if (b === B.ice) {
          if (n === B.ice || SOLID_N[n]) continue;
          boxFace(2, x, y, z, f, FULL_LO, FULL_HI, layer, tint);
        } else if (b === B.cactus) {
          // inset sides always show; top/bottom hidden by more cactus or solid ground
          if (f.d[1] > 0 && n === B.cactus) continue;
          if (f.d[1] < 0 && (n === B.cactus || SOLID_N[n])) continue;
          boxFace(0, x, y, z, f, CACTUS_LO, CACTUS_HI, layer, tint);
        } else if (b === B.snow_layer) {
          if (f.d[1] < 0 && SOLID_N[n]) continue;
          if (f.d[1] === 0 && (SOLID_N[n] || n === B.snow_layer)) continue;
          boxFace(0, x, y, z, f, FULL_LO, SNOW_HI, layer, tint, vr);
        } else {
          // full cubes and leaves: hidden only by opaque cubes, so (unless
          // fast) leaves never hide their neighbours and leaf-leaf faces are
          // drawn, as cut-outs
          if (SOLID_N[n]) continue;
          boxFace(LEAVES[b] && !fast ? 3 : 0, x, y, z, f, FULL_LO, FULL_HI, layer, tint, vr, faceAO(x, y, z, f));
        }
      }
    }
  }
  const [opaque, water, ice, cutout] = out.map((q) => q.done());
  return { opaque, cutout, water, ice, meanY: sumY / (size * size), minY };
}
