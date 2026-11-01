// Texture array and block-face table for the client scene, built from the
// viewer's tileset export (viewer/textures.json, written by build.sh) so the
// client draws the same blocks as the viewer through viewer/mesh.js.
// Shared names come from viewer/tilelist.js; water is two runs of 4 frames (tileFor
// returns frame 0; the shader adds the animation frame) and tiles missing from the export fall back to flat grey.
//
// Unlike the viewer, the tint flag is not packed into alpha (mipmapping averages
// alpha, so a flag there flips unpredictably at a distance). Instead each texel
// is split into two premultiplied textures that mipmap exactly:
//   colour  RGBA8: untinted colour (0 for tinted texels), alpha = coverage
//   tint    R8:    greyscale of climate-tinted texels (0 otherwise)
// The shader rebuilds (colour.rgb + tint.r * vtint) / colour.a.

import { faceLayers } from '../viewer/mesh.js';
import { buildMips } from '../viewer/mips.js';
import { NAMES as SHARED, TINTED } from '../viewer/tilelist.js';

const S = 16;
/** Layer names in texture-array order. */
export const NAMES = [...SHARED, 'farmland_dry', 'farmland_wet', 'wheat_0', 'wheat_1', 'wheat_2', 'wheat_3',
  'planter_side', 'planter_top', 'planter_bottom', 'planter_farmland_top',
  // no art yet (sea-flora-and-shellfish open question 7): flat grey until drawn
  'kelp_0', 'kelp_1'];
const L = Object.fromEntries(NAMES.map((n, i) => [n, i]));
/** Layers [ANIM_FIRST, ANIM_END) are the water runs; the shader offsets them by the frame. */
export const ANIM_FIRST = L.water_0, ANIM_END = L.water_flow_3 + 1;

/** Layer for block id b on a face with vertical normal dy; `names` maps block id to name. */
function tileFor(B, names, b, dy, fast) {
  switch (b) {
    case B.grass: return dy > 0 ? L.grass_top : dy < 0 ? L.dirt : L.grass_side;
    case B.water: return dy === 0 ? L.water_flow_0 : L.water_0;
    case B.oak_log: return dy ? L.log_top : L.log_side;
    case B.birch_log: return dy ? L.birch_log_top : L.birch_log_side;
    case B.spruce_log: return dy ? L.spruce_log_top : L.spruce_log_side;
    case B.oak_leaves: case B.birch_leaves: case B.spruce_leaves: return fast ? L.leaves_fast : L.leaves;
    case B.cactus: return dy ? L.cactus_top : L.cactus_side;
    case B.pumpkin: return dy ? L.pumpkin_top : L.pumpkin_side;
    case B.snow_layer: return L.snow;
    case B.farmland_dry: case B.farmland_wet: return dy > 0 ? L[names[b]] : L.dirt;
    case B.planter_dirt: return dy > 0 ? L.planter_top : dy < 0 ? L.planter_bottom : L.planter_side;
    case B.planter_farmland_dry: case B.planter_farmland_wet:
      return dy > 0 ? L.planter_farmland_top : dy < 0 ? L.planter_bottom : L.planter_side;
    // eight growth stages share the four wheat textures, two stages each
    case B.wheat_0: case B.wheat_1: case B.wheat_2: case B.wheat_3:
    case B.wheat_4: case B.wheat_5: case B.wheat_6: case B.wheat_7: return L[`wheat_${(b - B.wheat_0) >> 1}`];
    default: return L[names[b]] ?? L.stone; // stone, dirt, sand, gravel, ice, cross plants
  }
}

/**
 * Uploads the tile array to `gl` (bound to TEXTURE_2D_ARRAY) and returns the
 * face table for legendContext(). `B` and `names` come from mesh.js / legend.
 */
