---
title: Stream the world without stalls
status: accepted
scope: game
source: "#1566"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
  D: todo
---

## Current state

Checked against the code on `master`:

- **Generation in the sim tick (section 1).** `Streamer::update` (`crates/game/src/stream.rs`) returns at once while the centre region is unchanged; otherwise it calls `RegionCache::update` (`crates/layout/src/cache.rs`), which generates every region within `load` that is not cached, and then `world.push(r.region.clone())` for each (a second copy of each 2 MB region). `data/streaming.toml` has `load = 1`, `unload = 2`, `decorate = true`. A corridor (`set_corridor`) can restrict loading further.
- **Copying blocks to JS (section 1).** `Walk.region_blocks` (`crates/game-web/src/lib.rs`) calls `Walker::region_blocks` (`crates/game/src/walk.rs`), which clones the region's `blocks` (128 × 128 × `HEIGHT` bytes, 2 MB), applies every edit and returns the copy to JS. `padRegion` (`client/regionpad.js`) calls it for the region and up to 8 neighbours and uses one column of each neighbour (the whole array for the region itself), so about 18 MB cross the wasm boundary per meshed region. Its output (130 × 130 columns) goes back into wasm in `meshBlocksWasm`.
- **Meshing and upload (sections 1, 3, 4).** `meshRegion` (`client/scene.js` ~line 399) does padding, `columnTints`, `meshBlocksWasm` and `upload` in one function called from `render`. `createStream({ budget = 1 })` (`client/stream.js`) limits meshing to one new region per frame, nearest first. `meshRegion` marks loaded neighbours (4-neighbours only) `stale`; `render` remeshes one per frame when nothing new was meshed. Edited regions come through `stream.edited` and are remeshed immediately.
- **Off-thread precedent.** `viewer/pool.js` (`WorkerPool`, shared priority queue, cancel) and `viewer/worker.js` run tiles and flyover jobs in workers. The client has no worker. `viewer/meshwasm.js` and `crates/web/src/mesh.rs` are the mesher.
- **Fog (section 4).** `scene.js` reads `Walk.load_radius()` (line ~355) and uses `fogFar` with the radial curve in `viewer/fog.js`; the doc's "fog end follows `load_radius`" holds.
- **Harness.** `client/test/dsl.mjs` has `start ... stream`, running `controller.js` over the real wasm with `regions.loaded|freed|pending|x|z`; it meshes nothing. There is no worker, no arrival event and no per-frame vertex count.
- **Determinism tests.** `crates/game/src/stream.rs` has `streamed_walk_matches_one_large_region`; `crates/terrain` and `crates/layout` have the overlapping-region tests. Nothing tests when a region joins the world, because it always joins in the tick that asks.
- **Not done yet:** every item in sections 2 to 4. Section 1 describes the present code and is accurate.

## Reconciliation notes

