// The main thread's side of meshworker.js: posts mesh jobs and resolves each
// with its region's reply, whatever order replies come back in.
//
//   const mesher = createMeshClient({ spawn, init: { module, ctx, seed, blendRadius } });
//   mesher.submit(rx, rz, blocks)  // -> Promise of meshPadded's result; `blocks`
//                                  // is transferred, so the caller loses it
//   mesher.terminate()
//
// `spawn()` returns something Worker-shaped (postMessage, onmessage); in the
// page that is `new Worker(new URL('./meshworker.js', import.meta.url), { type: 'module' })`,
// in Node an in-process stand-in calling meshworker.js's handler directly.

export function createMeshClient({ spawn, init }) {
  const worker = spawn();
  const pending = new Map(); // id -> { resolve, reject }
  let nextId = 0;
  worker.onmessage = ({ data: { id, result, error } }) => {
    const job = pending.get(id);
    if (!job) return;
    pending.delete(id);
    if (error !== undefined) job.reject(new Error(error));
    else job.resolve(result);
  };
  if (init) worker.postMessage({ type: 'init', ...init });
  return {
    submit(rx, rz, blocks) {
      const id = nextId++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        worker.postMessage({ type: 'mesh', id, rx, rz, blocks }, [blocks.buffer]);
      });
    },
    terminate() {
      worker.terminate?.();
      for (const job of pending.values()) job.reject(new Error('mesh worker terminated'));
      pending.clear();
    },
  };
}
