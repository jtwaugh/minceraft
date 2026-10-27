// Flyover playback: loads the chunks the plan's visibility schedule says the
// camera will see, just before it sees them, through the page's worker pool;
// runs the clock; records the canvas to a video file.
//
// The page supplies `env`:
//   pool                                  the WorkerPool (../pool.js)
//   upload({ verts, idx }) -> handle      put a mesh part on the GPU
//   free(handle)
//   render(frame)                         draw one frame (see FlyoverPlayer.frame)
//   canvas                                the canvas to record (2D composite with captions)

export { CHUNK } from './plan.js';

export const OPENING = 10;   // seconds of flight rendered before playback may start
const LOOKAHEAD = 8;         // keep chunks loading this far ahead of the camera
const FLYOVER_BLEND = 2;     // biome colour blend radius for flyover chunks (the legend's is 3)
const BEHIND = 2;            // free chunks this long after they were last in view
// Job priorities in the shared pool (lower runs sooner). Map tiles use 0.
export const PRI = { plan: 1, opening: 2, stream: 3 };

const key = (cx, cz) => `${cx},${cz}`;
const PARTS = ['opaque', 'cutout', 'water', 'ice'];
// Chunks are meshed with fast leaves until the schedule's `full` time comes
// within `ahead`; then a full-detail mesh replaces the fast one.
const wantFast = (e, t) => e.full > t;

export class ChunkStream {
  constructor(seed, env, schedule) {
    this.seed = seed; this.env = env; this.schedule = schedule;
    this.loaded = new Map();   // key -> { cx, cz, fast, parts, meanY, minY }
    this.jobs = new Map();     // key -> pool job (queued or running)
    this.stats = { generated: 0, ms: 0 };
  }

  /**
   * Load what's in view from time t to t + ahead, soonest first; cancel jobs
   * and free chunks outside [t - BEHIND, t + ahead + 4].
   */
  update(t, ahead = LOOKAHEAD, priority = PRI.stream) {
    const want = new Set();
    for (const e of this.schedule) {
      if (e.first > t + ahead) break; // sorted by first
      if (e.last < t - 0.5) continue;
      const k = key(e.cx, e.cz);
      want.add(k);
      const fast = wantFast(e, t + ahead);
      const pri = priority + Math.max(0, (fast ? e.first : Math.max(e.first, e.full)) - t) / 100;
      const have = this.loaded.get(k);
      if (have && (!have.fast || fast)) continue;
      const job = this.jobs.get(k);
      if (job && job.args.fast === fast) { this.env.pool.reprioritise(job, pri); continue; }
      job?.cancel();
      this.request(e, k, pri, fast);
    }
    for (const [k, job] of this.jobs) if (!want.has(k)) { job.cancel(); this.jobs.delete(k); }
    const keepUntil = t + ahead + 4;
    for (const e of this.schedule) {
      const k = key(e.cx, e.cz), c = this.loaded.get(k);
      if (c && (e.last < t - BEHIND || e.first > keepUntil)) this.drop(k, c);
    }
  }

  request(e, k, priority, fast) {
    const job = this.env.pool.run('chunk', { seed: this.seed, cx: e.cx, cz: e.cz, fast, blend: FLYOVER_BLEND }, priority);
    this.jobs.set(k, job);
    job.promise.then((d) => {
      if (this.jobs.get(k) !== job || this.disposed) return;
      this.jobs.delete(k);
      const parts = {};
      for (const name of PARTS) parts[name] = this.env.upload(d[name]);
      const old = this.loaded.get(k);
      if (old) this.drop(k, old); // a fast mesh replaced by full detail
      this.loaded.set(k, { cx: d.cx, cz: d.cz, fast: d.fast, parts, meanY: d.meanY, minY: d.minY });
      this.stats.generated++; this.stats.ms += job.ms || 0;
    }, () => {});
  }

  drop(k, c) {
    for (const part of Object.values(c.parts)) this.env.free(part);
    this.loaded.delete(k);
  }

  /** [loaded, total] for the chunks in view between t0 and t1, at the detail they need by t1. */
  progress(t0, t1) {
    let n = 0, done = 0;
    for (const e of this.schedule) {
      if (e.first > t1) break;
      if (e.last < t0) continue;
      const c = this.loaded.get(key(e.cx, e.cz));
      n++; if (c && (!c.fast || wantFast(e, t1))) done++;
    }
    return [done, n];
  }

  /** True when everything in view between t0 and t1 is on the GPU. */
  ready(t0, t1 = t0) { const [d, n] = this.progress(t0, t1); return d === n; }

