// Flyover planning: pick an island, find interesting terrain nearby and turn
// them into a smooth, timed camera path. Pure functions over the wasm
// module's coarse_heightmap survey; no DOM or WebGL, so Node can run it too
// (see viewer/flyover/README.md).

export const SEA = 64;
export const SURVEY_STEP = 8;     // blocks between survey samples (one per 2 lattice columns)
export const SURVEY_N = 256;      // samples per side: 2048 x 2048 blocks, ~0.5 s in wasm
export const REFINE_STEP = 4;     // path-corridor survey for camera clearance (every lattice column)

// Biome ids (data/biomes.toml).
export const BIOME = { rainforest: 0, swamp: 1, seasonal: 2, forest: 3, savanna: 4, shrubland: 5,
  taiga: 6, desert: 7, plains: 8, ice_desert: 9, tundra: 10 };
const WOODED = new Set([BIOME.rainforest, BIOME.seasonal, BIOME.forest, BIOME.taiga]);
// Snow-topped: below cool_t, the snow-layer temperature (data/biomes.toml).
export const SNOWY = new Set([BIOME.taiga, BIOME.ice_desert, BIOME.tundra]);

// ---------------------------------------------------------------- survey grids

/** Coarse survey of an n x n grid of samples centred on world (cx, cz). */
export function survey(wasm, seed, cx, cz, step = SURVEY_STEP, n = SURVEY_N) {
  const x0 = Math.floor((cx - (n * step) / 2) / 4) * 4;
  const z0 = Math.floor((cz - (n * step) / 2) / 4) * 4;
  return grid(wasm.coarse_heightmap(seed, x0, z0, step, n, n), x0, z0, step, n, n);
}

/** Coarse survey covering the world rectangle [xa, xb] x [za, zb]. */
export function surveyRect(wasm, seed, xa, za, xb, zb, step = REFINE_STEP) {
  const x0 = Math.floor(xa / 4) * 4, z0 = Math.floor(za / 4) * 4;
  const nx = Math.ceil((xb - x0) / step) + 1, nz = Math.ceil((zb - z0) / step) + 1;
  return grid(wasm.coarse_heightmap(seed, x0, z0, step, nx, nz), x0, z0, step, nx, nz);
}

function grid(raw, x0, z0, step, nx, nz) {
  const n = nx * nz;
  const g = { x0, z0, step, nx, nz, top: new Uint8Array(n), biome: new Uint8Array(n),
    gaps: new Uint8Array(n), gapH: new Uint8Array(n) };
  for (let i = 0; i < n; i++) {
    g.top[i] = raw[i * 4]; g.biome[i] = raw[i * 4 + 1];
    g.gaps[i] = raw[i * 4 + 2]; g.gapH[i] = raw[i * 4 + 3];
  }
  return g;
}

const wx = (g, i) => g.x0 + i * g.step;
const wz = (g, k) => g.z0 + k * g.step;
const land = (g, c) => g.top[c] >= SEA;

/** Terrain height (top solid y, at least sea level) at a world point, bilinear. */
export function heightAt(g, x, z) {
  const fx = Math.max(0, Math.min(g.nx - 1.001, (x - g.x0) / g.step));
  const fz = Math.max(0, Math.min(g.nz - 1.001, (z - g.z0) / g.step));
  const i = Math.floor(fx), k = Math.floor(fz), tx = fx - i, tz = fz - k;
  const h = (a, b) => Math.max(SEA - 1, g.top[b * g.nx + a]);
  const a = h(i, k) + (h(i + 1, k) - h(i, k)) * tx;
  const b = h(i, k + 1) + (h(i + 1, k + 1) - h(i, k + 1)) * tx;
  return a + (b - a) * tz;
}

/** Highest terrain within radius r of a world point (for camera clearance). */
export function maxHeightNear(g, x, z, r) {
  const i0 = Math.max(0, Math.floor((x - r - g.x0) / g.step)), i1 = Math.min(g.nx - 1, Math.ceil((x + r - g.x0) / g.step));
  const k0 = Math.max(0, Math.floor((z - r - g.z0) / g.step)), k1 = Math.min(g.nz - 1, Math.ceil((z + r - g.z0) / g.step));
  let m = SEA - 1, wooded = false;
  for (let k = k0; k <= k1; k++) for (let i = i0; i <= i1; i++) {
    const c = k * g.nx + i;
    if (g.top[c] > m) m = g.top[c];
    if (land(g, c) && WOODED.has(g.biome[c])) wooded = true;
  }
  return { h: m, wooded };
}

// Connected components (4-neighbour) of cells where pred(c) holds.
function components(g, pred) {
  const { nx, nz } = g, lab = new Int32Array(nx * nz).fill(-1), out = [];
  for (let s = 0; s < nx * nz; s++) {
    if (lab[s] >= 0 || !pred(s)) continue;
    const comp = { id: out.length, cells: [], edge: false };
    const st = [s]; lab[s] = comp.id;
    while (st.length) {
      const c = st.pop(), i = c % nx, k = (c / nx) | 0;
      comp.cells.push(c);
      if (i === 0 || k === 0 || i === nx - 1 || k === nz - 1) comp.edge = true;
      if (i > 0) visit(c - 1); if (i < nx - 1) visit(c + 1);
      if (k > 0) visit(c - nx); if (k < nz - 1) visit(c + nx);
    }
    out.push(comp);
    function visit(q) { if (lab[q] < 0 && pred(q)) { lab[q] = comp.id; st.push(q); } }
  }
  return { comps: out, lab };
}

