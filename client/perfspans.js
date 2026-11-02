// Timing spans for the render loop: tick, mesh and upload. Each call is bracketed with
// performance.mark/measure, then the marks and the measure are cleared at once so the
// timeline does not grow. take() returns { count, total, max } in ms per name since the
// last call and resets.

export const SPAN_NAMES = ['tick', 'mesh', 'upload'];

export function createSpans(perf = performance) {
  let sums = fresh();
  function fresh() {
    return Object.fromEntries(SPAN_NAMES.map((n) => [n, { count: 0, total: 0, max: 0 }]));
  }
  function span(name, fn) {
    const start = `${name}:start`, end = `${name}:end`;
    perf.mark(start);
    try {
      return fn();
    } finally {
      perf.mark(end);
      const ms = perf.measure(name, start, end).duration;
      perf.clearMarks(start);
      perf.clearMarks(end);
      perf.clearMeasures(name);
      const s = sums[name];
      s.count++;
      s.total += ms;
      if (ms > s.max) s.max = ms;
    }
  }
  function take() {
    const out = sums;
    sums = fresh();
    return out;
  }
  return { span, take };
}
