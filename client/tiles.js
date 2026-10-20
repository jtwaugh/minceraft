// Texture array and block-face table for the client scene, built from the
// viewer's tileset export (viewer/textures.json, written by build.sh) so the
// client draws the same blocks as the viewer through viewer/mesh.js.
// Same layer order as viewer/index.html; water is two runs of 4 frames (tileFor
// returns frame 0; the shader adds the animation frame) and tiles missing from the export fall back to flat grey.
//
// Unlike the viewer, the tint flag is not packed into alpha (mipmapping averages
// alpha, so a flag there flips unpredictably at a distance). Instead each texel
// is split into two premultiplied textures that mipmap exactly:
//   colour  RGBA8: untinted colour (0 for tinted texels), alpha = coverage
//   tint    R8:    greyscale of climate-tinted texels (0 otherwise)
// The shader rebuilds (colour.rgb + tint.r * vtint) / colour.a.

import { faceLayers } from '../viewer/mesh.js';

const S = 16;
const NAMES = ['stone', 'dirt', 'grass_top', 'grass_side', 'sand', 'gravel', 'water_0', 'water_1', 'water_2', 'water_3',
  'water_flow_0', 'water_flow_1', 'water_flow_2', 'water_flow_3',
  'log_side', 'log_top', 'leaves', 'birch_log_side', 'birch_log_top', 'spruce_log_side', 'spruce_log_top',
  'ice', 'snow', 'cactus_side', 'cactus_top', 'pumpkin_side', 'pumpkin_top',
  'tall_grass', 'fern', 'dead_bush', 'dandelion', 'rose', 'sugar_cane', 'brown_mushroom', 'red_mushroom',
  'leaves_fast', 'farmland_dry', 'farmland_wet', 'wheat_0', 'wheat_1', 'wheat_2', 'wheat_3'];
const TINTED = { grass_top: '89', grass_side: '89', leaves: '89', tall_grass: '89', fern: '89' };
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
  gl.activeTexture(gl.TEXTURE1);
  upload(gl, gl.R8, gl.RED, grey);
  gl.activeTexture(gl.TEXTURE0);
  upload(gl, gl.RGBA8, gl.RGBA, colour);
  return faceLayers((b, dy, fast) => tileFor(B, names, b, dy, fast));
}

/** Creates, fills and binds a mipmapped tile array on the active texture unit. */
function upload(gl, internal, format, data) {
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, gl.createTexture());
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.texImage3D(gl.TEXTURE_2D_ARRAY, 0, internal, S, S, NAMES.length, 0, format, gl.UNSIGNED_BYTE, data);
  gl.generateMipmap(gl.TEXTURE_2D_ARRAY);
  // Filtering and wrap come from the per-pass samplers (samplers.js), bound
  // by the scene; the texture only needs complete mips here.
}
