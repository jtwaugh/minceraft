// Block meshing shared by the 3D view (main thread) and the flyover chunk
// workers. Produces quads (4 vertices each) in a compact 16-byte vertex
// format, drawn with one shared index buffer (quadIndices):
//
//   bytes 0-5   position x, y, z  int16, in 1/16 block (POS_SCALE)
//   bytes 6-7   uv                uint8, in 1/16 (UV_SCALE)
//   byte  8     texture layer     uint8
//   byte  9     shade             uint8, shade * 128 (SHADE_SCALE)
//   bytes 10-12 tint rgb          uint8, normalized
//   byte  13    reserved (ambient occlusion, per viewer-rendering)
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

// Block ids (data/blocks.toml); names and colours come from legend().
export const B = {
  air: 0, stone: 1, dirt: 2, grass: 3, sand: 4, gravel: 5, water: 6, ice: 7,
  oak_log: 8, birch_log: 9, spruce_log: 10, oak_leaves: 11, birch_leaves: 12, spruce_leaves: 13,
  tall_grass: 14, fern: 15, dead_bush: 16, dandelion: 17, rose: 18, sugar_cane: 19,
  cactus: 20, brown_mushroom: 21, red_mushroom: 22, pumpkin: 23, snow_layer: 24,
  raft_planks: 25, butcher_block: 26, griddle: 27, crate: 28,
  farmland_dry: 29, farmland_wet: 30,
  wheat_0: 31, wheat_1: 32, wheat_2: 33, wheat_3: 34, wheat_4: 35, wheat_5: 36, wheat_6: 37, wheat_7: 38,
};

const flags = (ids) => { const a = new Uint8Array(256); for (const b of ids) a[b] = 1; return a; };
// Full cubes that hide the faces of whatever is next to them.
const SOLID = [B.stone, B.dirt, B.grass, B.sand, B.gravel, B.oak_log, B.birch_log, B.spruce_log, B.pumpkin,
  B.raft_planks, B.butcher_block, B.griddle, B.crate, B.farmland_dry, B.farmland_wet];
export const OPAQUE = flags(SOLID);
export const LEAVES = flags([B.oak_leaves, B.birch_leaves, B.spruce_leaves]);
// With fast leaves, leaves count as solid too.
const OPAQUE_FAST = flags([...SOLID, B.oak_leaves, B.birch_leaves, B.spruce_leaves]);
// Drawn as two crossed quads.
export const CROSS = flags([B.tall_grass, B.fern, B.dead_bush, B.dandelion, B.rose, B.sugar_cane, B.brown_mushroom, B.red_mushroom]);

/**
 * Meshing context: names, colours and biomes from the legend() JSON, plus
 * `faceLayer`, the texture layer of every block face (see faceLayers). The
 * page owns the texture array, so it builds the table and hands it over.
 */
