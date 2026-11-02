// Verdict for a real-timing perf pass: pure functions over per-replay `frameStats()` results
// (docs/design/play-critic-perf-render.md section 3, reconciler notes 4 and 5). CPU spans only.
import { HITCH_MS } from './frametimes.js';

/** How many times the pass replays the same path. */
export const PERF_REPLAYS = 5;
/** A pass is over when the median of the replays' slowest frame is above this (one source: HITCH_MS). */
export const PERF_MAX_MS = HITCH_MS;

/** Median of numbers; the mean of the middle two for an even count; 0 when empty. */
export function median(values) {
  if (!values.length) return 0;
  const s = [...values].sort((a, b) => a - b);
  const mid = s.length >> 1;
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

/**
 * `replays` is an array of `{ frames, p50, p99, max, hitches, records }`. Returns
 * `{ replays, medianMax, medianHitches, verdict, text }`; with no replays the verdict is null and text empty.
 */
export function perfVerdict(replays) {
  if (!replays.length) return { replays: 0, medianMax: 0, medianHitches: 0, verdict: null, text: '' };
  const medianMax = median(replays.map((r) => r.max));
  const medianHitches = median(replays.map((r) => r.hitches));
  const verdict = medianMax > PERF_MAX_MS ? 'over' : 'ok';
  const slowest = replays.reduce((a, b) => (b.max > a.max ? b : a));
  const lines = [
    `perf ${verdict}: median max ${medianMax.toFixed(1)} ms (limit ${PERF_MAX_MS.toFixed(1)} ms), ` +
      `median hitches ${medianHitches}, ${replays.length} replays`,
    `slowest replay: max ${slowest.max.toFixed(1)} ms, ${slowest.hitches} hitches`,
    ...(slowest.records ?? []).map((r) =>
      `  hitch ${r.ms.toFixed(1)} ms: ${r.meshed} meshed  ${r.remeshed} remeshed  ` +
      `${r.verts} verts  ${r.ticks} ticks  ${r.chunks} chunks`),
  ];
  return { replays: replays.length, medianMax, medianHitches, verdict, text: lines.join('\n') };
}