// Multi-source BFS distance (in cells, 4-neighbour) from the cells in `src`
// through cells where pass(c) holds, up to maxD.
function bfsDist(g, src, pass, maxD) {
  const d = new Int16Array(g.nx * g.nz).fill(-1);
  let q = [];
  for (const c of src) { d[c] = 0; q.push(c); }
  for (let step = 1; step <= maxD && q.length; step++) {
    const next = [];
    for (const c of q) {
      const i = c % g.nx;
      for (const n of [i > 0 ? c - 1 : -1, i < g.nx - 1 ? c + 1 : -1, c - g.nx, c + g.nx]) {
        if (n < 0 || n >= d.length || d[n] >= 0 || !pass(n)) continue;
        d[n] = step; next.push(n);
      }
    }
    q = next;
  }
  return d;
}

function centroid(g, cells) {
  let sx = 0, sz = 0;
  for (const c of cells) { sx += c % g.nx; sz += (c / g.nx) | 0; }
  return [wx(g, sx / cells.length), wz(g, sz / cells.length)];
}

// The cell farthest from the component's boundary (its "middle"), so points
// for blob-shaped regions (a desert, a lake) land well inside them.
function innermost(g, cells) {
  const inside = new Uint8Array(g.nx * g.nz);
  for (const c of cells) inside[c] = 1;
  const edge = cells.filter((c) => {
    const i = c % g.nx;
    return i === 0 || i === g.nx - 1 || !inside[c - 1] || !inside[c + 1] || !inside[c - g.nx] || !inside[c + g.nx];
  });
  const d = bfsDist(g, edge, (c) => inside[c] === 1, 1000);
  let best = cells[0];
  for (const c of cells) if (d[c] > d[best]) best = c;
  return { cell: best, depth: d[best] };
}

const cellXZ = (g, c) => [wx(g, c % g.nx), wz(g, (c / g.nx) | 0)];

// ---------------------------------------------------------------- islands

/**
 * Islands in the survey: land components that don't touch its edge, sized
 * for a flyover start (about 35 to 240 blocks across). Sorted best first.
 */
export function findIslands(g) {
  const area = g.step * g.step;
  const { comps, lab } = components(g, (c) => land(g, c));
  const out = [];
  const midX = wx(g, g.nx / 2), midZ = wz(g, g.nz / 2);
  for (const comp of comps) {
    const blocks = comp.cells.length * area;
    if (comp.edge || blocks < 1200 || blocks > 60000) continue;
    let maxH = 0, wooded = 0, snowy = 0, peakCell = comp.cells[0];
    const biomes = new Map();
    for (const c of comp.cells) {
      if (g.top[c] > maxH) { maxH = g.top[c]; peakCell = c; }
      const b = g.biome[c];
      biomes.set(b, (biomes.get(b) || 0) + 1);
      if (WOODED.has(b)) wooded++;
      if (SNOWY.has(b)) snowy++;
    }
    // Open water around it: how far (up to 64 blocks) to the nearest other land.
    const d = bfsDist(g, comp.cells, (c) => !land(g, c) || lab[c] !== comp.id, 8);
    let iso = 8;
    for (let c = 0; c < d.length; c++) if (d[c] > 0 && land(g, c) && lab[c] !== comp.id) iso = Math.min(iso, d[c]);
    const [cx, cz] = centroid(g, comp.cells);
    const n = comp.cells.length;
    const variety = [...biomes.values()].filter((v) => v / n >= 0.1).length;
    const size = Math.exp(-((Math.log(blocks / 16000)) ** 2) / 2);  // best around 16k blocks^2
    const score = 2 * size + Math.min(maxH - SEA, 30) / 15 + 1.5 * wooded / n + 0.4 * variety
      + iso / 4 - snowy / n - Math.hypot(cx - midX, cz - midZ) / 1500;
    out.push({ cells: comp.cells, blocks, cx, cz, maxH, peak: cellXZ(g, peakCell), iso: iso * g.step,
      wooded: wooded / n, snowy: snowy / n, variety, score, label: islandLabel(blocks, maxH, wooded / n, snowy / n) });
  }
  return out.sort((a, b) => b.score - a.score);
}

function islandLabel(blocks, maxH, wooded, snowy) {
  const size = blocks < 4000 ? 'Islet' : blocks < 20000 ? 'Island' : 'Large island';
  const kind = snowy > 0.5 ? 'snowy ' : wooded > 0.5 ? 'wooded ' : maxH > 84 ? 'hilly ' : '';
  return `${kind ? kind[0].toUpperCase() + kind.slice(1) : ''}${kind ? size.toLowerCase() : size}`;
}

// ---------------------------------------------------------------- features

/**
 * Candidate features across the survey: peaks, cliffs, overhangs, narrow
 * channels, lakes and biome showcases. Each is
 * { kind, label, x, z, y, score, dir? (unit [dx, dz] for cliffs/channels), len? }.
 */
