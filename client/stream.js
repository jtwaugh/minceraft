// Region streaming for the client, apart from WebGL so it runs in Node. The
// sim owns what is loaded (Walk.drain_stream); this mirrors it: load events
// become a queue of regions to mesh, nearest the viewer first and at most
// `budget` per frame, and unload events become regions whose meshes to free.
//
//   const s = createStream({ budget: 1 });
//   const { mesh, free, stale } = s.frame(walk.drain_stream(), centre);
//   // mesh: [{ rx, rz }] to mesh and upload now; free: [{ rx, rz }] to delete;
//   // stale: [{ rx, rz }] to mesh again, at most one and only when mesh is empty
//
// `events` is the flat [kind, rx, rz, ...] list of drain_stream: kind 1 a
// region loaded, 0 unloaded. A region the sim loads twice is meshed once.
//
// Meshing a region makes its meshed N/S/E/W neighbours stale: they were padded
// with this region's edge as their own, which can leave a hole in a cliff wall.

export const REGION_SIZE = 128;

/** The region holding world column (x, z). */
export function regionOf(x, z) {
  return { rx: Math.floor(x / REGION_SIZE), rz: Math.floor(z / REGION_SIZE) };
}

export function createStream({ budget = 1 } = {}) {
  const queued = new Map(); // "rx,rz" -> { rx, rz }: loaded, not yet meshed
  const meshed = new Set();
  const stale = new Map(); // "rx,rz" -> { rx, rz }: meshed, to mesh again on a spare frame
  const key = (rx, rz) => `${rx},${rz}`;
  return {
    frame(events, centre) {
      const free = [];
      for (let i = 0; i < events.length; i += 3) {
        const [kind, rx, rz] = [events[i], events[i + 1], events[i + 2]];
        const k = key(rx, rz);
        if (kind === 1) {
          if (!meshed.has(k)) queued.set(k, { rx, rz });
        } else if (!queued.delete(k) && meshed.delete(k)) {
          stale.delete(k);
          free.push({ rx, rz }); // queued-only regions have no meshes to free
        }
      }
      const dist = (r) => Math.max(Math.abs(r.rx - centre.rx), Math.abs(r.rz - centre.rz));
      const near = (r) => Math.hypot(r.rx - centre.rx, r.rz - centre.rz);
      const mesh = [...queued.values()].sort((a, b) => dist(a) - dist(b) || near(a) - near(b)).slice(0, budget);
      for (const r of mesh) {
        queued.delete(key(r.rx, r.rz));
        meshed.add(key(r.rx, r.rz));
        for (const [dx, dz] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
          const n = { rx: r.rx + dx, rz: r.rz + dz };
          if (meshed.has(key(n.rx, n.rz))) stale.set(key(n.rx, n.rz), n);
        }
        stale.delete(key(r.rx, r.rz));
      }
      const next = mesh.length ? null : stale.entries().next().value;
      if (next) stale.delete(next[0]);
      return { mesh, free, stale: next ? [next[1]] : [] };
    },
    // `pairs` is the flat [rx, rz, ...] of Walk.drain_edited_regions: the meshed
    // regions among them, to mesh again (so no longer stale). Unmeshed ones pick
    // the edit up when meshed.
    edited(pairs) {
      const out = [];
      for (let i = 0; i < pairs.length; i += 2) {
        const k = key(pairs[i], pairs[i + 1]);
        if (meshed.has(k)) {
          out.push({ rx: pairs[i], rz: pairs[i + 1] });
          stale.delete(k);
        }
      }
      return out;
    },
    get pending() { return queued.size; },
    get meshed() { return meshed.size; },
    get stale() { return stale.size; },
  };
}
