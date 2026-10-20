// Model atlas and box geometry for the client, with no GL: packAtlas() packs
// the model faces (and any named textures) of the tileset export
// (viewer/textures.json) into one RGBA image description, and boxVertices()
// turns a box into vertices with per-face UVs into it. Face orientation
// follows MODEL FACE CONVENTIONS in art/PROMPT.md; front is -z at facing 0, as
// drawAnimals in scene.js assumes. UV v runs down the image, row 0 first, so
// the grid's top-left pixel is (u0, v0). Faces missing from the atlas use a
// flat grey rectangle, as tiles.js does.

const WIDTH = 128;
const GAP = 1; // empty border so linear filtering does not bleed between rects
const GREY = [128, 128, 128, 255];
export const FALLBACK = '(grey)';

const hexRgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

/**
 * Packs `model.face` grids of every model in `exportJson`, plus the textures
 * named in `textureNames`, into one atlas. Returns { width, height, data
 * (RGBA, Uint8Array), rects: { name: {x, y, w, h} }, uvOf(name) }. uvOf gives
 * { u0, v0, u1, v1 } for a name, or the grey rectangle if it is not packed.
 * Deterministic: grids are placed on shelves, tallest first, then by name.
 */
export function packAtlas(exportJson, textureNames = []) {
  const pal = Object.fromEntries(Object.entries(exportJson.palette).map(([k, h]) => [k, hexRgb(h)]));
  const grids = new Map();
  const wanted = new Set(textureNames);
  for (const batch of Object.values(exportJson.batches)) {
    for (const [name, rows] of Object.entries(batch.textures || {})) if (wanted.has(name)) grids.set(name, rows);
    for (const [model, faces] of Object.entries(batch.models || {})) {
      for (const [face, rows] of Object.entries(faces)) grids.set(`${model}.${face}`, rows);
    }
  }
  const order = [...grids].sort(([an, a], [bn, b]) => b.length - a.length || (an < bn ? -1 : an > bn ? 1 : 0));
  order.unshift([FALLBACK, ['g']]);

  const rects = {};
  let x = 0, y = 0, shelf = 0;
  for (const [name, rows] of order) {
    const w = rows[0].length, h = rows.length;
    if (w + GAP > WIDTH) throw new Error(`${name}: ${w} px wide does not fit the ${WIDTH} px atlas`);
    if (x + w + GAP > WIDTH) { x = 0; y += shelf + GAP; shelf = 0; }
    rects[name] = { x, y, w, h };
    x += w + GAP;
    shelf = Math.max(shelf, h);
  }
  let height = 1;
  while (height < y + shelf + GAP) height *= 2;

  const data = new Uint8Array(WIDTH * height * 4);
  for (const [name, rows] of order) {
    const r = rects[name];
    rows.forEach((row, j) => [...row].forEach((ch, i) => {
      const px = ch === '.' ? [0, 0, 0, 0] : name === FALLBACK ? GREY : [...pal[ch], 255];
      data.set(px, ((r.y + j) * WIDTH + r.x + i) * 4);
    }));
  }
  const uvOf = (name) => {
    const r = rects[name] || rects[FALLBACK];
    return { u0: r.x / WIDTH, v0: r.y / height, u1: (r.x + r.w) / WIDTH, v1: (r.y + r.h) / height };
  };
  return { width: WIDTH, height, data, rects, uvOf };
}

// Each face: its top-left corner (0 = lo, 1 = hi per axis) and the unit axes of
// the image's right and down edges, all seen from outside the box.
const FACES = [
  { name: 'front', tl: [1, 1, 0], right: [-1, 0, 0], down: [0, -1, 0] },
  { name: 'back', tl: [0, 1, 1], right: [1, 0, 0], down: [0, -1, 0] },
  { name: 'left', tl: [0, 1, 0], right: [0, 0, 1], down: [0, -1, 0] },
  { name: 'right', tl: [1, 1, 1], right: [0, 0, -1], down: [0, -1, 0] },
  { name: 'top', tl: [1, 1, 1], right: [-1, 0, 0], down: [0, 0, -1] },
  { name: 'bottom', tl: [1, 0, 0], right: [-1, 0, 0], down: [0, 0, 1] },
];

/**
 * One box { lo: [x, y, z], size: [w, h, d], faces: { front, back, left,
 * right, top, bottom } } to { vertices: Float32Array (x y z u v per vertex, 24
 * vertices), indices: Uint16Array (36, counter-clockwise seen from outside) }.
 * `uvOf(faceName)` is packAtlas's uvOf; face names are `model.face` strings.
 */
export function boxVertices(box, uvOf) {
  const { lo, size } = box;
  const vertices = new Float32Array(24 * 5);
  const indices = new Uint16Array(36);
  FACES.forEach((f, n) => {
    const { u0, v0, u1, v1 } = uvOf((box.faces || {})[f.name]);
    const tl = f.tl.map((hi, a) => lo[a] + hi * size[a]);
    const r = f.right.map((d, a) => d * size[a]), d = f.down.map((e, a) => e * size[a]);
    const corners = [[0, 0, u0, v0], [0, 1, u0, v1], [1, 1, u1, v1], [1, 0, u1, v0]]; // TL BL BR TR
    corners.forEach(([i, j, u, v], k) => {
      vertices.set([0, 1, 2].map((a) => tl[a] + i * r[a] + j * d[a]), (n * 4 + k) * 5);
      vertices.set([u, v], (n * 4 + k) * 5 + 3);
    });
    indices.set([0, 1, 2, 0, 2, 3].map((k) => n * 4 + k), n * 6);
  });
  return { vertices, indices };
}