export function findFeatures(g) {
  const { nx, nz, top } = g, out = [];
  const at = (i, k) => top[Math.max(0, Math.min(nz - 1, k)) * nx + Math.max(0, Math.min(nx - 1, i))];

  // Peaks: local maxima over 24 blocks with at least 14 blocks of prominence
  // within 48 blocks.
  for (let k = 3; k < nz - 3; k++) for (let i = 3; i < nx - 3; i++) {
    const h = at(i, k);
    if (h < 80) continue;
    let isMax = true, lo = h;
    for (let dk = -6; dk <= 6 && isMax; dk++) for (let di = -6; di <= 6; di++) {
      const v = at(i + di, k + dk);
      if (Math.abs(di) <= 3 && Math.abs(dk) <= 3 && v > h) { isMax = false; break; }
      lo = Math.min(lo, v);
    }
    if (!isMax || h - lo < 14) continue;
    out.push({ kind: 'peak', label: `Peak · y ${h}`, x: wx(g, i), z: wz(g, k), y: h,
      score: (h - SEA) / 12 + (h - lo) / 14 });
  }

  // Cliffs: a drop of 12+ blocks between neighbouring samples (8 blocks
  // apart). Clustered; the camera faces the cliff from its low side.
  const drop = new Int16Array(nx * nz), dirs = new Float32Array(nx * nz * 2);
  for (let k = 1; k < nz - 1; k++) for (let i = 1; i < nx - 1; i++) {
    const c = k * nx + i, h = top[c];
    if (h < SEA) continue;
    let best = 0, bx = 0, bz = 0;
    for (const [di, dk] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const dd = h - Math.max(SEA - 1, at(i + di, k + dk));
      if (dd > best) { best = dd; bx = di; bz = dk; }
    }
    if (best >= 12) { drop[c] = best; dirs[c * 2] = bx; dirs[c * 2 + 1] = bz; }
  }
  for (const comp of components(g, (c) => drop[c] > 0).comps) {
    if (comp.cells.length < 3) continue;
    let best = comp.cells[0], dx = 0, dz = 0, sum = 0;
    for (const c of comp.cells) { if (drop[c] > drop[best]) best = c; dx += dirs[c * 2]; dz += dirs[c * 2 + 1]; sum += drop[c]; }
    const l = Math.hypot(dx, dz) || 1;
    out.push({ kind: 'cliff', label: `Cliff · ${drop[best]} m drop`, ...xzy(g, best), dir: [dx / l, dz / l],
      score: Math.min(sum, 240) / 60 + drop[best] / 10 });
  }

  // Overhangs, arches and floating rock: survey columns with an air gap
  // above sea level under solid ground.
  for (const comp of components(g, (c) => g.gapH[c] >= 3).comps) {
    let best = comp.cells[0], sum = 0;
    for (const c of comp.cells) { sum += g.gapH[c]; if (g.gapH[c] > g.gapH[best]) best = c; }
    if (g.gapH[best] < 4) continue;
    // open side: the neighbour direction where the ground is lowest
    const i = best % nx, k = (best / nx) | 0;
    let open = [1, 0], lo = 999;
    for (const [di, dk] of [[2, 0], [-2, 0], [0, 2], [0, -2]]) {
      const v = at(i + di, k + dk); if (v < lo) { lo = v; open = [Math.sign(di), Math.sign(dk)]; }
    }
    const floating = lo < SEA && comp.cells.length <= 3;
    out.push({ kind: 'overhang', label: floating ? 'Floating rock' : comp.cells.length >= 4 ? 'Overhangs' : 'Overhang',
      ...xzy(g, best), dir: open, gap: g.gapH[best], score: 1.5 + Math.min(sum, 100) / 25 + g.gapH[best] / 6 });
  }

  // Narrow channels: water with land within two samples on opposite sides.
  const water = (i, k) => at(i, k) < SEA;
  const narrow = new Uint8Array(nx * nz);
  for (let k = 2; k < nz - 2; k++) for (let i = 2; i < nx - 2; i++) {
    if (!water(i, k)) continue;
    const ew = (!water(i - 1, k) || !water(i - 2, k)) && (!water(i + 1, k) || !water(i + 2, k));
    const ns = (!water(i, k - 1) || !water(i, k - 2)) && (!water(i, k + 1) || !water(i, k + 2));
    if (ew !== ns) narrow[k * nx + i] = 1; // one axis only, so not just a puddle
  }
  for (const comp of components(g, (c) => narrow[c] === 1).comps) {
    if (comp.cells.length < 6) continue;
    const axis = principalAxis(g, comp.cells);
    if (axis.len < 6 * g.step) continue;
    out.push({ kind: 'channel', label: 'Channel', x: axis.cx, z: axis.cz, y: SEA - 1, dir: axis.dir,
      len: axis.len, score: 1 + axis.len / 48 });
  }

  // Lakes: inland water not connected to the survey edge.
  for (const comp of components(g, (c) => !land(g, c)).comps) {
    if (comp.edge || comp.cells.length < 4 || comp.cells.length > 300) continue;
    const { cell } = innermost(g, comp.cells);
    out.push({ kind: 'lake', label: 'Lake', ...xzy(g, cell), y: SEA - 1, score: 1 + Math.log2(comp.cells.length) / 3 });
  }

  // Biome showcases: the middle of large deserts, snowfields, rainforests and
  // swamps (they read clearly from the air).
  const SHOW = { [BIOME.desert]: 'Desert', [BIOME.ice_desert]: 'Ice desert', [BIOME.tundra]: 'Tundra',
    [BIOME.taiga]: 'Taiga', [BIOME.rainforest]: 'Rainforest', [BIOME.swamp]: 'Swampland' };
  for (const [id, name] of Object.entries(SHOW)) {
    const b = +id;
    for (const comp of components(g, (c) => land(g, c) && g.biome[c] === b).comps) {
      if (comp.cells.length < 120) continue;
      const { cell, depth } = innermost(g, comp.cells);
      out.push({ kind: 'biome', label: name, ...xzy(g, cell), score: 0.8 + Math.log2(comp.cells.length / 120) / 2 + depth / 10 });
    }
  }
  return out;
}

const xzy = (g, c) => { const [x, z] = cellXZ(g, c); return { x, z, y: g.top[c] }; };

function principalAxis(g, cells) {
  const pts = cells.map((c) => cellXZ(g, c));
  const cx = pts.reduce((s, p) => s + p[0], 0) / pts.length, cz = pts.reduce((s, p) => s + p[1], 0) / pts.length;
  let sxx = 0, szz = 0, sxz = 0;
  for (const [x, z] of pts) { sxx += (x - cx) ** 2; szz += (z - cz) ** 2; sxz += (x - cx) * (z - cz); }
  const a = 0.5 * Math.atan2(2 * sxz, sxx - szz), dir = [Math.cos(a), Math.sin(a)];
  let lo = 0, hi = 0;
  for (const [x, z] of pts) { const t = (x - cx) * dir[0] + (z - cz) * dir[1]; lo = Math.min(lo, t); hi = Math.max(hi, t); }
  return { cx: cx + dir[0] * (lo + hi) / 2, cz: cz + dir[1] * (lo + hi) / 2, dir, len: hi - lo };
}