1. **Overlap with `play-critic-perf-render.md`.** That doc (accepted, all phases `todo`) measures the stall; this one removes it. Its Phase A (hitch record and overlay) is the `[manual check]` instrument in section 5; its Phase B moves the CPU half of `meshRegion` out of `scene.js` into a GL-free module and adds the work-per-frame DSL paths. Phase B here needs that move. The two docs do not conflict, but the planner plans one phase at a time per doc, so nothing orders them. This doc's Phase B must not be planned before the other doc's Phase B is done; the planner should record `**Depends on:**` the ticket that closes it. Phase A and C here do not depend on it. This doc's Phase C ("no frame uploads more than its budget") and that doc's `verts.max_per_frame` path are the same assertion; build the path once, in whichever lands first.
2. **Section 1: the generation count.** "3 regions at a straight border, 5 at a corner" is right for `load = 1` (3 new of 9; 5 at a diagonal). The sim also clones each region again into the world (`world.push(r.region.clone())`), a second 2 MB per region that the doc does not list. Removing it (moving the region into the world and keeping only one copy) belongs with Phase D.
3. **Section 3: `region_border`.** `Walk.region_blocks` also applies edits by scanning every edit in the world per call; a border export should apply them too, since a neighbour's edited border column changes the padded mesh. Suggest `region_border(rx, rz, side)` returns the 128 × `HEIGHT` column with edits applied, and `padRegion` takes it through the same `get` seam it has now, so `client/test/meshorigin.test.mjs` and a padded-pair test keep working. A unit test: padded output from border columns equals padded output from full neighbours, for a region with an edit on its border.
4. **Section 3: worker blocks copy vs. border export.** The doc offers two ways to give a worker the blocks. A worker with its own copy of loaded blocks must also receive edits (picks and placements) or its meshes show stale terrain; a border export from the main wasm needs no sync but still crosses the thread boundary per mesh job. Open question 5 below.
5. **Section 2: worker generation and CLAUDE.md invariants.** `crates/terrain` stays `no_std` and dependency-free; a worker can load the same wasm the viewer's workers load (`crates/web`) or a new export in `crates/game-web`. Generation is `layout` code (`RegionCache`, with `std`), so the worker needs a `layout`-based export; keep it in `crates/game-web` rather than `terrain`. This adds a second wasm instance in the client; report gzipped wasm size before and after, per CLAUDE.md. A region generated in a worker must be byte-identical to one generated in the sim (the existing invariant: a block depends only on seed and coordinate); add a test that compares them.
6. **Section 2: determinism rule.** The sim is the only authority on what is loaded (`Walk.drain_stream`); the client mirrors it. If generation moves to a worker, the sim can only treat a region as loaded once its blocks are in. Options are in open question 2. Whichever is chosen, `streamed_walk_matches_one_large_region` must keep passing with no pending regions, and the new harness case must show the "touchable regions are loaded" rule with arrival delayed.
7. **Section 2: what a player can touch.** Collision reads `RegionSet`; a region missing from it is not solid. With the fallback "hold the player at the border", the held player must also be held in the harness; with "generate synchronously", the stall returns on a fast fly (sprint speed is in `data/`; check `docs/design/sprinting.md` for the number before claiming "several seconds").
8. **Section 2: prefetch radius.** `data/streaming.toml` is read by `Streamer`, not by `build.rs`, and is game data of this project, so adding `prefetch` there is allowed (unlike the perf-render doc's thresholds). It must be at least `load` and at most `unload`, else a prefetched region is dropped on arrival. `Streamer`'s config parsing and its `shipped_radii_parse` test change with it.
9. **Section 3 and 4: GPU budget.** Upload budget is in vertices per frame. The harness cannot count vertices until the mesh module exists (note 1), so Phase C's test needs that module, but its logic (a queue that takes ready meshes nearest first up to a budget) can be a pure function in `client/stream.js` tested without it. Phase C is ordered after B in the doc because uploads only queue once meshing is async; with synchronous meshing it can still cap vertices per frame, so it stays independent of B.
10. **`[manual check]` items.** Section 5's first bullet ("no frame over the hitch threshold on a typical laptop") is a visual and hardware judgment; tagged below. Measurable version: the perf-render hitch record shows zero hitches (threshold `2 * FRAME_BUDGET_MS`, about 33 ms, from `client/frametimes.js`) over 5 replays of a fixed sprint route across 4 borders in headless Chrome, once that doc's Phase E exists.
11. **Open question 3 (sim in a worker).** Not in the phases. It changes `client/controller.js`, input, the HUD and `client/debug.js` and the DSL, which runs `controller.js` in Node over the real wasm; moving the sim out would break the harness's design (CLAUDE.md, Testing the client). The smallest alternative is Phases B and D here. Listed as a question, not planned.
12. **SPEC.** The doc states no Beta behaviour; SPEC.md is untouched. No change to world output: regions are the same blocks, delivered later. Phase A changes no block, only how many are copied.
13. **`[external]`.** None.

## Idea

# Mince Raft: Stream the world without stalls

Status: idea
Scope: game

Walking or flying across a region border in a streamed world (`Walk.streamed`) freezes the game for a moment. Everything that border costs runs on the main thread inside one or two frames: generating the new regions, copying their blocks out to JS, meshing them and uploading the meshes. This idea moves generation and meshing off the main thread, spreads what has to stay there over several frames, and keeps the sim deterministic. `docs/design/play-critic-perf-render.md` measures the stall; this idea removes it. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. Where the stall comes from

- **Generation is synchronous, all in one tick.** `game::stream::Streamer::update` (`crates/game/src/stream.rs`) calls `RegionCache::update` (`crates/layout/src/cache.rs`) when the player's region changes. That generates every newly in-range region before it returns. With `load = 1` (`data/streaming.toml`), crossing a straight border generates 3 decorated 128 × 128 regions in one sim tick, and crossing at a corner generates 5.
- **Each meshed region copies about 20 MB.** `meshRegion` (`client/scene.js`) calls `padRegion` (`client/regionpad.js`), which calls `walk.region_blocks` for the region and each of its 8 neighbours. Each call clones the region's 2 MB block array in Rust, applies edits and copies it into JS. Only one column of each neighbour is used. Then the 130 × 130 padded array goes back into wasm for `meshBlocksWasm`.
- **Meshing and upload share a frame.** `createStream({ budget: 1 })` meshes at most one region per frame, but that one region is meshed and uploaded inside `render`. Each newly meshed region also marks its loaded neighbours `stale`, and they are remeshed one per spare frame afterwards, each paying the same cost again.
- The viewer already does this work off the main thread (`viewer/pool.js`, `viewer/worker.js`); the client does not.

## 2. Generate ahead, off the main thread

- Want: a region's blocks are generated in a worker, not in the sim tick. The sim asks for a region early, gets it back later and adds it to the world when it arrives.
- Want: regions are requested before the player needs them. When the player enters a region, its neighbours are about 128 blocks from the far edge, which is several seconds of sprinting or flying. So a worker has time if requests go out when the centre changes (or earlier, on a prefetch radius larger than `load`).
- Want: a fallback if the player gets to a region that has not arrived (a fast fly, a teleport, a slow device). Options: generate it synchronously then (the stall comes back only in that case), or hold the player at the border until it arrives.
- **Determinism.** A block still depends only on the seed and its coordinate. What changes is *when* a region joins the world. The sim must not behave differently depending on worker timing in a way the harness can't reproduce. Options: the sim treats arrival as an input event it records (so a transcript replays exactly), or the harness runs the worker synchronously and the rule "a region a player can touch is always loaded" is tested on its own.

> **Reconciler:** see notes 2, 5, 6, 7 and 8. A worker needs a `layout`-based wasm export in `crates/game-web`, not `terrain`, and the sim's second per-region clone goes with it.

## 3. Mesh off the main thread

- Want: the CPU half of `meshRegion` (padding, tints, `meshBlocksWasm`) runs in a worker and returns vertex arrays as transferables. Only `upload` runs in `render`. This builds on play-critic-perf-render Phase B, which moves that half out of `scene.js` already.
- Want: padding reads only the one border column of each neighbour, not the whole neighbour (e.g. a `region_border(rx, rz, side)` export, or the worker keeps its own copy of loaded blocks).
- Want: a new region's neighbour remeshes (`stale`) go through the same queue, so they don't land in the main thread either.
- Edits (picks, placements) stay fast. A remesh after an edit should show on the next frame or soon after, so it either takes priority in the queue or stays synchronous.

> **Reconciler:** see notes 1, 3 and 4. The 4-neighbour `stale` set is what the code does today; the 8-neighbour reads in `padRegion` are only for the corner columns.

## 4. Spread uploads across frames

- Want: if several meshes are ready in one frame, upload them up to a per-frame vertex budget and leave the rest for the next frame. The nearest region goes first, as `createStream` sorts today.
- Want: a region whose mesh hasn't arrived is simply missing, hidden by fog, never a hole next to the player. Fog end follows `load_radius` today, so the regions that are still loading should be at the edge of the fog.

> **Reconciler:** see note 9. With `load = 1` the nearest ring of regions is within 128 blocks of the player at all times, so "hidden by fog" holds only if fog closes before that; check `FOG_END * fogFar` in `viewer/fog.js` against `load_radius * REGION_SIZE` before relying on it (the second bullet is a `[manual check]`).

## 5. How we know it worked

- With play-critic-perf-render Phase A in place, crossing a border at sprint speed, and flying across several, shows no frame over the hitch threshold on a typical laptop. `[manual check]`
- A harness case crosses several borders and asserts that a region the player is standing in or touching is always loaded, and that no frame meshes or uploads more than its budget.
- The determinism tests for overlapping regions still pass, and a streamed transcript replays the same.

> **Reconciler:** the first bullet is a `[manual check]`; note 10 gives a measurable version.

---

## Phases (suggested)

- A: padding reads border columns only, with no full-neighbour copies (1, 3). Small and on its own; it removes most of the copying even before any worker exists.
- B: meshing in a worker, with upload in `render` and the stale queue moved with it (3). Needs play-critic-perf-render Phase B.
- C: upload budget across frames (4).
- D: generation off the sim tick, requested ahead, with the fallback and the determinism rule (2).

> **Reconciler:** phases kept as written. Done when: A, a `region_border` export exists, `padRegion` uses it, and a test shows the padded array equals the one built from full neighbours, including an edit on a border; `region_blocks` is called once per meshed region, not nine times. B, the CPU meshing runs in a worker and returns transferables, `render` only uploads, the `stale` queue goes through the worker, an edit's remesh is not delayed behind new regions, and `meshorigin.test.mjs` passes (depends on play-critic-perf-render Phase B, note 1). C, a pure function takes ready meshes nearest-first up to a vertex budget, tested with a fixed list, and a script on a `stream` case asserts no frame exceeds the budget. D, generation runs in a worker with a result byte-identical to the sim's (note 5), regions are requested on centre change (or `prefetch`), the chosen fallback and determinism rule are implemented, and a script crosses several borders asserting the player's region and the regions it touches are always loaded. D waits on open questions 1, 2 and 4, so it stays last. If the chosen design for D replaces the stale queue or the budget, the docs updater should note it in B and C.

## Open questions

1. Generation fallback (section 2): generate synchronously when needed, or hold the player at the border?
2. Determinism (section 2): record region arrival as an input event, or run synchronously in the harness and test the "touchable regions are loaded" rule apart?
3. Should the sim itself (the `Walk` wasm instance) move into a worker, with the main thread only rendering? That would cover generation and meshing at once, but it changes how input, the HUD and the debug console reach the sim.
4. Prefetch radius: keep `load = 1` and request on centre change, or add a prefetch radius in `data/streaming.toml`?

## Open questions (reconciler)

1. **Q1 fallback.** Options: A, generate synchronously when the player reaches an unarrived region (suggested: the stall returns only in a rare case, and the harness needs no hold logic); B, hold the player at the border (needs a collision rule and a HUD cue, and is the same code path in the harness).
2. **Q2 determinism.** Options: A, the sim records arrival as an input event, so a transcript replays exactly (suggested if generation really runs in a worker); B, the harness runs generation synchronously and the "touchable regions are loaded" rule is tested separately (cheaper, but the real worker path is untested by scripts).
3. **Q3 sim in a worker.** Options: A, no (suggested, note 11: it breaks the harness's design); B, yes, as its own design doc.
4. **Q4 prefetch.** Options: A, request on centre change with `load = 1` (suggested for the first cut, no data change); B, add `prefetch` to `data/streaming.toml` (note 8).
5. **Q5 worker blocks.** Options: A, `region_border` from the main wasm (suggested: no edit sync); B, the worker keeps its own copy of loaded blocks and receives edits (less crossing, more state to keep in sync).
