---
title: Terrain shelter and exposure for wind
status: accepted
scope: game
source: "#1788"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Checked against `master` (4b4e600).

- **Wind is not built.** `docs/design/wind.md` is accepted with both phases `todo`. There is no wind module in `crates/game/src/`, no `data/wind.toml`, and no wind export in `crates/game-web/src/lib.rs`. This idea changes and extends a function that does not exist yet, so both phases here wait on the matching wind phase.
- **Breeze not built.** `docs/design/land-sea-breeze.md` (#1750, accepted, both phases `todo`) proposes the cell cache of land fractions `L`. No cache, `L` or `generate_coarse_heightmap` call exists in `crates/game`. Its Phase A is not built, so the "share the cell cache if it lands first" condition is open.
- **Coarse heights.** `terrain::generate_coarse_heightmap(seed, x0, z0, step, nx, nz)` (`crates/terrain/src/lib.rs:445`) returns `[top_y, biome, gaps, gap_h]` per sample on the world-aligned 4-block lattice (`step` rounds up to a multiple of 4, the origin down to one). `top_y` is the highest solid block and matches the full generator's terrain height. There is no decoration and no margin, so it is a pure function of seed and position. It is exported to wasm in `crates/web/src/lib.rs:55`, tested in `crates/terrain/src/tests.rs:174`, and not yet called from game code.
- **Water.** `SEA_LEVEL` is 64 (`lib.rs:52`); every non-solid block below it is water. In a water column `top_y` is the seabed, below `SEA_LEVEL`.
- **Player height.** `data/player.toml` has `eye_height` 1.62 and `sneak_eye_drop`. Nothing there is wind-related.
- **Client.** The sway and its single `swayPhase` uniform are as `wind.md`'s Current state describes (`client/sway.js`, `client/scene.js`). There is no per-cell wind texture. `client/frametimes.js` has `FrameTimes`.
- **Day cycle.** `docs/design/day-night-cycle.md` (accepted, `todo`) is not needed here.
- **Not built:** `H(x, z)`, the exposure, shelter and channelling terms, their keys in `data/wind.toml`, the `y` input, the wind texture and the readout lines.

## Reconciliation notes

- **Signature change in an accepted doc (the idea asks for this to be said).** `wind.md` defines `wind(seed, x, z, tick)` and its Phase A tests and export are written against that signature. Option A here makes it `wind(seed, x, y, z, t)`. Because nothing is built, wind Phase A takes `y` from the start, unused until this idea's Phase A, so this idea adds no signature change later. The wind plan needs a note saying so. `wind.md`'s text is the author's and is not edited here; the docs updater records the change when it lands. (Decided in #1798.)
- **Depends on unbuilt docs.** Phase A needs wind Phase A. Phase B needs wind Phase B plus the per-cell wind texture, which `wind.md` lists only as its alternative option B with no phase of its own. Suggest each phase's first ticket `Depends on` the last ticket of the matching wind plan, so the factory does not start early.
- **Order against the breeze.** Layers apply in the idea's order: (fronts + breeze), channelling, shelter, exposure, gusts, then floor and ceiling. This needs the breeze to be a separable term inside the wind function, which `land-sea-breeze.md` already asks for (front speed exposed as an intermediate). If the breeze is built later, its Phase A must slot in before channelling; say so in that ticket. Whichever of this idea and #1750 is built first builds the shared cell cache, storing the heights and the land fraction together; the other adds its field to the same fill. (Decided in #1799.)
- **`H` from the coarse heightmap.** `H = max(top_y, SEA_LEVEL)` gives "water counts as `SEA_LEVEL`", since water columns have `top_y < SEA_LEVEL`. Two caveats: `top_y` is the highest solid block, so floating rock and overhangs raise `H` (the heightmap's `gaps` field marks them; ignoring it is simplest and fits "generated ground"), and the 4-block lattice limits `H` to 4-block resolution, which is fine for radii of tens of blocks. State cell size and sample step as separate keys, as `land-sea-breeze.md` suggests, and assert the cell size is a multiple of the step.
- **Two smoothings.** The idea needs `H` (for shelter and channelling) and `H_wide` (the "wider local mean", for exposure). Both can come from one cell fill. A cell value must depend only on seed and cell, so the fill samples a fixed window (cell plus the largest radius), never one cut by what is cached. A wide radius makes each miss large; report the window size. `[manual check]`, measurable form: one cell fill on the native bench profile fits a 20 Hz tick's budget, and cold and warm caches give equal values.
- **Look-ups can cross cell edges.** Shelter looks up to the largest `d_i` upwind, so one query can touch several cells and trigger several fills. Determinism is unaffected. The cache must be a pure memo, bounded, with eviction never changing a value (as `land-sea-breeze.md` requires). It lives in the game crate, so `crates/terrain` stays `no_std` with no runtime dependencies.
- **Floats.** Keep every step to `+ - * /`, `floor` and `sqrt`: bilinear lookups, finite differences for `∇H`, `ŵ` via `sqrt`, the 90° rotation by swapping components, and easing as smoothstep or other polynomials (no `sin`, `exp` or `pow`). The 1e-9 native-versus-wasm tolerance of #1731 then applies unchanged.
- **Zero wind.** `ŵ = w / |w|` is undefined when the wind is calm, and `wind.md` says calm is possible. Suggest `ŵ = w / max(|w|, ε)` with `ε` in data, so the shelter samples do not jump and the factor fades with the wind. Without it, "no jumps when the wind direction changes" fails near calm.
- **Channelling sign is discontinuous as written.** "Sign chosen to agree with the incoming wind" flips when the wind is perpendicular to the contour. Suggest projecting the incoming wind onto the contour line, `c·(c·w)` with `c` the unit contour direction, and blending toward that: no sign choice, continuous, and the wind keeps whichever way along the valley it already pointed. Also fade the turn out as `|∇H|` goes to zero (flat ground has no contour direction).
- **Continuity test.** `max_i` and `clamp` are continuous, so the "small steps in position, height or time give small steps in output" test holds if the eased pieces are. Derive the step bound from the data keys, not a hard-coded number.
- **Height for callers without one.** The fallback in option A (surface value) is `H(x, z)` plus the data offset. Provide one helper in the game crate (a surface-height function) so the sim and client do not each compute it. The client passes eye height; the raft and other entities pass their own `y`.
- **Wind option B texture (Phase B).** One query per texture cell per frame multiplies the cost by the cell count. Fronts and gusts are the same across the view, so only the terrain factors need to vary by cell. Suggest the texture holds per-cell terrain results (speed factor and channelled direction) around the player, world-aligned and scrolled by whole cells so values do not shimmer as the player moves. `sway.js` and `swayGlsl` change together, with a mirror test in the `plantfade.js` pattern; reduced motion still gives zero sway.
- **"Hills feel windy, hollows feel calm".** `[manual check]`. The author's behaviour tests (hilltop vs plain, downwind vs upwind of a ridge, N–S valley with a westerly) are the measurable form; find the terrain features by scanning the coarse heightmap rather than hard-coding coordinates, so the tests survive generator changes.
- **Cost.** `[manual check]`, measurable form: median frame time on the fixed flyover seed within 3% of before (as `wind.md` asks), and the first query in new terrain stays under one tick of budget.
- **Over water.** Sea is flat at `SEA_LEVEL`, so a raft on open sea sees exposure near neutral; a raft under a cliff sees the cliff's `H`. The test can find a cliff-backed coast by scanning the coarse heightmap.
- **Not counted (on purpose).** Trees, canopy and player-built blocks do not shelter. A cheap test: placing blocks changes no wind value, because the field reads only the generator.
- **World output:** unchanged. No block, height, biome or decoration depends on wind. Add the climate and heightmap-unchanged test `wind.md` asks for.
- **SPEC.md.** Beta has no wind. Nothing here states Beta behaviour; the only Beta fact used (`top_y` against `SEA_LEVEL`) is in `generate_coarse_heightmap`'s own documentation. No `spec-question` is needed.
- **Wasm size.** Report the `game-web` gzipped size before and after each phase.

## Idea (author's text)

# Mince Raft: Terrain shelter and exposure for wind

Status: idea
Scope: game (client gets it through the existing wind uniforms)

Wind (`wind.md`, accepted from #1725) and the land and sea breeze (#1750) take no account of the ground's shape. A hilltop, a deep valley and the sheltered side of a ridge all get the same wind. This idea adds a terrain layer that changes the wind's strength and direction at a point from the generated ground around it:
- **exposure:** stronger the higher a point stands above its surroundings.
- **shelter:** weaker in the lee of higher ground upwind.
- **channelling:** in low ground, the wind turns to run along valleys.

Hills then feel windy, hollows feel calm, and wind funnels through gaps. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `docs/design/wind.md` (accepted): `wind(seed, x, z, t) -> (dx, dz)` in blocks per second, from fronts and gusts. `t` is `f64` ticks (#1732). Visual-accuracy cross-platform with a 1e-9 tolerance (#1731), built from `+ - * /`, `floor`, `sqrt`. Tunables in `data/wind.toml`. Phase B samples it once per frame at the player and drives sway through uniforms (option A). Option B, a per-cell wind texture, is the noted alternative. Its "later" list names terrain shelter and channelling.
- #1750 (idea): land and sea breeze. Proposes a coarse, world-aligned, cached grid of land fractions in the game crate, filled from `terrain::generate_coarse_heightmap`.
- #1748 / PR #1749: day and night cycle (not needed here).
- `terrain::generate_coarse_heightmap`: world-aligned lattice samples with `top_y` (highest solid block), matching the full generator's terrain height.
- The wind function has no height input. It's the same at sea level and on a peak.

## 2. Want

- Want: **a smoothed ground height field** `H(x, z)`: generated terrain height with water columns counted at `SEA_LEVEL` (so sea floor depth doesn't matter), on a coarse world-aligned grid. Pure function of seed and position, using generated terrain only (player-built blocks don't count). **Share the cell cache with the breeze's land fraction** (#1750) if that lands first: one cell fill gives both values. Cell size and smoothing radius go in `data/wind.toml`.
- Want: **a height input.** Exposure needs to know how high the query point is. Options:
  - A. Add `y` to the wind function, `wind(seed, x, y, z, t)`. The client passes the player's eye height and the sim passes the entity's height.
  - B. No `y`: always evaluate at the local ground height `H(x, z)`, plus a fixed offset in data.
  - **A preferred**, with B's surface value as what callers use when they have no height. This changes the signature in an accepted doc, so the reconciler should say so.
- Want: **exposure.** A speed factor from the point's height above the wider local mean (`y − H_wide(x, z)`): above 1 on hilltops and ridges, below 1 in hollows. It's clamped, with the gain, the floor and the ceiling in data. It eases in over a few blocks, so there's no jump at the ground.
- Want: **shelter.** Look upwind along the wind's direction at a few distances `d_i` (in data, e.g. 8, 16, 32 blocks). The steepest upwind rise `max_i (H(p − ŵ·d_i) − y) / d_i` sets a shelter factor that falls from 1 toward a floor as the rise steepens. Behind a cliff or ridge the wind drops. On the windward face it doesn't.
  - `ŵ` is the unit wind direction, found with `sqrt`.
  - Which wind sets the upwind direction: the fronts alone, or fronts + breeze? Fronts + breeze preferred, so the breeze can be sheltered by a headland.
    > **Reconciler:** Decided: fronts + breeze set the shelter direction. This needs the breeze, so it waits on #1750 Phase A; until the breeze is built, shelter uses the fronts only. (Decided in #1800.)
- Want: **channelling.** In low ground (below the local mean), turn the wind part-way toward the contour direction of `H`: `∇H` rotated 90°, with the sign chosen to agree with the incoming wind. The turn is stronger the deeper the point sits and the steeper the valley sides. No trig: rotate by swapping components and blend vectors. Strength goes in data.
- Want: **order of the layers.** (fronts + breeze) gives the incoming direction. Then channelling, shelter and exposure are applied, then gusts, then the existing speed floor and ceiling. Gusts that roll in a sheltered spot are weaker too.
- Want: **client.** Under wind Phase B option A, one sample at the player's eye drives every swaying plant in view. Standing in a valley would calm the hilltop trees in the distance too. Options:
  - A. Accept it for now. The player's own surroundings are what they notice.
  - B. Move to wind option B (a small per-cell wind texture around the player, sampled by world position in the shader) so shelter shows across the view. Coarse cells match the grid this layer already has.
  - **B preferred** for this idea's visible phase. It's the first layer that varies on a scale smaller than the view.
- Want: the debug wind readout gains the exposure, shelter and channel turn at the player.

## 3. Phases (suggested)

- Phase A: `H` with its cache (shared with the breeze if present), the height input, exposure, shelter, channelling, data and tests. Depends on wind Phase A.
- Phase B: visible across the view (wind option B texture), plus the readout. Depends on wind Phase B.

## 4. Things to check

- **Determinism.** Values depend only on seed, world position, height and time, never on region origin, load order, streaming or cache state. Tests:
  - Overlapping and cold-cache queries agree.
  - A replay reproduces the result.
  - Cache eviction never changes a value.
- **Behaviour tests on a fixed seed:**
  - A hilltop is windier than the plain around it.
  - A point just downwind of a ridge is calmer than one just upwind.
  - In a valley running roughly north–south with a westerly wind, the wind at the valley floor has turned toward north or south.
- **Continuity.** No jumps when the wind direction changes (shelter samples move with it), at cell edges (bilinear) or at the ground (eased exposure). Add a test that small steps in position, height or time give small steps in output.
- **Cost.** Shelter adds a few height lookups per query, all bilinear on cached cells. Wind option B means many queries per frame (one per texture cell), so measure `FrameTimes` and the first query in new terrain (cell fill hitch).
- **Over water.** Sea counts as flat at `SEA_LEVEL`, so the open sea is exposed and coasts behind cliffs are sheltered. Check a raft under a cliff gets calmer wind.
- **Not counted (on purpose):** trees and forest canopy (that's the biome roughness layer, a separate idea), and player-built walls and buildings. Say so in the doc, so nobody expects a hut to block the wind.
- **Seeding.** Reads generator output, adds no randomness. **World output: unchanged.**
- **Wasm size** before and after.
- **Not Beta behaviour.** Beta has no wind. Everything here is a project choice, and nothing goes in SPEC.md.

## Phases (reconciler)

The author's suggested phases in section 3 are used as written; the header lists them.

**Phase A.** The terrain factors in the wind function. Covers: the `H` / `H_wide` cell fill from `generate_coarse_heightmap` over a fixed window, bilinear lookup and a bounded cell cache in a new game-crate module (shared with the breeze's cache if that has landed, per open question 2); the use of the `y` input that wind Phase A already takes (decided in #1798) and the surface-height helper; exposure, shelter and channelling applied in the idea's order between the (fronts + breeze) sum and the gusts; the new keys in `data/wind.toml`. Waits on wind Phase A. Done when unit tests show: cold-cache, warm-cache and overlapping queries agree; eviction changes no value; a replay reproduces the result; on a fixed seed a hilltop is windier than the plain around it, a point just downwind of a ridge is calmer than one just upwind, and the wind on the floor of a roughly north–south valley under a westerly has turned toward north or south; a raft-height point under a cliff is calmer than open sea; small steps in position, height, time and wind direction give small steps in output; placing blocks changes no value; climate and heightmap output for a fixed seed are unchanged; and `./build.sh` succeeds with the `game-web` wasm size reported before and after.

**Phase B.** Visible across the view. Covers: the world-aligned per-cell wind texture around the player (wind option B), sampled by world position in the vertex shader for plants, leaves and trunks, with the `sway.js` mirror and `swayGlsl` changed together; the exposure, shelter and channel-turn lines on the wind readout. Waits on wind Phase B. Done when the sway and particle tests pass, the client mirror matches the wasm values, an input script checks the readout lines at a fixed position, the texture is stable as the player moves (values at a world cell do not change), and median frame time on the fixed flyover seed is within 3% of before.

## Open questions

2. **Who builds the shared cell cache?**
   - A. Whichever of the two ideas (this one or #1750) is built first builds it, storing the heights and the land fraction together; the other adds its field to the same fill.
   - B. This idea builds its own cache and #1750 later merges into it (or the reverse), with a follow-up ticket.
3. **Does shelter also reduce the gust amplitude?** The order puts gusts after shelter, and the author says gusts in a sheltered spot are weaker too.
   - A. Yes: the shelter factor multiplies the gust amplitude as well as the front speed.
   - B. No: gusts are added at full strength after the terrain factors, then the floor and ceiling apply.