/**
 * Choose up to `max` features near the island: best score first, spaced at
 * least `gap` blocks apart, at most two of a kind, within `radius` of the
 * island centre.
 */
export function chooseFeatures(features, island, { max = 4, radius = 420, gap = 110 } = {}) {
  const near = features.filter((f) => Math.hypot(f.x - island.cx, f.z - island.cz) <= radius)
    .sort((a, b) => b.score - a.score);
  const picked = [], kinds = {};
  for (const f of near) {
    if (picked.length >= max) break;
    if ((kinds[f.kind] || 0) >= 2) continue;
    if (picked.some((p) => Math.hypot(p.x - f.x, p.z - f.z) < gap)) continue;
    if (Math.hypot(f.x - island.cx, f.z - island.cz) < 40) continue; // the island segment covers it
    picked.push(f); kinds[f.kind] = (kinds[f.kind] || 0) + 1;
  }
  return picked;
}

/**
 * Score islands by themselves plus the features within reach, so the start
 * is a good island with something worth flying to.
 */
export function rankIslands(islands, features) {
  for (const isl of islands) {
    const near = chooseFeatures(features, isl);
    isl.reach = near.reduce((s, f) => s + Math.min(f.score, 4), 0);
    isl.total = isl.score + 0.35 * isl.reach;
  }
  return islands.sort((a, b) => b.total - a.total);
}

// ---------------------------------------------------------------- route

// Visit order: open tour from the island, nearest neighbour then 2-opt.
function tour(start, pts) {
  const left = pts.slice(), order = [];
  let cur = start;
  while (left.length) {
    let bi = 0;
    for (let i = 1; i < left.length; i++) if (dist(cur, left[i]) < dist(cur, left[bi])) bi = i;
    cur = left.splice(bi, 1)[0]; order.push(cur);
  }
  const path = [start, ...order];
  for (let improved = true; improved;) {
    improved = false;
    for (let i = 1; i < path.length - 1; i++) for (let j = i + 1; j < path.length; j++) {
      const a = path[i - 1], b = path[i], c = path[j], d = path[j + 1];
      const before = dist(a, b) + (d ? dist(c, d) : 0), after = dist(a, c) + (d ? dist(b, d) : 0);
      if (after < before - 1e-6) { path.splice(i, j - i + 1, ...path.slice(i, j + 1).reverse()); improved = true; }
    }
  }
  return path.slice(1);
}
const dist = (a, b) => Math.hypot(a.x - b.x, a.z - b.z);
const norm2 = ([x, z]) => { const l = Math.hypot(x, z) || 1; return [x / l, z / l]; };

/**
 * Keyframes for the flight. Each is { eye: [x, y, z], target: [x, y, z],
 * speed, label?, hold? }: eye is a camera position the path passes through,
 * target where it looks there, speed the cruise speed (blocks/s) nearby.
 * Feature shots also have `look`, the point the camera must hold in frame
 * around that keyframe (buildPath aims at it explicitly).
 */
export function keyframes(island, features) {
  const keys = [];
  const order = tour({ x: island.cx, z: island.cz }, features);
  const first = order[0] || { x: island.cx + 1, z: island.cz };
  const out = norm2([first.x - island.cx, first.z - island.cz]);
  const r = Math.sqrt(island.blocks / Math.PI);           // island radius
  const ih = island.maxH;
  const centre = [island.cx, ih * 0.5 + SEA * 0.5, island.cz];

  // 1. Raft's-eye start: just above the water on the far side of the island,
  //    looking across it; then rise over the top and head out to sea.
  keys.push({ eye: [island.cx - out[0] * (r + 46), SEA + 3, island.cz - out[1] * (r + 46)],
    target: centre, speed: 8, label: `${island.label} · ${Math.round(island.blocks)} m²`, hold: 2.5 });
  const side = [-out[1], out[0]];
  keys.push({ eye: [island.cx - out[0] * (r + 12) + side[0] * r * 0.5, SEA + 8, island.cz - out[1] * (r + 12) + side[1] * r * 0.5],
    target: [island.peak[0], ih, island.peak[1]], speed: 11 });
  keys.push({ eye: [island.peak[0], ih + 16, island.peak[1]],
    target: [island.peak[0] + out[0] * 60, SEA + 2, island.peak[1] + out[1] * 60], speed: 14 });
  keys.push({ eye: [island.cx + out[0] * (r + 30), SEA + 14, island.cz + out[1] * (r + 30)],
    target: [island.cx + out[0] * (r + 120), SEA + 6, island.cz + out[1] * (r + 120)], speed: 25 });

  // 2. Each feature: a pass with the camera to one side, looking at it.
  let prev = { x: keys[keys.length - 1].eye[0], z: keys[keys.length - 1].eye[2] };
  order.forEach((f, n) => {
    const next = order[n + 1];
    const travel = norm2([(next ? next.x : f.x + (f.x - prev.x)) - prev.x, (next ? next.z : f.z + (f.z - prev.z)) - prev.z]);
    const perp = [-travel[1], travel[0]];
    const fy = Math.max(f.y, SEA);
    if (f.kind === 'channel') {
      // fly down the channel low over the water, entering at the end nearer us
      let d = f.dir;
      if ((f.x - prev.x) * d[0] + (f.z - prev.z) * d[1] < 0) d = [-d[0], -d[1]];
      const h = f.len / 2;
      keys.push({ eye: [f.x - d[0] * h, SEA + 5, f.z - d[1] * h], target: [f.x + d[0] * h, SEA + 3, f.z + d[1] * h], speed: 17, label: f.label, look: featurePoint(f) });
      keys.push({ eye: [f.x + d[0] * h, SEA + 6, f.z + d[1] * h], target: [f.x + d[0] * (h + 80), SEA + 4, f.z + d[1] * (h + 80)], speed: 20 });
    } else if (f.kind === 'cliff' || f.kind === 'overhang') {
      // stand off on the low/open side, facing it, sliding across
      const o = f.dir, off = f.kind === 'cliff' ? 34 : 26;
      const base = [f.x + o[0] * off, f.z + o[1] * off];
      const lat = [-o[1] * 18, o[0] * 18];
      const ey = f.kind === 'cliff' ? fy - 4 : fy - (f.gap || 6) / 2;
      keys.push({ eye: [base[0] - lat[0], ey, base[1] - lat[1]], target: [f.x, fy - 6, f.z], speed: 10, label: f.label, look: featurePoint(f) });
      keys.push({ eye: [base[0] + lat[0], ey + 4, base[1] + lat[1]], target: [f.x, fy - 4, f.z], speed: 11, look: featurePoint(f) });
    } else if (f.kind === 'peak') {
      // stand well off and near summit height, so the peak shows against the sky
      keys.push({ eye: [f.x + perp[0] * 75, fy + 2, f.z + perp[1] * 75], target: [f.x, fy, f.z], speed: 13, label: f.label, look: featurePoint(f) });
    } else if (f.kind === 'lake') {
      keys.push({ eye: [f.x - travel[0] * 30, SEA + 10, f.z - travel[1] * 30], target: [f.x + travel[0] * 50, SEA, f.z + travel[1] * 50], speed: 15, label: f.label, look: featurePoint(f) });
    } else {
      keys.push({ eye: [f.x - travel[0] * 20, fy + 22, f.z - travel[1] * 20], target: [f.x + travel[0] * 40, fy, f.z + travel[1] * 40], speed: 17, label: f.label });
    }
    prev = f;
  });

  // 3. Finale: climb and look back over everything flown past.
  const last = keys[keys.length - 1];
  const back = norm2([island.cx - last.eye[0], island.cz - last.eye[2]]);
  keys.push({ eye: [last.eye[0] - back[0] * 50, 128, last.eye[2] - back[1] * 50],
    target: [last.eye[0] + back[0] * 160, SEA, last.eye[2] + back[1] * 160], speed: 11, hold: 3 });
  return keys;
}

