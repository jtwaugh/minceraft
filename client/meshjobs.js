// Streamed regions' mesh jobs (docs/design/stream-without-stalls.md section 3):
// the job queue and the mesh client joined, apart from WebGL so it runs in Node.
// The render thread gathers a region's padded blocks when its job is sent, then
// takes the finished meshes to upload.
//
//   const jobs = createMeshJobs({ queue, client, gather });  // gather(rx, rz) -> padded blocks
//   jobs.submit({ rx, rz, kind });   // kind 'edit', 'new' or 'stale' (meshqueue.js)
//   jobs.pump(centre);               // send the jobs the queue lets out
//   jobs.take();                     // [{ rx, rz, kind, result }] finished since the last take
//   jobs.cancel(rx, rz);             // the region was freed: it is never taken
//   jobs.busy                        // anything queued, in flight or waiting to be taken

export function createMeshJobs({ queue, client, gather }) {
  let finished = [];
  return {
    submit: (job) => queue.submit(job),
    pump(centre) {
      for (let job; (job = queue.next(centre));) {
        const { rx, rz } = job;
        client.submit(rx, rz, gather(rx, rz)).then(
          (result) => { if (queue.done(rx, rz)) finished.push({ ...job, result }); },
          (e) => { queue.done(rx, rz); console.error(`mesh job ${rx},${rz} failed`, e); },
        );
      }
    },
    take() {
      const out = finished;
      finished = [];
      return out;
    },
    cancel(rx, rz) {
      queue.cancel(rx, rz);
      finished = finished.filter((f) => f.rx !== rx || f.rz !== rz);
    },
    get busy() { return queue.queued + queue.inFlight + finished.length > 0; },
  };
}
