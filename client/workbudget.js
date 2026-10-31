// Per-frame work budgets (docs/design/play-critic-perf-render.md section 2, reconciliation note 4):
// plain numbers, apart from Phase A's wall-clock hitch threshold. A frame's work is
// `{ regions, chunks, verts }`, as the harness counts it: regions meshed plus remeshed,
// chunks the sim loaded and vertices meshed over all mesh kinds.

/** The stream's budget (client/stream.js): one region meshed or remeshed per frame. */
export const MAX_REGIONS_PER_FRAME = 1;
/** Chunk loads in one frame; a streamed sprint and flight across region borders peaks at 9. */
export const MAX_CHUNKS_PER_TICK = 16;
/** Vertices meshed in one frame; one region of seed 12345 near the start peaks at about 805 000. */
export const MAX_VERTS_PER_FRAME = 1_000_000;

const BUDGETS = { regions: MAX_REGIONS_PER_FRAME, chunks: MAX_CHUNKS_PER_TICK, verts: MAX_VERTS_PER_FRAME };

/** The fields of `work` over their budget (a value equal to its budget is within it), in a fixed order. */
export function overBudget(work) {
  return Object.keys(BUDGETS).filter((k) => work[k] > BUDGETS[k]);
}
