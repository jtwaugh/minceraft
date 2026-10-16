// Texture array and block-face table for the client scene, built from the
// viewer's tileset export (viewer/textures.json, written by build.sh) so the
// client draws the same blocks as the viewer through viewer/mesh.js.
// Same layer order and tint convention as viewer/index.html; water is a single
// still frame here and tiles missing from the export fall back to flat grey.

import { faceLayers } from '../viewer/mesh.js';

const S = 16;
const TINT_A = 200; // alpha marking a greyscale, climate-tinted texel (shader convention)
const NAMES = ['stone', 'dirt', 'grass_top', 'grass_side', 'sand', 'gravel', 'water_0', 'water_flow_0',
  'log_side', 'log_top', 'leaves', 'birch_log_side', 'birch_log_top', 'spruce_log_side', 'spruce_log_top',
  'ice', 'snow', 'cactus_side', 'cactus_top', 'pumpkin_side', 'pumpkin_top',
  'tall_grass', 'fern', 'dead_bush', 'dandelion', 'rose', 'sugar_cane', 'brown_mushroom', 'red_mushroom',
  'leaves_fast'];
const TINTED = { grass_top: '89', grass_side: '89', leaves: '89', tall_grass: '89', fern: '89' };
const L = Object.fromEntries(NAMES.map((n, i) => [n, i]));

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
    if (!rows) return () => [128, 128, 128, 255];
    const k = S / rows.length;
    const tint = TINTED[name === 'leaves_fast' ? 'leaves' : name] || '';
    const maxL = Math.max(1, ...[...tint].map((ch) => lum(pal[ch])));
    return (x, y) => {
      const ch = rows[Math.floor(y / k)][Math.floor(x / k)];
      if (ch === '.') return name === 'leaves_fast' ? [96, 96, 96, TINT_A] : [0, 0, 0, 0];
      if (tint.includes(ch)) { const v = 255 * lum(pal[ch]) / maxL; return [v, v, v, TINT_A]; }
      return [...pal[ch], 255];
    };
  };
  const data = new Uint8Array(S * S * 4 * NAMES.length);
  NAMES.forEach((n, layer) => {
    const px = tile(n);
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) data.set(px(x, y), ((layer * S + y) * S + x) * 4);
  });
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, tex);
  gl.texImage3D(gl.TEXTURE_2D_ARRAY, 0, gl.RGBA8, S, S, NAMES.length, 0, gl.RGBA, gl.UNSIGNED_BYTE, data);
  gl.generateMipmap(gl.TEXTURE_2D_ARRAY);
  gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_MIN_FILTER, gl.NEAREST_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_WRAP_S, gl.REPEAT);
  gl.texParameteri(gl.TEXTURE_2D_ARRAY, gl.TEXTURE_WRAP_T, gl.REPEAT);
  return faceLayers((b, dy, fast) => tileFor(B, names, b, dy, fast));
}
