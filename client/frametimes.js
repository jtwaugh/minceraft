// Rolling frame-time statistics for the debug overlay and headless logging.
// The target for "feels smooth at 60 fps" is p99 under 16.7 ms
// (docs/design/basic-gameplay.md, reconciler note 10).

export const FRAME_BUDGET_MS = 1000 / 60;

// A wall-clock hitch is a frame longer than two frames at 60 Hz (about 33.3 ms).
export const HITCH_MS = 2 * FRAME_BUDGET_MS;

// How many hitch records FrameTimes keeps; the oldest goes first.
export const HITCH_RECORDS = 20;

export class FrameTimes {
  constructor(capacity = 300) {
    this.buf = new Float64Array(capacity);
    this.n = 0; // frames recorded so far
    this.records = []; // the last HITCH_RECORDS hitches, oldest first
  }

  /** `work` ({ meshed, remeshed, verts, ticks, chunks }) is kept when the frame is a hitch. */
  push(ms, work) {
    this.buf[this.n % this.buf.length] = ms;
    this.n++;
    if (ms > HITCH_MS) this.recordHitch(ms, work);
  }

  recordHitch(ms, { meshed = 0, remeshed = 0, verts = 0, ticks = 0, chunks = 0 } = {}) {
    this.records.push({ ms, meshed, remeshed, verts, ticks, chunks });
    if (this.records.length > HITCH_RECORDS) this.records.shift();
  }

  /** Hitch records, oldest first. */
  hitchRecords() {
    return this.records.slice();
  }

  /** The latest hitch record as one overlay line; empty when there is none. */
  formatLastHitch() {
    const r = this.records.at(-1);
    return r ? `last hitch ${r.ms.toFixed(1)} ms: ${r.meshed} meshed  ${r.remeshed} remeshed  ` +
      `${r.verts} verts  ${r.ticks} ticks  ${r.chunks} chunks` : '';
  }

  get count() {
    return Math.min(this.n, this.buf.length);
  }

  /** Nearest-rank percentile (p in 0..100) over the window; 0 when empty. */
  percentile(p) {
    const n = this.count;
    if (!n) return 0;
    const sorted = Array.from(this.buf.subarray(0, n)).sort((a, b) => a - b);
    return sorted[Math.min(n - 1, Math.max(0, Math.ceil((p / 100) * n) - 1))];
  }

  /** Slowest frame in the window; 0 when empty. */
  get max() {
    let m = 0;
    for (const ms of this.buf.subarray(0, this.count)) m = Math.max(m, ms);
    return m;
  }

  /** Frames in the window longer than HITCH_MS. */
  get hitches() {
    let h = 0;
    for (const ms of this.buf.subarray(0, this.count)) if (ms > HITCH_MS) h++;
    return h;
  }

  summary() {
    return {
      frames: this.count,
      p50: this.percentile(50),
      p99: this.percentile(99),
      max: this.max,
      hitches: this.hitches,
    };
  }

  /** One line for the overlay and the console. */
  format() {
    const { frames, p50, p99, max, hitches } = this.summary();
    return `frame p50 ${p50.toFixed(1)} ms  p99 ${p99.toFixed(1)} ms  max ${max.toFixed(1)} ms  ` +
      `${hitches} hitches  (${frames} frames)`;
  }
}
