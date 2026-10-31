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
// A fully transparent rect: faces that name it are discarded by the shader.
export const CLEAR = '(clear)';
// The four thin edge faces of a sprite slab, all CLEAR, so only the icon shows.
export const CLEAR_EDGES = { left: CLEAR, right: CLEAR, top: CLEAR, bottom: CLEAR };

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
  order.unshift([FALLBACK, ['g']], [CLEAR, ['.']]);

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

// Animal models in the tileset export (batch 15); the client requests exactly these.
export const ANIMAL_MODELS = ['cow', 'pig', 'chicken'];
const FACE_NAMES = FACES.map((f) => f.name);

/** Unit box (footprint 1x1 centred on the origin, 1 high) textured with `species`'s faces; unknown species get grey faces. */
export function animalBoxes(species) {
  const faces = ANIMAL_MODELS.includes(species) ? Object.fromEntries(FACE_NAMES.map((n) => [n, `${species}.${n}`])) : {};
  return [{ lo: [-0.5, 0, -0.5], size: [1, 1, 1], faces }];
}

const HURT_TINT = [0.85, 0.12, 0.1];

/**
 * Pose of animal `a` ({ pos, size: [w, h], facing (degrees), dying, hurt }) for
 * drawing the unit box of animalBoxes: { model: column-major Float32Array(16)
 * (scale to size, then turn about the bottom centre, then move to pos), tint }.
 */
export function animalPose(a) {
  const [w, h] = a.size, f = a.facing * Math.PI / 180;
  // Facing 0 looks along -z, so local x is (cos f, 0, sin f) and local z
  // (-sin f, 0, cos f). Tipping rolls the body 90 degrees about local z:
  // x goes up and y goes to the -x side, so the lift is half the width.
  const c = Math.cos(f), s = Math.sin(f);
  const model = new Float32Array(a.dying
    ? [0, w, 0, 0, -c * h, 0, -s * h, 0, -s * w, 0, c * w, 0, a.pos[0], a.pos[1] + w / 2, a.pos[2], 1]
    : [c * w, 0, s * w, 0, 0, h, 0, 0, -s * w, 0, c * w, 0, a.pos[0], a.pos[1], a.pos[2], 1]);
  return { model, tint: a.hurt ? HURT_TINT : [1, 1, 1] };
}
