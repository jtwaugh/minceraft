// Upload queue (docs/design/stream-without-stalls.md section 4), apart from
// WebGL so it runs in Node. Meshes that are ready wait here and go to the GPU
// a few per frame, up to a vertex budget, so a burst of ready regions is spread
// over several frames.
//
//   const q = createUploadQueue({ budget: UPLOAD_VERTS_PER_FRAME });
//   q.add({ rx, rz, kind, verts, ... }); // kind 'edit', 'new' or 'stale'; verts
//                                        // = vertices in the finished mesh
//   const ready = q.take(centre);        // the entries to upload this frame
//   q.cancel(rx, rz);                    // the region was freed: drop its entry
//   q.pending;                           // entries still waiting
//
// `take` hands out `edit` entries first, so an edit's remesh shows soon
// (section 3), then the rest nearest `centre` (Chebyshev distance, then
// Euclidean, as createStream and meshqueue.js sort). It adds entries in that
// order while the running vertex total stays within `budget`, and stops at the
// first that would go over. The first entry is always taken, even alone over
// the budget, so a large region still arrives. Leftovers wait for the next
// `take`. A region added again replaces its older entry.

export function createUploadQueue({ budget }) {
  const ready = new Map(); // "rx,rz" -> entry
  const key = (rx, rz) => `${rx},${rz}`;
  return {
    add(entry) {
      ready.set(key(entry.rx, entry.rz), entry);
    },
    take(centre) {
      const dist = (r) => Math.max(Math.abs(r.rx - centre.rx), Math.abs(r.rz - centre.rz));
      const near = (r) => Math.hypot(r.rx - centre.rx, r.rz - centre.rz);
      const edit = (r) => (r.kind === 'edit' ? 0 : 1);
      const order = [...ready.values()].sort((a, b) => edit(a) - edit(b) || dist(a) - dist(b) || near(a) - near(b));
      const out = [];
      let total = 0;
      for (const e of order) {
        if (out.length > 0 && total + e.verts > budget) break;
        out.push(e);
        total += e.verts;
        ready.delete(key(e.rx, e.rz));
      }
      return out;
    },
    cancel(rx, rz) {
      ready.delete(key(rx, rz));
    },
    get pending() { return ready.size; },
  };
}