// Centripetal Catmull-Rom between p1 and p2 (alpha = 0.5), t in [0, 1].
function catmull(p0, p1, p2, p3, t) {
  const d = (a, b) => Math.max(1e-3, Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) ** 0.5);
  const t1 = d(p0, p1), t2 = t1 + d(p1, p2), t3 = t2 + d(p2, p3);
  const u = t1 + (t2 - t1) * t;
  const L = (a, b, ta, tb) => a.map((v, i) => ((tb - u) * v + (u - ta) * b[i]) / (tb - ta));
  const A1 = L(p0, p1, 0, t1), A2 = L(p1, p2, t1, t2), A3 = L(p2, p3, t2, t3);
  const B1 = L(A1, A2, 0, t2), B2 = L(A2, A3, t1, t3);
  return L(B1, B2, t1, t2);
}

function spline(points, perSeg) {
  const ext = [points[0].map((v, i) => 2 * v - points[1][i]), ...points,
    points[points.length - 1].map((v, i) => 2 * v - points[points.length - 2][i])];
  const out = [];
  for (let s = 0; s < points.length - 1; s++) {
    for (let j = 0; j < perSeg; j++) out.push({ p: catmull(ext[s], ext[s + 1], ext[s + 2], ext[s + 3], j / perSeg), u: s + j / perSeg });
  }
  out.push({ p: points[points.length - 1], u: points.length - 1 });
  return out;
}

function smooth(arr, sigma) {
  if (sigma <= 0) return arr.slice();
  const r = Math.ceil(sigma * 3), w = [];
  for (let i = -r; i <= r; i++) w.push(Math.exp(-(i * i) / (2 * sigma * sigma)));
  return arr.map((_, i) => {
    let s = 0, n = 0;
    for (let j = -r; j <= r; j++) { const k = Math.max(0, Math.min(arr.length - 1, i + j)); s += arr[k] * w[j + r]; n += w[j + r]; }
    return s / n;
  });
}

/**
 * Timed camera path through the keyframes. `ground` is a survey grid that
 * covers the route (surveyRect at REFINE_STEP) for terrain clearance.
 * Returns { duration, at(t) -> { eye, target, label, labelAlpha }, samples, keys }.
 */
