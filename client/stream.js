// Region streaming for the client, apart from WebGL so it runs in Node. The
// sim owns what is loaded (Walk.drain_stream); this mirrors it: load events
// become a queue of regions to mesh, nearest the viewer first and at most
// `budget` per frame, and unload events become regions whose meshes to free.
//
//   const s = createStream({ budget: 1 });
//   const { mesh, free } = s.frame(walk.drain_stream(), centre);
//   // mesh: [{ rx, rz }] to mesh and upload now; free: [{ rx, rz }] to delete
//
// `events` is the flat [kind, rx, rz, ...] list of drain_stream: kind 1 a
// region loaded, 0 unloaded. A region the sim loads twice is meshed once.

export const REGION_SIZE = 128;

/** The region holding world column (x, z). */
export function regionOf(x, z) {
  return { rx: Math.floor(x / REGION_SIZE), rz: Math.floor(z / REGION_SIZE) };
}

export function createStream({ budget = 1 } = {}) {
  const queued = new Map(); // "rx,rz" -> { rx, rz }: loaded, not yet meshed
  const meshed = new Set();
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
          free.push({ rx, rz }); // queued-only regions have no meshes to free
        }
      }
      const dist = (r) => Math.max(Math.abs(r.rx - centre.rx), Math.abs(r.rz - centre.rz));
      const near = (r) => Math.hypot(r.rx - centre.rx, r.rz - centre.rz);
      const mesh = [...queued.values()].sort((a, b) => dist(a) - dist(b) || near(a) - near(b)).slice(0, budget);
      for (const r of mesh) {
        queued.delete(key(r.rx, r.rz));
        meshed.add(key(r.rx, r.rz));
      }
      return { mesh, free };
    },
    get pending() { return queued.size; },
    get meshed() { return meshed.size; },
  };
}
