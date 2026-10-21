// The raft as a box model instead of meshed blocks. raftBoxes() reads the
// scene's raft_blocks() array ((z * size + x) * 128 + y, 0 = air) and returns
// boxes for models.js boxVertices, all inside the cells that hold raft blocks:
// a row of logs under a plank deck. Plank cells get the model; any other raft
// block (the deck stations) is a plain cube with grey faces until it has a model.
// Coordinates are region-local; the caller translates by the region origin and
// the sim's raft offset.

export const RAFT_PLANKS = 25; // block id of raft_planks (data/blocks.toml)
export const RAFT_TEXTURES = ['raft_planks', 'raft_edge', 'raft_bottom'];
const SIDES = [['left', -1, 0], ['right', 1, 0], ['front', 0, -1], ['back', 0, 1]];
const LOGS = 4; // a row of logs fills one cell: 4 logs, each a quarter block thick
const DECK = 0.5; // plank deck thickness; the logs sit under it, lashed
const HEIGHT = 128;

/** Face names of every model face the raft requests (`raft_log.*` from batch 17, plus RAFT_TEXTURES). */
export const RAFT_FACES = [
  ...['front', 'back', 'left', 'right', 'top', 'bottom'].map((f) => `raft_log.${f}`),
  ...RAFT_TEXTURES,
];

/** Boxes ({ lo, size, faces }) for the raft cells of `blocks`, in a region `size` blocks square. */
export function raftBoxes(blocks, size) {
  const at = (x, y, z) => x >= 0 && z >= 0 && x < size && z < size && blocks[(z * size + x) * HEIGHT + y] !== 0;
  const boxes = [];
  for (let z = 0; z < size; z++) {
    for (let x = 0; x < size; x++) {
      for (let y = 0; y < HEIGHT; y++) {
        const id = blocks[(z * size + x) * HEIGHT + y];
        if (id === 0) continue;
        if (id !== RAFT_PLANKS) { boxes.push({ lo: [x, y, z], size: [1, 1, 1], faces: {} }); continue; }
        const faces = { top: 'raft_planks', bottom: 'raft_bottom' };
        for (const [name, dx, dz] of SIDES) faces[name] = at(x + dx, y, z + dz) ? 'raft_planks' : 'raft_edge';
        boxes.push({ lo: [x, y + 1 - DECK, z], size: [1, DECK, 1], faces });
        const w = 1 / LOGS;
        const log = Object.fromEntries(['front', 'back', 'left', 'right', 'top', 'bottom'].map((f) => [f, `raft_log.${f}`]));
        for (let i = 0; i < LOGS; i++) boxes.push({ lo: [x + i * w, y + 1 - DECK - w, z], size: [w, w, 1], faces: log });
      }
    }
  }
  return boxes;
}
