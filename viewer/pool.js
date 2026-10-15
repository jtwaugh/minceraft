// A pool of viewer workers (worker.js) with one shared priority queue, so map
// tiles, flyover planning and flyover chunks all run off the main thread and
// the most urgent job always goes next.
//
//   const pool = new WorkerPool(init);            init: posted to every worker first
//   const job = pool.run('tile', args, priority); lower priority runs sooner
//   job.promise -> result;  job.cancel()          drops it if it hasn't started

export class WorkerPool {
  constructor(init, size = Math.max(2, Math.min(4, (navigator.hardwareConcurrency || 4) - 1))) {
    this.queue = [];
    this.running = new Map(); // id -> job
    this.idle = [];
    this.nextId = 1;
    this.size = size;
    for (let i = 0; i < size; i++) {
      const w = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
      w.onmessage = (e) => this.done(w, e.data);
      w.onerror = (e) => console.error('worker:', e.message);
      w.postMessage({ type: 'init', args: init });
      this.idle.push(w);
    }
  }

  run(type, args, priority = 0) {
    const job = { id: this.nextId++, type, args, priority, cancelled: false };
    job.promise = new Promise((resolve, reject) => { job.resolve = resolve; job.reject = reject; });
    job.cancel = () => {
      if (job.cancelled) return;
      job.cancelled = true;
      const i = this.queue.indexOf(job);
      if (i >= 0) this.queue.splice(i, 1);
      job.reject(Object.assign(new Error('cancelled'), { cancelled: true }));
    };
    job.promise.catch(() => {}); // cancellation is not an error for callers that don't wait
    // keep the queue sorted by priority, first come first served within one
    let i = this.queue.length;
    while (i > 0 && this.queue[i - 1].priority > priority) i--;
    this.queue.splice(i, 0, job);
    this.pump();
    return job;
  }

  /** Change a queued job's priority. */
  reprioritise(job, priority) {
    const i = this.queue.indexOf(job);
    if (i < 0 || job.priority === priority) return;
    this.queue.splice(i, 1);
    job.priority = priority;
    let j = this.queue.length;
    while (j > 0 && this.queue[j - 1].priority > priority) j--;
    this.queue.splice(j, 0, job);
  }

  get busy() { return this.running.size + this.queue.length; }

  pump() {
    while (this.idle.length && this.queue.length) {
      const job = this.queue.shift(), w = this.idle.pop();
      this.running.set(job.id, job);
      w.postMessage({ id: job.id, type: job.type, args: job.args });
    }
  }

  done(w, { id, result, error, ms }) {
    this.idle.push(w);
    const job = this.running.get(id);
    this.running.delete(id);
    if (job && !job.cancelled) {
      job.ms = ms;
      if (error) job.reject(new Error(error)); else job.resolve(result);
    }
    this.pump();
  }
}
