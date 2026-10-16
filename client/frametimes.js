// Rolling frame-time statistics for the debug overlay and headless logging.
// The target for "feels smooth at 60 fps" is p99 under 16.7 ms
// (docs/design/basic-gameplay.md, reconciler note 10).

export const FRAME_BUDGET_MS = 1000 / 60;

export class FrameTimes {
  constructor(capacity = 300) {
    this.buf = new Float64Array(capacity);
    this.n = 0; // frames recorded so far
  }

  push(ms) {
    this.buf[this.n % this.buf.length] = ms;
    this.n++;
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

  summary() {
    return { frames: this.count, p50: this.percentile(50), p99: this.percentile(99) };
  }

  /** One line for the overlay and the console. */
  format() {
    const { frames, p50, p99 } = this.summary();
    return `frame p50 ${p50.toFixed(1)} ms  p99 ${p99.toFixed(1)} ms  (${frames} frames)`;
  }
}