export async function loadTiles(gl, B, names, textureUrl) {
  let all = {}, pal = {};
  try {
    const set = await (await fetch(textureUrl)).json();
    all = Object.assign({}, ...Object.values(set.batches).map((b) => b.textures || {}));
    pal = Object.fromEntries(Object.entries(set.palette).map(([k, hex]) =>
      [k, [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16))]));
  } catch (e) {
    console.warn('textures unavailable, using flat grey:', e);
  }
  const lum = (c) => 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
  const tile = (name) => {
    const rows = all[name === 'leaves_fast' ? 'leaves' : name];
    if (!rows) return () => [128, 128, 128, 255, 0];
    const k = S / rows.length;
    const tint = TINTED[name === 'leaves_fast' ? 'leaves' : name] || '';
    const maxL = Math.max(1, ...[...tint].map((ch) => lum(pal[ch])));
    return (x, y) => {
      const ch = rows[Math.floor(y / k)][Math.floor(x / k)];
      // [r, g, b, coverage, tint grey]
      if (ch === '.') return name === 'leaves_fast' ? [0, 0, 0, 255, 96] : [0, 0, 0, 0, 0];
      if (tint.includes(ch)) return [0, 0, 0, 255, 255 * lum(pal[ch]) / maxL];
      return [...pal[ch], 255, 0];
    };
  };
  const colour = new Uint8Array(S * S * 4 * NAMES.length);
  const grey = new Uint8Array(S * S * NAMES.length);
  NAMES.forEach((n, layer) => {
    const px = tile(n);
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) {
      const i = (layer * S + y) * S + x, p = px(x, y);
      colour.set(p.slice(0, 4), i * 4);
      grey[i] = p[4];
    }
  });
  // Tint on unit 1, colour on unit 0 (left bound and active for the scene).
  const { colourLevels, tintLevels } = buildLevels(colour, grey, NAMES.length);
  gl.activeTexture(gl.TEXTURE1);
  upload(gl, gl.R8, gl.RED, tintLevels);
  gl.activeTexture(gl.TEXTURE0);
  upload(gl, gl.RGBA8, gl.RGBA, colourLevels);
  return faceLayers((b, dy, fast) => tileFor(B, names, b, dy, fast));
}

/**
 * Mip chains for the tile array, level 0 first; each level holds every layer
 * back to back. Colour is premultiplied with the level's coverage alpha. The
 * tint goes through the same chain as a grey texel with that alpha, so its
 * levels are scaled by the same factor and tint / alpha stays the
 * alpha-weighted mean at every level.
 */
export function buildLevels(colour, grey, layers) {
  const n = Math.log2(S) + 1;
  const colourLevels = Array.from({ length: n }, (_, l) => new Uint8Array(layers * (S >> l) ** 2 * 4));
  const tintLevels = Array.from({ length: n }, (_, l) => new Uint8Array(layers * (S >> l) ** 2));
  for (let k = 0; k < layers; k++) {
    const c = colour.subarray(k * S * S * 4, (k + 1) * S * S * 4);
    const t = new Uint8Array(S * S * 4);
    for (let i = 0; i < S * S; i++) t.set([grey[k * S * S + i], 0, 0, c[i * 4 + 3]], i * 4);
    const cm = buildMips(c, S, { premultiply: true });
    const tm = buildMips(t, S, { premultiply: true });
    for (let l = 0; l < n; l++) {
      const px = (S >> l) ** 2;
      colourLevels[l].set(cm[l], k * px * 4);
      for (let i = 0; i < px; i++) tintLevels[l][k * px + i] = tm[l][i * 4];
    }
  }
  return { colourLevels, tintLevels };
}

/** Creates, fills and binds a tile array with every mip level on the active texture unit. */
function upload(gl, internal, format, levels) {
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, gl.createTexture());
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  levels.forEach((data, l) => {
    const s = S >> l;
    gl.texImage3D(gl.TEXTURE_2D_ARRAY, l, internal, s, s, NAMES.length, 0, format, gl.UNSIGNED_BYTE, data);
  });
  // Filtering and wrap come from the per-pass samplers (samplers.js), bound
  // by the scene.
}
