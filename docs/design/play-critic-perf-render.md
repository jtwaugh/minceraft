---
title: Play critic sees lag and render faults
status: accepted
scope: game
source: "#1526"
superseded_by: ""
phases:
  A: done
  B: done
  C: done
  D: done
  E: todo
  F: todo
---

## Current state

Checked against the code on `master`:

- **Phase A done (plan #1562).** PRs #1585 (`FrameTimes` max, `hitches`, `HITCH_MS = 2 * FRAME_BUDGET_MS`, overlay and `frameStats()`) and #1651 (per-hitch work records, `formatLastHitch`, `records` in `frameStats()`) built section 1; the first bullet below describes the code before them.
- **Phase B done (plan #1658).** PRs #1667 (CPU meshing moved to `client/regionmesh.js`, `meshRegionCpu`), #1670 (stale-neighbour remesh planned in the stream), #1676 (harness meshes regions; `regions.max_per_frame`, `chunks.max_per_tick`, `verts.max_per_frame` in `client/test/dsl.mjs`) and #1678 (`client/workbudget.js`, the `overbudget` event, `events.overbudget`) built section 2; the Section 2 bullets below describe the code before them.
- **Phase C done (plan #1681).** PRs #1705 (UV snap built from shared constants in `client/texsnap.js`, tested at every view angle), #1735 (`client/meshcheck.js` crack check on opaque full-block faces) and #1706 (`client/test/regionborder.test.mjs`, padded neighbour regions mesh like the two together) built section 4; the Section 4 bullets below describe the code before them.
- **Phase D done (plan #1919, after #1743).** PRs #1802 (`perf` and `render` in `KINDS`), #1809 (`render.faults` on streamed worlds), #1837 (prompt kinds and streamed-world goals), #1926 (`diagnostic` class in `client/tools/observe.mjs`, gated to streamed worlds in `play.mjs`) and #1936 (prompt names the diagnostic paths) built section 6; the Section 6 bullet below describes the code before them.
- **Frame stats (section 1).** `client/frametimes.js` has `FrameTimes` (300-frame ring buffer, nearest-rank `percentile`, `summary()` = `{ frames, p50, p99 }`, `format()`) and `FRAME_BUDGET_MS` (1000 / 60). `client/scene.js` creates `frames = new FrameTimes(300)`, pushes `now - lastFrame` each frame, exposes `window.frameStats = () => frames.summary()` and builds the overlay text from `format()` plus an "over/within the 16.7 ms p99 budget" line. There is no max, no hitch count and no per-frame work record. `client/test/frametimes.test.mjs` tests it.
- **Region meshing (section 2).** `meshRegion` in `client/scene.js` does the CPU half (`padRegion` from `client/regionpad.js`, `columnTints`, `meshBlocksWasm` from `viewer/meshwasm.js`, over `crates/web/src/mesh.rs`) and the GPU half (`upload`, `regionParts`, `meshes`) in one function. The `stale` map and the one-per-spare-frame remesh of neighbours also live in `scene.js`'s `render`. `padRegion` and `meshBlocksWasm` are plain JS over wasm, and `client/test/meshorigin.test.mjs` already calls `meshBlocksWasm` from Node, so the CPU half can run headless.
- **Streaming (section 2).** `client/stream.js` `createStream({ budget = 1 })` turns `walk.drain_stream()` events into regions to mesh (at most `budget` per frame, nearest first) and regions to free. `controller.js` `streamRegions` returns `{ mesh, free, remesh, loaded, freed, pending, centre }` from it each frame. The DSL (`client/test/dsl.mjs`) has `start ... stream`, which runs this and exposes `regions.loaded|freed|pending|x|z`, but meshes nothing.
- **DSL paths (section 2).** No `regions.max_per_frame`, `chunks.max_per_tick`, `verts.max_per_frame` or `hitch` event.
- **Critic (section 6).** `client/tools/finding.mjs` has `KINDS = ['control-flow', 'layout']`; `.github/prompts/critic.md` line 69 lists the same two kinds. `.github/workflows/claude-critic.yml` exists (the play-critic doc's Phase E, done). Verify pass: `client/tools/verify.mjs`.
- **Shader UV snap (section 4).** The client's block fragment shader in `client/scene.js` (~line 139 to 143) snaps UVs to texel centres with a `clamp((fract(px) - 0.5) / fw, -0.5, 0.5)` term and samples with `textureGrad`. `docs/design/viewer-rendering.md` Phase A added the snap (PR #520 for the client) and tracks the fog and filtering work; nothing there tests it.
- **Region-border meshing (section 4).** `padRegion` (`client/regionpad.js`) pads a region with one column from each neighbour, reading an unloaded neighbour as the region's own nearest edge column. The `stale` remesh in `scene.js` exists because of that fallback. `client/test/meshorigin.test.mjs` checks the mesh origin only; there is no test that a padded neighbour pair meshes like the two regions meshed together.
- **No headless Chrome.** `client/package.json` and `.github/workflows/` have no browser, SwiftShader or `performance.mark` use. CI is `ci.yml`.

## Reconciliation notes

1. **Section 2 repro can't hold as written.** `createStream` has `budget = 1`, so `mesh` holds at most one region per frame, and `remesh` comes only from edits (`regions.edited`). The scene's `stale` remesh adds at most one more in a frame with no new region. So `regions.max_per_frame` (meshed + remeshed) tops out at 1 for new regions plus the edited ones, and `expect regions.max_per_frame > 1` after sprinting across a border would fail on today's code. Either that is the intended result (the stream budget is the fix, and the path proves it holds: use `expect ... <= 1`), or the repro needs an edit (a pick on a region border) to reach 2. State which in Phase B's done-when.
2. **"Hitch" has no deterministic meaning in the harness.** The harness gives frames synthetic `dt` (`frame <ms>`), so the `hitch` event can only be defined on work, not time. Keep the two definitions apart: Phase A's hitch is wall-clock (`ms > threshold`, scene only); Phase B's `hitch` event is work over a budget (harness). Name the DSL event and the overlay count differently if they would be confused (e.g. `overbudget`).
3. **Moving CPU meshing out of `scene.js` (open question 3).** Feasible: `meshRegion`'s first half uses only `walk`, `terrain`, `ctx`, `seed`, `blendRadius` and `REGION_SIZE`, all of which `dsl.mjs`'s `world()` already builds. It is a refactor of the client, not the terrain crate, so no CLAUDE.md invariant is hit. It touches the render loop, which CLAUDE.md says to keep in `controller.js` where it can be tested, so it fits that rule. Suggest keeping it in this doc as Phase B (it is the only way the harness can count vertices), but as a ticket of its own with no behaviour change, and the `stale` bookkeeping moves with it.
4. **Work budgets and thresholds (section 2, open question 1).** Prior art: the critic doc chose "constants in a check module, each with a test" for layout thresholds (#971, `client/hudcheck.js`). Follow that; a `data/*.toml` entry is wrong here, since `data/` is read by `crates/terrain/build.rs` and is for world data. The wall-clock hitch threshold should be derived from `FRAME_BUDGET_MS` (two frames = `2 * FRAME_BUDGET_MS`, about 33.3 ms) so there is one source of truth; work budgets are plain numbers.
5. **Section 3 (headless Chrome) conflicts with nothing in the invariants** but adds a dev dependency to `client/` (the viewer has no bundler, and `client/package.json` has none today). Keep it a `devDependency` or a tool outside the shipped files; it must not enter `client/index.html`'s load path. `[manual check]` as written: "median over several replays over the threshold" is measurable once the threshold and replay count are constants; propose 5 replays and the Phase A hitch threshold.
6. **Section 4 UV snap (`#1527`).** The doc names issue #1527 as the first case; this reconciler could not read it. The check "snap stays inside the face's own tile at every view angle" needs the shader's snap as a pure function, because GLSL isn't run in Node. Either mirror the formula in JS and test that (risk: drift from the shader; mitigate by building the shader string from the same constants and testing the string, as `TEX_SIZE` already is) or leave it to the Phase F pixel pass. Suggest the first with the shared constant.
7. **"Every face edge shared exactly, at integer positions" (section 4).** True only for cube faces. Water surfaces are lowered, plants and cutouts are inset and models are not on the grid (`client/modelgl.js`, `meshes.cutout`, `meshes.water`). Restrict the check to the `opaque` mesh's full-block faces, or list the exceptions in Phase C's done-when.
8. **Section 5 and `render.seams` vs the art rules.** "Every art colour is from one 16-colour palette" holds for `art/` (CLAUDE.md, Art), but the client also applies biome tints (`columnTints`), AO and sky/fog. The check therefore needs tint off as well as fog off, or a per-block expected colour. Say so in Phase F's done-when. `[manual check]` for "the model only looks at crops"; measurable version: the pass emits a crop list with at most N crops per run.
9. **Critic changes (section 6).** `KINDS` in `finding.mjs`, the schema line in `critic.md`, and the `critic.test.mjs`, `finding.test.mjs` and `criticprompt.test.mjs` expectations all change together. The prompt must not name keys (play-critic doc note 16). Advisory (section 3) findings need a flag the verify pass skips, e.g. `advisory: true`, which `report.mjs` groups separately; today every finding must replay or is dropped.
10. **`workflow` changes.** Phase E's CI schedule (open question 2) means a new or changed file under `.github/workflows/`, which the agent app can't write. The maintainer would add it; put the draft in `.github/workflow-drafts/` as the play-critic doc did.
11. **Overlap.** `docs/design/play-critic-agent.md` is the parent: this doc extends its section 4 (finding kinds) and section 3 (screenshots only when asked). It does not conflict; when D is built, that doc's "kind (control-flow | layout)" line goes stale and the docs updater should note it. `docs/design/viewer-rendering.md` owns the shader work the render checks guard; nothing here changes the shader.
12. **Determinism invariant.** Section 4's border test restates it for meshes; the terrain-level invariant (blocks depend only on seed and coordinate) is already tested in `crates/terrain`. No change to `crates/terrain`, `data/` or world output; no wasm size change except if the mesh move adds exports (it should not).
13. **SPEC.** The doc states no Beta behaviour.
14. **`[external]`.** None. Headless Chrome is a dev tool installed in CI, not a hosted service.
15. **Open question 3 is answered here:** keep it in this doc as its own phase (note 3).

## Idea

# Mince Raft: Play critic sees lag and render faults

Status: idea
Scope: game

The play critic (`docs/design/play-critic-agent.md`) can't see two kinds of problem a player notices at once: sudden lag (a stall while terrain loads) and render faults (dark seams along block edges). It plays through the headless harness, which by design renders nothing: `client/test/dsl.mjs` runs `controller.js`, but region meshing, GPU upload and the shaders all live in `client/scene.js` and never execute. Its findings are limited to `control-flow` and `layout`. This idea gives it two more kinds, `perf` and `render`, each with something an `expect` line can assert so the verify pass still works. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. The frame overlay hides stalls

The overlay shows p50 and p99 over the last 300 frames (`client/frametimes.js`, nearest rank). p99 of 300 drops the slowest 3 frames, so one 150 ms stall reads "within the 16.7 ms p99 budget".

- Want: the overlay and `window.frameStats()` also report the slowest frame in the window and how many frames went over a hitch threshold (e.g. 33 ms, two frames at 60 Hz).
- Want: for each hitch frame, a short record of the work done in it: regions meshed and remeshed, vertices uploaded, sim ticks run and chunks generated.

> **Reconciler:** see notes 2 and 4. The overlay line is in `scene.js` (~line 606); the ring buffer holds times only, so hitch records need a parallel buffer or a separate list.

## 2. Work per frame, counted in the harness

Timing isn't deterministic, so the verify pass can't replay "this frame took 80 ms". The amount of work done in one frame is deterministic, and too much work in one frame is what causes a stall.

- Want: the CPU half of meshing a region (`padRegion` + `meshBlocks` in `meshRegion`, `client/scene.js`) moves out of `scene.js` to code the controller or harness drives, so the harness meshes regions too. Upload stays in `scene.js`.
- Want: DSL paths for work per frame, e.g. `regions.max_per_frame` (meshed + stale remeshes in one frame), `chunks.max_per_tick` (load events from `walk.drain_stream()` in one tick) and `verts.max_per_frame`, each a running maximum since the case began, and a `hitch` event when a frame's work goes over its budget.
- Repro shape: `start walk stream`, sprint or fly across region borders, `expect regions.max_per_frame > 1`.
- Open: the budgets. Constants in a check module with tests, like the layout thresholds (#971), or `data/`.

> **Reconciler:** see notes 1, 2, 3 and 4. The repro's `> 1` conflicts with the stream's budget of one region per frame. Note the code calls `meshBlocksWasm`, not `meshBlocks`.

## 3. Real frame timing, as a separate pass

Counts catch too much work in one frame, but not one unit of work that is slow by itself.

- Want: a perf pass that runs a scripted walk on the real `scene.js` in headless Chrome, with `performance.mark` around tick, mesh and upload, and reads the hitch records from section 1.
- In CI the GPU is SwiftShader (a software renderer), so only the CPU spans count. A finding needs its median over several replays to be over the threshold.
- Its findings are advisory: they carry a repro but don't go through the `expect` verify pass. A finding is marked by an `advisory` boolean field, which `client/tools/verify.mjs` skips and `client/tools/report.mjs` groups apart. (Decided in #1976.)
- This pass runs in CI on a schedule alongside the critic workflow. The pixel pass (section 5) is local only. (Decided in #1975.)

> **Reconciler:** see notes 5, 9 and 10. `[manual check]` "median over several replays over the threshold": testable once the threshold and replay count are constants (note 5).

## 4. Render faults without screenshots

CLAUDE.md keeps screenshots for judging looks, only when the maintainer asks. Most render faults have a cause that can be checked as data instead:

- Texture sampling: the UV snap in the block shader must stay inside the face's own tile at every view angle (#1527 is the first case).
- Mesh cracks: every face edge in a region's mesh is shared exactly, at integer positions.
- Region borders: meshing two padded neighbour regions gives the same border faces as meshing both at once, in line with the determinism invariant.
- Want: these as tests, plus a `render` finding kind the critic can file when one of them fails for a case it found while playing.

> **Reconciler:** see notes 6, 7 and 12. The mesh and border checks run in Node on `meshBlocksWasm` output; the UV snap needs the shader formula as data.

## 5. A pixel pass, when the maintainer asks

- Want: render a fixed seed, pose and viewport headlessly (SwiftShader renders the same way every run) with fog off and fixed shading.
- Every art colour comes from one 16-colour palette, so on a flat wall every pixel should be a palette colour times a known shade. The check counts off-palette and near-black pixels inside solid geometry (`render.seams`), so an `expect` can assert it and no golden images are needed.
- The model only looks at crops around flagged pixels.
- Runs only when the maintainer asks for a screenshot pass, as the play-critic doc already says.

> **Reconciler:** see note 8. `[manual check]` "SwiftShader renders the same way every run": verify by rendering twice in CI and comparing; if it differs, the pass can't be exact.

## 6. Critic changes

- `finding.mjs` and `critic.md` accept `kind` `perf` and `render`.
- The critic's goals include walking or flying across region borders in a streamed world (`start walk stream`), not only the raft scene.
- The verify pass replays `perf` and `render` repros like any other. Section 3 findings are advisory. To let a player-mode critic assert them, the work-per-frame paths (section 2) and the render-fault paths (e.g. `render.faults`) move out of the `debug` observation class into a new class (e.g. `diagnostic`) in `client/tools/observe.mjs`. It is returned in player mode on streamed worlds only, and the critic prompt names those paths. (Decided in #1743.)

> **Reconciler:** see notes 9 and 11. `start walk stream` exists in the DSL.

---

## Phases (suggested)

- A: overlay max and hitch count, hitch records (1)
- B: move CPU meshing out of `scene.js`, work-per-frame DSL paths and `hitch` event (2)
- C: render-fault tests as data: UV snap, mesh cracks, region borders (4)
- D: `perf` and `render` finding kinds in the critic, streamed-world goals (6)
- E: headless Chrome perf pass (3)
- F: palette-check pixel pass, maintainer-triggered (5)

> **Reconciler:** phases kept as written. Done when: A, `FrameTimes` reports max and hitch count (a unit test over a fixed series with one 150 ms frame shows both while p99 stays under budget) and the overlay and `frameStats()` carry them; B, the CPU meshing is a module with no GL, `scene.js` calls it, `meshorigin.test.mjs` still passes and a script asserts the new paths on a `stream` case (note 1); C, the three checks are tests that fail on a seeded fault; D, `KINDS`, the prompt and its tests accept `perf` and `render`, and a fixture `render` finding replays; E, a script runs the scene in headless Chrome and prints hitch records (the workflow draft goes in `.github/workflow-drafts/`, note 10); F, the pass prints a `render.seams` count and a crop list for a fixed seed, pose and viewport. D does not depend on E or F; it needs only A to C for something to file, so E and F, which wait on open question 2, stay last.

## Open questions

1. Hitch threshold and work budgets: fixed numbers, or derived from the 16.7 ms target?
3. Is moving CPU meshing out of `scene.js` (B) acceptable as part of this, or should it be its own design?

## Open questions (reconciler)

1. **Q1 thresholds.** Options: A, derive the hitch threshold from `FRAME_BUDGET_MS` (`2 * FRAME_BUDGET_MS`) and keep work budgets as constants in a check module with tests (suggested, note 4); B, fixed numbers everywhere; C, put them in `data/*.toml` (not suggested, `data/` is for world data read by `build.rs`).
3. **Q3 mesh move.** Answered in note 3: keep it in this doc as Phase B, as its own no-behaviour-change ticket. Options: A, as suggested; B, split into its own design doc.
4. **Q4 section 2 repro.** `expect regions.max_per_frame > 1` can't hold with `budget = 1` (note 1). Options: A, assert `<= 1` as a guard on the stream budget; B, add an edit on a border so remeshes push the count above 1; C, raise the stream budget (a behaviour change, not suggested).
