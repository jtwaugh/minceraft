// Mesh job queue (docs/design/stream-without-stalls.md section 3), apart from
// WebGL and workers so it runs in Node. It decides which region to mesh next
// and how many meshes are being made at once; whoever runs the jobs (a worker
// pool, or the main thread) reports each result back.
//
//   const q = createMeshQueue({ limit: 2 });
//   q.submit({ rx, rz, kind });   // kind 'edit', 'new' or 'stale'
//   const job = q.next(centre);   // { rx, rz, kind }, or null if none is queued
//                                 // or `limit` jobs are already in flight
//   q.done(rx, rz);               // a result came back: true to use it, false
//                                 // if it was cancelled and must be discarded
//   q.cancel(rx, rz);             // the region was freed: drop its queued job
//                                 // and discard its in-flight result
//
// Jobs go out `edit` first, so an edit's remesh does not wait behind new
// regions, then `new` nearest `centre` (Chebyshev distance, then Euclidean, as
// createStream sorts), then `stale` in the order submitted. A region has at most
// one queued job: submitting it again keeps the higher-priority kind. A region
// in flight gets no second job in flight until its result comes back, so
// results, accepted in any order, are one per region. A cancelled in-flight job
// still holds its slot until `done`, since its worker is still busy.

const RANK = { edit: 0, new: 1, stale: 2 };

export function createMeshQueue({ limit = 1 } = {}) {
  const queued = new Map(); // "rx,rz" -> { rx, rz, kind }, in submit order
  const flying = new Map(); // "rx,rz" -> true while live, false once cancelled
  const key = (rx, rz) => `${rx},${rz}`;
  return {
    submit({ rx, rz, kind }) {
      if (!(kind in RANK)) throw new Error(`unknown mesh job kind: ${kind}`);
      const k = key(rx, rz);
      const old = queued.get(k);
      if (!old || RANK[kind] < RANK[old.kind]) queued.set(k, { rx, rz, kind });
    },
    next(centre) {
      if (flying.size >= limit) return null;
      const dist = (r) => Math.max(Math.abs(r.rx - centre.rx), Math.abs(r.rz - centre.rz));
      const near = (r) => Math.hypot(r.rx - centre.rx, r.rz - centre.rz);
      let best = null;
      for (const [k, job] of queued) {
        if (flying.has(k)) continue;
        if (!best || RANK[job.kind] < RANK[best.kind]) best = job;
        else if (job.kind === 'new' && best.kind === 'new' && (dist(job) - dist(best) || near(job) - near(best)) < 0) best = job;
      }
      if (!best) return null;
      queued.delete(key(best.rx, best.rz));
      flying.set(key(best.rx, best.rz), true);
      return best;
    },
    done(rx, rz) {
      const k = key(rx, rz);
      const live = flying.get(k) === true;
      flying.delete(k);
      return live;
    },
    cancel(rx, rz) {
      const k = key(rx, rz);
      queued.delete(k);
      if (flying.has(k)) flying.set(k, false);
    },
    get queued() { return queued.size; },
    get inFlight() { return flying.size; },
  };
}