  dispose() {
    this.disposed = true;
    for (const job of this.jobs.values()) job.cancel();
    this.jobs.clear();
    for (const [k, c] of this.loaded) this.drop(k, c);
  }
}

/**
 * Plays a plan (flyover/plan.js, hydrated). On creation it starts rendering
 * the first OPENING seconds in the background (`prepared` tells when that's
 * done). States: 'loading' (waiting for chunks before the clock can run),
 * 'playing', 'paused', 'done'.
 */
export class FlyoverPlayer {
  constructor(plan, seed, env, onState = () => {}) {
    this.plan = plan; this.env = env; this.onState = onState;
    this.stream = new ChunkStream(seed, env, plan.schedule);
    this.t = 0; this.state = 'paused'; this.wantPlay = false; this.last = null;
    this.gate = OPENING;   // before the clock first runs (or after a seek), wait for this much
    this.recorder = null;
    this.frameNo = 0;
    this.stream.update(0, OPENING, PRI.opening);
  }

  get duration() { return this.plan.path.duration; }
  /** [loaded, total] chunks for the opening OPENING seconds. */
  get opening() { return this.stream.progress(0, OPENING); }
  get prepared() { const [d, n] = this.opening; return d === n; }

  play() { if (this.t >= this.duration) this.seek(0); this.wantPlay = true; this.last = null; }
  pause() { this.wantPlay = false; this.setState('paused'); if (this.recorder?.state === 'recording') this.recorder.pause(); }
  seek(t) {
    this.t = Math.max(0, Math.min(this.duration, t)); this.last = null;
    this.gate = 3;
    this.stream.update(this.t);
  }

  setState(s) { if (s !== this.state) { this.state = s; this.onState(s); } }

  /** Advance the clock by real time `now` (ms) if allowed, then render. */
  frame(now) {
    // Never show holes: the clock runs only while everything in view now (and,
    // after a start or seek, for the next `gate` seconds) is loaded. While it
    // waits the recording is paused too.
    const ready = this.stream.ready(this.t, this.t + this.gate);
    if (this.wantPlay && ready) {
      this.gate = 0;
      // Real time, so recordings (stamped by wall clock) play at the right
      // speed; only a long hitch is capped, as a jump rather than a crawl.
      if (this.last !== null) this.t += Math.min(0.25, (now - this.last) / 1000);
      this.last = now;
      this.setState('playing');
      if (this.recorder?.state === 'paused') this.recorder.resume();
      if (this.t >= this.duration) { this.t = this.duration; this.wantPlay = false; this.setState('done'); this.stopRecording(); }
    } else {
      this.last = null;
      if (this.wantPlay) { this.setState('loading'); if (this.recorder?.state === 'recording') this.recorder.pause(); }
    }
    // while waiting at a start or seek, keep the whole gated stretch wanted
    if (this.frameNo++ % 10 === 0 || !ready) {
      this.stream.update(this.t, Math.max(LOOKAHEAD, this.gate), this.gate ? PRI.opening : PRI.stream);
    }
    const f = this.plan.path.at(this.t);
    this.env.render({ ...f, t: this.t, duration: this.duration, chunks: this.stream.loaded.values(), state: this.state });
  }

  /**
   * Play from the start while recording the env canvas. Resolves with
   * { blob, type } when the flight ends (or stopRecording() is called).
   */
  record(fps = 30, bitrate = 10e6) {
    const type = ['video/mp4;codecs=avc1.640028', 'video/mp4', 'video/webm;codecs=vp9', 'video/webm']
      .find((t) => window.MediaRecorder?.isTypeSupported(t));
    if (!type) return Promise.reject(new Error('MediaRecorder is not available in this browser'));
    const stream = this.env.canvas.captureStream(fps);
    const rec = new MediaRecorder(stream, { mimeType: type, videoBitsPerSecond: bitrate });
    const parts = [];
    rec.ondataavailable = (e) => { if (e.data.size) parts.push(e.data); };
    const done = new Promise((resolve) => {
      rec.onstop = () => { stream.getTracks().forEach((tr) => tr.stop()); resolve({ blob: new Blob(parts, { type }), type }); };
    });
    this.seek(0);
    rec.start(1000);
    rec.pause();                // resumes on the first frame the clock runs
    this.recorder = rec;
    this.play();
    return done;
  }

  stopRecording() {
    if (this.recorder && this.recorder.state !== 'inactive') this.recorder.stop();
    this.recorder = null;
  }

  dispose() { this.stopRecording(); this.stream.dispose(); }
}
