// Mesh crack check (docs/design/play-critic-perf-render.md section 4): a pure function over a
// mesh part's vertex bytes (layout in viewer/mesh.js). Each fault is plain data,
// `{ check, face, detail }`, `face` the index of the quad it is about.
//
// Run it on the `opaque` part only. It checks full-block faces: quads at least one block wide
// both ways whose plane is on the block grid (greedy-merged rectangles included):
// - `grid`: every vertex sits on a whole block position, at a corner of the quad's rectangle;
// - `overlap`: no two faces of the same direction in the same plane cover the same block;
// - `gap`: every edge of a face is shared, by a face of the same direction and plane on the
//   other side or by a face meeting it at the edge (a step, a pit or a hidden top), so a
//   flat area has no holes. Edges on the meshed area's border (`size`), which meet faces of
//   the neighbouring area (region borders are a check of their own), and on its lowest plane
//   (solid ground is clipped there, so the bottom faces are left out) are not checked.
// Skipped, as they are not on the grid: snow layer tops and sides (1/8 high) and cactus
// (inset 1/16) in `opaque` (their edges still count where they lie along a full-block face's
// edge), and the other parts: water (lowered surfaces), ice, cutout (leaves, plants, bushes,
// shellfish) and models (client/modelgl.js).

import { VERTEX_BYTES, POS_SCALE } from '../viewer/mesh.js';

// One block in raw position units.
const S = Math.round(1 / POS_SCALE);
const DIR = ['x', 'y', 'z'];
const onGrid = (t) => t % S === 0;

// The 4 corners of quad q, in raw units: [[x, y, z] x 4].
function corners(dv, q) {
  return [0, 1, 2, 3].map((j) => [0, 1, 2].map((a) => dv.getInt16((q * 4 + j) * VERTEX_BYTES + a * 2, true)));
}

/**
 * Faults in mesh part `{ verts, faces }`, whose x and z run from 0 to `size` (omit `size`
 * to check edges at every x and z). Positions in faults are blocks, local to the mesh.
 */
export function meshCracks({ verts, faces }, size = null) {
  const dv = new DataView(verts), faults = [];
  const planes = new Map(); // "axis,dir,plane" -> Map("u,v" -> face)
  const edges = new Map(); // unit edge "axis:x,y,z" -> bit mask of the normal axes of faces along it
  const names = [];
  let minY = Infinity;
  for (let q = 0; q < faces; q++) {
    const v = corners(dv, q);
    const lo = [0, 1, 2].map((a) => Math.min(...v.map((p) => p[a]))), hi = [0, 1, 2].map((a) => Math.max(...v.map((p) => p[a])));
    minY = Math.min(minY, lo[1]);
    // the normal axis is the flattest; its plane is where most corners sit
    const pa = [0, 1, 2].reduce((m, a) => (hi[a] - lo[a] < hi[m] - lo[m] ? a : m), 0);
    const counts = new Map(v.map((p) => [p[pa], v.filter((r) => r[pa] === p[pa]).length]));
    const plane = [...counts].reduce((m, c) => (c[1] > m[1] ? c : m))[0];
    // unit edges between consecutive corners that run along a grid line
    for (let j = 0; j < 4; j++) {
      const a = v[j], b = v[(j + 1) % 4], ax = [0, 1, 2].filter((k) => a[k] !== b[k]);
      if (ax.length !== 1 || !a.every(onGrid) || !b.every(onGrid)) continue;
      const e = ax[0], p = a.slice();
      for (let t = Math.min(a[e], b[e]); t < Math.max(a[e], b[e]); t += S) {
        p[e] = t;
        const k = `${e}:${p.map((c) => c / S)}`;
        edges.set(k, (edges.get(k) ?? 0) | (1 << pa));
      }
    }
    const [ua, va] = [0, 1, 2].filter((a) => a !== pa);
    if (!onGrid(plane) || hi[ua] - lo[ua] < S || hi[va] - lo[va] < S) continue;
    // direction from the diagonals' cross product, which one stray corner can't flip
    const d1 = v[2].map((t, a) => t - v[0][a]), d2 = v[3].map((t, a) => t - v[1][a]);
    const dir = Math.sign(d1[ua] * d2[va] - d1[va] * d2[ua]) * (pa === 1 ? -1 : 1);
    const name = `face ${q} (${dir > 0 ? '+' : '-'}${DIR[pa]} at ${lo.map((t) => Math.floor(t / S))})`;
    const exact = [ua, va].every((a) => onGrid(lo[a]) && onGrid(hi[a])) && v.every((p) => p[pa] === plane && [ua, va].every((a) => p[a] === lo[a] || p[a] === hi[a]));
    if (!exact) faults.push({ check: 'grid', face: q, detail: `${name}: corners off the block grid: ${v.map((p) => p.join(',')).join(' ')}` });
    const key = `${pa},${dir},${plane / S}`;
    if (!planes.has(key)) planes.set(key, new Map());
    const cells = planes.get(key);
    for (let u = Math.round(lo[ua] / S); u < Math.round(hi[ua] / S); u++) for (let w = Math.round(lo[va] / S); w < Math.round(hi[va] / S); w++) {
      const c = `${u},${w}`;
      if (cells.has(c)) faults.push({ check: 'overlap', face: q, detail: `${name} overlaps face ${cells.get(c)} at ${u},${w}` });
      else cells.set(c, q);
    }
    names[q] = name;
  }
  // gaps: a covered cell next to an open one, with no face meeting the edge between them
  const border = (a, t) => a !== 1 && size !== null && (t === 0 || t === size);
  for (const [key, cells] of planes) {
    const [pa, dir, plane] = key.split(',').map(Number), [ua, va] = [0, 1, 2].filter((a) => a !== pa);
    for (const [c, q] of cells) {
      const [u, w] = c.split(',').map(Number);
      for (const [du, dw] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        if (cells.has(`${u + du},${w + dw}`)) continue;
        // the edge: along va at ua = u or u + 1, or along ua at va = w or w + 1
        const p = [0, 0, 0], ea = du ? va : ua, fa = du ? ua : va;
        p[pa] = plane; p[ua] = u; p[va] = w; p[fa] += (du || dw) > 0 ? 1 : 0;
        if (border(fa, p[fa]) || border(pa, plane) ||(fa === 1 && p[1] * S === minY)) continue;
        if ((edges.get(`${ea}:${p}`) ?? 0) & ~(1 << pa)) continue;
        const open = [0, 0, 0];
        open[pa] = dir > 0 ? plane - 1 : plane; open[ua] = u + du; open[va] = w + dw;
        faults.push({ check: 'gap', face: q, detail: `${names[q]}: open edge toward the ${dir > 0 ? '+' : '-'}${DIR[pa]} face of block ${open}` });
      }
    }
  }
  return faults;
}