export function buildPath(keys, ground, { clearance = 7, cruise = 30, fps = 30 } = {}) {
  const PER_SEG = 64;
  const eyeS = spline(keys.map((k) => k.eye), PER_SEG);
  const tgtS = spline(keys.map((k) => k.target), PER_SEG);
  const n = eyeS.length;

  // Clearance: never closer than `clearance` blocks over the highest terrain
  // within 10 blocks (+7 where trees might stand). Lift early and ease off
  // late: a running max over +-40 samples, then a Gaussian.
  const floor = eyeS.map(({ p }) => {
    const { h, wooded } = maxHeightNear(ground, p[0], p[2], 10);
    return h + 1 + clearance + (wooded ? 7 : 2);
  });
  const lifted = eyeS.map(({ p }, i) => Math.max(p[1], floor[i]));
  const W = 40, held = lifted.map((_, i) => {
    let m = -Infinity;
    for (let j = Math.max(0, i - W); j <= Math.min(n - 1, i + W); j++) m = Math.max(m, lifted[j]);
    return m;
  });
  const ys = smooth(held, 16).map((y, i) => Math.min(150, Math.max(y, floor[i])));
  const eyes = eyeS.map(({ p }, i) => [p[0], ys[i], p[2]]);

  // Speed: each keyframe's own speed within ~25 blocks of it, rising to
  // `cruise` on the long legs between them.
  const arc = [0];
  for (let i = 1; i < n; i++) arc.push(arc[i - 1] + Math.hypot(eyes[i][0] - eyes[i - 1][0], eyes[i][1] - eyes[i - 1][1], eyes[i][2] - eyes[i - 1][2]));
  const keyArc = keys.map((_, k) => arc[Math.min(n - 1, k * PER_SEG)]);
  const v = smooth(arc.map((s) => {
    let best = Infinity, sp = cruise;
    keys.forEach((k, j) => { const d = Math.abs(s - keyArc[j]); if (d < best) { best = d; sp = k.speed; } });
    const f = Math.max(0, Math.min(1, (best - 25) / 70));
    return sp + (cruise - sp) * f * f * (3 - 2 * f);
  }), 8);
  const times = [0];
  for (let i = 1; i < n; i++) times.push(times[i - 1] + (arc[i] - arc[i - 1]) / Math.max(0.5, v[i]));
  // holds: linger at a keyframe (time is added there; the camera still drifts)
  let extra = 0;
  const keyTime = [];
  for (let i = 0, kk = 0; i < n; i++) {
    if (Math.abs(eyeS[i].u - kk) < 1e-9 && kk < keys.length) { extra += keys[kk].hold || 0; keyTime[kk] = times[i] + extra - (keys[kk].hold || 0) / 2; kk++; }
    times[i] += extra;
  }
  const total = times[n - 1];
  const duration = total + Math.min(3, total / 4); // easing costs half of each ramp at both ends

  // View direction: towards the keyframe targets, pitch-clamped and smoothed
  // so pans never whip. Clearance lifts the eye but not the target, which can
  // tip the view straight down, so keep the pitch between about -25 and +9
  // degrees.
  const clampPitch = (d, lo = -0.42, hi = 0.15) => {
    const h = Math.hypot(d[0], d[2]) || 1e-6, y = Math.max(lo, Math.min(hi, d[1]));
    const k = Math.sqrt(1 - y * y) / h;
    return [d[0] * k, y, d[2] * k];
  };
  const dirs = eyes.map((e, i) => clampPitch(norm3(tgtS[i].p.map((t, a) => t - e[a]))));
  const sd = [0, 1, 2].map((a) => smooth(dirs.map((d) => d[a]), 24));
  // Then aim: around each feature shot the camera turns onto the feature's
  // point and holds it, fully within 1.5 s of the keyframe, easing out by 3.5 s.
  const aims = keys.map((k, j) => k.look && keyTime[j] !== undefined ? { p: k.look, t: keyTime[j] } : null).filter(Boolean);
  if (aims.length) {
    const aimed = eyes.map((e, i) => {
      let w = 0, p = null;
      for (const a of aims) {
        const dt = Math.abs(times[i] - a.t), x = Math.max(0, Math.min(1, (3.5 - dt) / 2));
        const wa = x * x * (3 - 2 * x);
        if (wa > w) { w = wa; p = a.p; }
      }
      const base = [sd[0][i], sd[1][i], sd[2][i]];
      if (!p) return base;
      const look = clampPitch(norm3(p.map((v, a) => v - e[a])), -0.75, 0.3);
      return norm3(base.map((v, a) => v + (look[a] - v) * w));
    });
    for (let a = 0; a < 3; a++) sd[a] = smooth(aimed.map((d) => d[a]), 4);
  }

  function at(tWall) {
    const tt = Math.max(0, Math.min(total, easeInv(tWall)));
    let lo = 0, hi = n - 1;
    while (hi - lo > 1) { const m = (lo + hi) >> 1; if (times[m] <= tt) lo = m; else hi = m; }
    const f = times[hi] > times[lo] ? (tt - times[lo]) / (times[hi] - times[lo]) : 0;
    const L = (a, b) => a.map((x, i) => x + (b[i] - x) * f);
    const eye = L(eyes[lo], eyes[hi]);
    const d = [0, 1, 2].map((a) => sd[a][lo] + (sd[a][hi] - sd[a][lo]) * f);
    const target = [eye[0] + d[0] * 50, eye[1] + d[1] * 50, eye[2] + d[2] * 50];
    // caption: the nearest labelled keyframe within -3 s .. +2.5 s
    let label = '', labelAlpha = 0;
    keys.forEach((k, i) => {
      if (!k.label || keyTime[i] === undefined) return;
      const dt = tt - keyTime[i];
      if (dt < -3 || dt > 2.5) return;
      const a = Math.min(1, (dt + 3) / 0.6, (2.5 - dt) / 0.6);
      if (a > labelAlpha) { label = k.label; labelAlpha = a; }
    });
    return { eye, target, label, labelAlpha };
  }
  // wall time -> path time: linear in the middle, quadratic ramps at the ends
  function easeInv(t) {
    const E = Math.min(3, total / 4), D = duration;
    if (t <= 0) return 0;
    if (t >= D) return total;
    if (t < E) return (t * t) / (2 * E);
    if (t > D - E) { const r = D - t; return total - (r * r) / (2 * E); }
    return t - E / 2;
  }
  return { duration, at, eyes, keys, fps };
}

// ---------------------------------------------------------------- visibility

// The flyover camera, as index.html renders it: vertical field of view
// (radians), aspect ratio, and the distance where fog reaches full sky colour.
export const VIEW = { fovY: 1.0, aspect: 16 / 9, fogEnd: 196 };
export const CHUNK = 64;
// Blocks added above the survey's highest point in a chunk for tree tops. The
// survey records the ground; the tallest trees (big oaks, tall spruces) stand
// well over 10 blocks above it, and a loose box only loads a few extra chunks.
export const TREE_CLEARANCE = 24;

/**
 * Which 64x64 chunks the camera can see, and when. Samples the path every
 * `step` seconds and keeps chunks whose box (survey minimum to maximum
 * height over the chunk, at least sea level, plus TREE_CLEARANCE) meets the
 * view frustum and lies within VIEW.fogEnd of the eye (radial distance to the
 * box), plus every chunk within `near` blocks of the camera. Each interval is
 * padded by `pad` seconds. Terrain occlusion is not tested: the camera flies
 * above the ground and some part of nearly every chunk in view shows (see
 * README). `full` is the first time (padded the same way) the camera comes
 * within FULL_DETAIL blocks of the chunk, or Infinity; before that it's drawn
 * with fast leaves. Returns [{ cx, cz, first, last, full }] sorted by first.
 */