export function legendContext(json, faceLayer = null) {
  const legend = JSON.parse(json);
  const COLORS = [], NAMES = [], BIOMES = [];
  for (const b of legend.blocks) { COLORS[b.id] = b.color; NAMES[b.id] = b.name; }
  for (const b of legend.biomes) BIOMES[b.id] = b;
  return { legend, COLORS, NAMES, BIOMES, faceLayer };
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
 * Per-column climate tints for a size x size area at world (wx, wz): grass
 * and leaf ramps by humidity. `wasm` needs biome_map, humidity_map and
 * temperature_map.
 */
export function columnTints(wasm, ctx, seed, wx, wz, size) {
  const bio = wasm.biome_map(seed, wx, wz, size, size);
  const hum = wasm.humidity_map(seed, wx, wz, size, size);
  const tmp = wasm.temperature_map(seed, wx, wz, size, size);
  const grass = [], leaf = [];
  for (let c = 0; c < size * size; c++) {
    const bi = ctx.BIOMES[bio[c]];
    grass.push(ramp3(bi.grass, hum[c]).map((v) => v / 255));
    leaf.push(ramp3(bi.leaf, hum[c]).map((v) => v / 255));
  }
  const mid = (size >> 1) * size + (size >> 1);
  return { grass, leaf, bio, tmp, centre: { biome: ctx.BIOMES[bio[mid]], t: tmp[mid] } };
}

const WHITE = [1, 1, 1];
function tintFor(ctx, b, tints, c) {
  switch (b) {
    case B.grass: case B.tall_grass: return tints.grass[c];
    case B.fern: return tints.grass[c].map((v) => v * 0.88);
    case B.oak_leaves: return tints.leaf[c];
    case B.birch_leaves: return lerp3(tints.leaf[c], ctx.COLORS[B.birch_leaves].map((v) => v / 255), 0.55);
    case B.spruce_leaves: return lerp3(tints.leaf[c], ctx.COLORS[B.spruce_leaves].map((v) => v / 255), 0.7);
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
function faceUV(f, [a, b, c]) {
  if (f.d[1] !== 0) return [a, c];
  return [f.d[0] !== 0 ? c : a, 1 - b];
}

const CACTUS_INSET = 1 / 16, SNOW_H = 1 / 8;

// Growable quad buffer in the packed vertex format.
class Quads {
  constructor() { this.buf = new ArrayBuffer(VERTEX_BYTES * 4 * 1024); this.n = 0; this.view(); }
  view() { this.i16 = new Int16Array(this.buf); this.u8 = new Uint8Array(this.buf); }
  quad(x, y, z, p, uv, layer, k, tint, top = null) {
    if ((this.n + 4) * VERTEX_BYTES > this.buf.byteLength) {
      const nb = new ArrayBuffer(this.buf.byteLength * 2);
      new Uint8Array(nb).set(this.u8); this.buf = nb; this.view();
    }
    const sh = Math.round(k * 128), r = Math.round(tint[0] * 255), g = Math.round(tint[1] * 255), b = Math.round(tint[2] * 255);
    for (let j = 0; j < 4; j++) {
      const o = (this.n + j) * VERTEX_BYTES;
      this.i16[o >> 1] = Math.round((x + p[j][0]) * 16);
      this.i16[(o >> 1) + 1] = Math.round((y + p[j][1]) * 16);
      this.i16[(o >> 1) + 2] = Math.round((z + p[j][2]) * 16);
      this.u8[o + 6] = Math.round(uv[j][0] * 16); this.u8[o + 7] = Math.round(uv[j][1] * 16);
      this.u8[o + 8] = layer; this.u8[o + 9] = sh;
      this.u8[o + 10] = r; this.u8[o + 11] = g; this.u8[o + 12] = b;
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
 *
 * Solid ground is clipped 8 blocks under the lowest surface. Returns
 * { opaque, cutout, water, ice } (each { verts, faces }), and the mean and
 * lowest surface heights. `opaque` has no transparent texels, so it can be
 * drawn without discard; `cutout` is leaves and plants.
 *
 * `fast`: fast leaves, for terrain seen from afar. Leaves hide each other
 * and the faces next to them like solid blocks, and use the hole-free
 * leaves_fast tile, so they move to `opaque`. In a forest that's about half
 * the faces.
 */
export function meshBlocks(ctx, blocks, tints, size, pad = 0, { fast = false } = {}) {
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
  const boxFace = (L, x, y, z, f, lo, hi, layer, tint) => {
    const p = f.c.map((c) => c.map((v, j) => lo[j] + v * (hi[j] - lo[j])));
    out[L].quad(x, y, z, p, p.map((q) => faceUV(f, q)), layer, f.s * (0.75 + y / 256), tint);
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
      const tint = tintFor(ctx, b, tints, c);
      if (CROSS[b]) {
        // two crossed quads, wound both ways so they show from either side
        const layer = layerOf(ctx, b, 0), k = 0.9 * (0.75 + y / 256);
        for (const q of [X1, X2]) {
          out[3].quad(x, y, z, q, XUV, layer, k, tint, XTOP);
          out[3].quad(x, y, z, [q[1], q[0], q[3], q[2]], [XUV[1], XUV[0], XUV[3], XUV[2]], layer, k, tint, [XTOP[1], XTOP[0], XTOP[3], XTOP[2]]);
        }
        continue;
      }
      for (const f of FACES) {
        if (y === y0 && f.d[1] < 0) continue;
        const n = at(x + f.d[0], y + f.d[1], z + f.d[2]);
        const layer = layerOf(ctx, b, f.d[1], fast);
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
          boxFace(0, x, y, z, f, FULL_LO, SNOW_HI, layer, tint);
        } else {
          // full cubes and leaves: hidden only by opaque cubes, so (unless
          // fast) leaves never hide their neighbours and leaf-leaf faces are
          // drawn, as cut-outs
          if (SOLID_N[n]) continue;
          boxFace(LEAVES[b] && !fast ? 3 : 0, x, y, z, f, FULL_LO, FULL_HI, layer, tint);
        }
      }
    }
  }
  const [opaque, water, ice, cutout] = out.map((q) => q.done());
  return { opaque, cutout, water, ice, meanY: sumY / (size * size), minY };
}