export const FULL_DETAIL = 80;
export function visibilitySchedule(path, g, { step = 0.25, pad = 1, near = 40, margin = 4 } = {}) {
  const tanV = Math.tan(VIEW.fovY / 2), tanH = tanV * VIEW.aspect, far = VIEW.fogEnd;
  const seen = new Map(), spans = new Map();
  // lowest and highest survey height over the chunk (never below sea level),
  // plus tree clearance on top
  const span = (cx, cz) => {
    const k = cx + ',' + cz;
    let s = spans.get(k);
    if (!s) {
      let lo = Infinity, hi = SEA;
      for (let x = cx * CHUNK; x <= (cx + 1) * CHUNK + g.step; x += g.step)
        for (let z = cz * CHUNK; z <= (cz + 1) * CHUNK + g.step; z += g.step) {
          const h = heightAt(g, Math.min(x, (cx + 1) * CHUNK), Math.min(z, (cz + 1) * CHUNK));
          lo = Math.min(lo, h); hi = Math.max(hi, h);
        }
      spans.set(k, s = [Math.max(SEA, lo), hi + TREE_CLEARANCE]);
    }
    return s;
  };
  const mark = (cx, cz, t, d) => {
    const k = cx + ',' + cz;
    let e = seen.get(k);
    if (e) { e.first = Math.min(e.first, t); e.last = Math.max(e.last, t); } else seen.set(k, e = { cx, cz, first: t, last: t, full: Infinity });
    if (d <= FULL_DETAIL) e.full = Math.min(e.full, t);
  };
  for (let t = 0; t <= path.duration + 1e-9; t += step) {
    const { eye, target } = path.at(t);
    const f = norm3(target.map((v, i) => v - eye[i]));
    const r = norm3([-f[2], 0, f[0]]);
    const u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    // inward side planes as (normal . d + margin >= 0); a box is in view if its
    // best corner passes all four and it reaches past the near plane
    const planes = [
      [f[0] * tanH - r[0], f[1] * tanH - r[1], f[2] * tanH - r[2]],
      [f[0] * tanH + r[0], f[1] * tanH + r[1], f[2] * tanH + r[2]],
      [f[0] * tanV - u[0], f[1] * tanV - u[1], f[2] * tanV - u[2]],
      [f[0] * tanV + u[0], f[1] * tanV + u[1], f[2] * tanV + u[2]],
    ];
    const boxInView = (lo, hi) => {
      const best = (n) => [0, 1, 2].reduce((s, a) => s + n[a] * (n[a] >= 0 ? hi[a] - eye[a] : lo[a] - eye[a]), 0);
      return best(f) >= 0.3 && planes.every((n) => best(n) + margin >= 0);
    };
    for (let cz = Math.floor((eye[2] - far) / CHUNK); cz <= Math.floor((eye[2] + far) / CHUNK); cz++)
      for (let cx = Math.floor((eye[0] - far) / CHUNK); cx <= Math.floor((eye[0] + far) / CHUNK); cx++) {
        const dx = Math.max(cx * CHUNK - eye[0], 0, eye[0] - (cx + 1) * CHUNK);
        const dz = Math.max(cz * CHUNK - eye[2], 0, eye[2] - (cz + 1) * CHUNK);
        const d = Math.hypot(dx, dz);
        if (d > far) continue;
        const [y0, y1] = span(cx, cz);
        const dy = Math.max(y0 - eye[1], 0, eye[1] - y1);
        if (Math.hypot(d, dy) > far) continue;
        if (d <= near || boxInView([cx * CHUNK, y0, cz * CHUNK], [(cx + 1) * CHUNK, y1, (cz + 1) * CHUNK])) mark(cx, cz, t, d);
      }
  }
  return [...seen.values()].map((e) => ({ ...e, first: Math.max(0, e.first - pad), last: Math.min(path.duration, e.last + pad),
    full: Math.max(0, e.full - pad) }))
    .sort((a, b) => a.first - b.first);
}
const norm3 = (v) => { const l = Math.hypot(...v) || 1; return v.map((c) => c / l); };

/**
 * How well the camera frames each chosen feature: the seconds during which
 * its point is inside the middle 80% of the frame, nearer than `far`, and
 * not hidden by terrain (ray-marched over `g`), plus the closest it gets to
 * the frame centre (degrees). [{ feature, seconds, minAngle }].
 */
export function featureCoverage(path, features, g, { step = 0.1, far = 140, frac = 0.8 } = {}) {
  const tanV = Math.tan(VIEW.fovY / 2) * frac, tanH = Math.tan(VIEW.fovY / 2) * VIEW.aspect * frac;
  const out = features.map((f) => ({ feature: f, seconds: 0, first: -1, minAngle: 180 }));
  for (let t = 0; t <= path.duration; t += step) {
    const { eye, target } = path.at(t);
    const fw = norm3(target.map((v, i) => v - eye[i]));
    const r = norm3([-fw[2], 0, fw[0]]);
    const u = [r[1] * fw[2] - r[2] * fw[1], r[2] * fw[0] - r[0] * fw[2], r[0] * fw[1] - r[1] * fw[0]];
    out.forEach((o) => {
      const f = o.feature, p = featurePoint(f);
      const d = p.map((v, i) => v - eye[i]), L = Math.hypot(...d);
      const z = d[0] * fw[0] + d[1] * fw[1] + d[2] * fw[2];
      if (z > 0) o.minAngle = Math.min(o.minAngle, Math.acos(Math.min(1, z / L)) * 180 / Math.PI);
      if (z <= 0.5 || L > far) return;
      if (Math.abs(d[0] * r[0] + d[1] * r[1] + d[2] * r[2]) > z * tanH) return;
      if (Math.abs(d[0] * u[0] + d[1] * u[1] + d[2] * u[2]) > z * tanV) return;
      const n = Math.ceil(L / 2);
      for (let k = 1; k < n - 1; k++) {
        const q = eye.map((v, i) => v + d[i] * k / n);
        if (heightAt(g, q[0], q[2]) > q[1] + 1) return;
      }
      if (o.first < 0) o.first = t;
      o.seconds += step;
    });
  }
  return out;
}

// The point a shot should show: the top of a peak or cliff, the middle of an
// overhang's gap side, the water surface of a channel or lake.
export function featurePoint(f) {
  if (f.kind === 'channel' || f.kind === 'lake') return [f.x, SEA, f.z];
  if (f.kind === 'overhang') return [f.x, Math.max(f.y, SEA) - (f.gap || 6) / 2, f.z];
  return [f.x, Math.max(f.y, SEA), f.z];
}

// ---------------------------------------------------------------- whole plan

const inRect = (b, x, z, m) => x >= b.x0 + m && x <= b.x1 - m && z >= b.z0 + m && z <= b.z1 - m;

/**
 * Plan a flyover as plain data (safe to post from a worker): survey around
 * (cx, cz), pick the island and features, keyframes, clearance survey and
 * visibility schedule. With `bounds` ({ x0, z0, x1, z1 }, the map on screen)
 * it prefers an island whose whole route stays inside them; if none does it
 * plans freely and sets `moved` with `mapOrigin`, where a map of the same size
 * would hold the route. Returns null if the survey has no island at all.
 * Turn the result into a playable plan with hydrate().
 */
export function planData(wasm, seed, cx, cz, { bounds = null, ...opts } = {}) {
  const t0 = Date.now();
  const g = survey(wasm, seed, cx, cz);
  const islands = findIslands(g);
  if (!islands.length) return null;
  const features = findFeatures(g);
  const routeBox = (keys) => {
    let xa = Infinity, za = Infinity, xb = -Infinity, zb = -Infinity;
    for (const k of keys) { xa = Math.min(xa, k.eye[0]); xb = Math.max(xb, k.eye[0]); za = Math.min(za, k.eye[2]); zb = Math.max(zb, k.eye[2]); }
    return { xa, za, xb, zb };
  };
  let pick = null;
  if (bounds) {
    // islands wholly on the map, features on the map, route on the map
    const onMap = features.filter((f) => inRect(bounds, f.x, f.z, 40));
    const cand = rankIslands(islands.filter((isl) => isl.cells.every((c) => {
      const [x, z] = [wx(g, c % g.nx), wz(g, (c / g.nx) | 0)];
      return inRect(bounds, x, z, 24);
    })), onMap);
    for (const island of cand.slice(0, 6)) {
      const chosen = chooseFeatures(onMap, island, opts);
      const keys = keyframes(island, chosen);
      const b = routeBox(keys);
      if (inRect(bounds, b.xa, b.za, 16) && inRect(bounds, b.xb, b.zb, 16)) { pick = { island, chosen, keys }; break; }
    }
  }
  let moved = false;
  if (!pick) {
    rankIslands(islands, features);
    const island = islands[0], chosen = chooseFeatures(features, island, opts);
    pick = { island, chosen, keys: keyframes(island, chosen) };
    moved = !!bounds;
  }
  // Build the path, then check the camera really shows each feature: one
  // that's in good view for under MIN_VIEW seconds (hidden by terrain, or
  // never framed) is swapped for the next candidate, twice at most.
  const MIN_VIEW = 2.5;
  const pool = (bounds && !moved) ? features.filter((f) => inRect(bounds, f.x, f.z, 40)) : features;
  const dropped = new Set();
  let ground, path, coverage, box;
  for (let attempt = 0; ; attempt++) {
    box = routeBox(pick.keys);
    // Clearance survey (every lattice column) over the route.
    ground = surveyRect(wasm, seed, box.xa - 48, box.za - 48, box.xb + 48, box.zb + 48);
    path = buildPath(pick.keys, ground);
    coverage = featureCoverage(path, pick.chosen, g);
    const weak = coverage.filter((c) => c.seconds < MIN_VIEW).map((c) => c.feature);
    if (!weak.length || attempt === 2) break;
    weak.forEach((f) => dropped.add(f));
    const chosen = chooseFeatures(pool.filter((f) => !dropped.has(f)), pick.island, opts);
    const keys = keyframes(pick.island, chosen);
    const b = routeBox(keys);
    if (bounds && !moved && !(inRect(bounds, b.xa, b.za, 16) && inRect(bounds, b.xb, b.zb, 16))) break;
    pick = { island: pick.island, chosen, keys };
  }
  const { island, chosen, keys } = pick;
  const schedule = visibilitySchedule(path, g);
  const size = bounds ? bounds.x1 - bounds.x0 : 1024;
  const mapOrigin = moved
    ? [Math.round(((box.xa + box.xb) / 2 - size / 2) / 64) * 64, Math.round(((box.za + box.zb) / 2 - size / 2) / 64) * 64]
    : null;
  const { cells, ...islandInfo } = island;
  return { survey: g, ground, island: { ...islandInfo, cells: Int32Array.from(cells) }, chosen, keys, schedule,
    coverage: coverage.map((c) => ({ label: c.feature.label, seconds: c.seconds, first: c.first, minAngle: c.minAngle })),
    islandCount: islands.length, featureCount: features.length, moved, mapOrigin, ms: Date.now() - t0 };
}

/** Add the timed camera path to planData() output (rebuilt from keys and ground). */
export function hydrate(data) {
  return data && { ...data, path: buildPath(data.keys, data.ground) };
}

/** planData + hydrate in one go (Node, or the page without workers). */
export function planFlyover(wasm, seed, cx, cz, opts = {}) {
  return hydrate(planData(wasm, seed, cx, cz, opts));
}
