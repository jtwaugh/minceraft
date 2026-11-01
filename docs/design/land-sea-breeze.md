---
title: Land and sea breeze
status: accepted
scope: game
source: "#1750"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Checked against `master` (731a1ff).

- **Wind is not built yet.** `docs/design/wind.md` is accepted with both phases `todo`. There is no `wind` module in `crates/game/src/`, no `data/wind.toml`, and no wind export in `crates/game-web/src/lib.rs`. This idea adds a layer to a function that does not exist, so Phase A here waits on wind Phase A.
- **Land test.** `terrain::generate_coarse_heightmap(seed, x0, z0, step, nx, nz)` (`crates/terrain/src/lib.rs:445`) returns 4 bytes per sample, `[top_y, biome, gaps, gap_h]`; land iff `top_y >= SEA_LEVEL` (`SEA_LEVEL` is 64, `lib.rs:52`). Samples sit on the world-aligned 4-block lattice: `step` rounds up to a multiple of 4 and `x0`, `z0` round down to one. It has no decoration and no margin, so it is a pure function of seed and position. It is already exposed to wasm by `crates/web/src/lib.rs:55`, and tested in `crates/terrain/src/tests.rs:174`.
- **Game crate and terrain.** `crates/game/Cargo.toml` and `crates/game-web/Cargo.toml` depend on `terrain`. The game crate calls other `terrain` APIs (for example in `stream.rs`, `targeting.rs`), but nothing calls `generate_coarse_heightmap` from game code yet. `crates/layout` calls it in `region_tests.rs`.
- **No land fraction field.** There is no land/water field, distance-to-coast or cell cache in the game crate.
- **Day cycle.** `#1748` is an idea issue; there is no `docs/design/` file for it, and no `time_of_day` or `daylight` in the code. Phase B here depends on it being accepted and built.
- **Sea ice.** `docs/design/sea-ice-floes.md` (accepted, `todo`) adds `floe_ice` in a band of cold water. The coarse heightmap reads the density lattice, not the surface pass, so ice never changes its `top_y`: frozen sea counts as water there.
- **Not built:** `L(x, z)`, the breeze term, the breeze readout line. Climate, terrain and decoration are untouched by this idea.

## Reconciliation notes

- **Phase A still incomplete (plan #1760).** The plan had no tickets (it deferred everything until wind Phase A exists), and the code has none of Phase A: no `L(x, z)` land fraction field, cell cache, `∇L`, breeze term or `breeze_*` keys (no `breeze` or land-fraction code in `crates/` or `data/`, and no wind module). Still to build: all of Phase A, once wind Phase A is built and open questions 2 and 3 are decided.
- **Depends on an unbuilt doc.** Phase A needs wind Phase A (the `wind` function, `data/wind.toml`, the export). The planner can plan this doc's Phase A as soon as it is merged, but the factory should not start its tickets before wind Phase A's are closed. Suggest the first ticket's `Depends on` points at the wind Phase A plan's last ticket. Phase B also needs wind Phase B's readout for the breeze line.
- **Ice is decided by the generator, not here.** The author's "What counts as water" check asks about ice. With the coarse heightmap as the land test, sea ice and `floe_ice` count as water because `top_y` is the seabed or land under them. That is the "water" answer, and it needs no extra code. See open question 1 for whether to keep it.
- **Lattice granularity.** The heightmap step is a multiple of 4, so `L` cannot be sampled finer than 4 blocks. The smoothing radius is tens of blocks, so this is fine. Cache cells (16 or 32 blocks) are then 4×4 or 8×8 samples. Suggest `data/wind.toml` states the cell size and the sample step as separate keys and a test asserts the cell size is a multiple of the step.
- **Smoothing at cell edges.** `L` is "the share of land in a smoothed neighbourhood", but option A stores one value per cell and interpolates. For determinism, a cell value must depend only on the seed and the cell, so its neighbourhood (radius up to tens of blocks) reaches into adjacent cells. Suggest the cell fill samples a fixed window around the cell centre (cell plus radius margin), never a window cut by what has been cached. The cost is a larger fill per miss; check it against `FrameTimes` (author's cost item). `[manual check]`, measurable form: a fill of one cell completes in under one 20 Hz tick's budget on the native bench profile, and the cell fill is asserted equal for cold and warm caches.
- **Cache location and bounds.** The cache lives in the game crate, outside `terrain`, so `crates/terrain` stays `no_std` with no runtime dependencies. A bounded map keyed by cell (evict the farthest from the last query) is std-only, which the game crate may use. The cache must not be sim state that a replay has to record: it is a pure memo, and tests must show eviction changes no value.
- **Time type.** `wind.md` takes `t` as `f64` ticks (decided in #1732). `s(t)` here takes the same. Day length and the dawn/dusk turn come from #1748 in Phase B.
- **Floats.** `wind.md` limits the wind to `+ - * /`, `floor` and `sqrt`, with a 1e-9 native-versus-wasm tolerance (#1731). Bilinear interpolation, finite differences and `s(t)` as a smooth ramp (for example a smoothstep, not `sin`) stay inside that. If #1748's `daylight` uses trig, `s(t)` inherits its tolerance; say so in the Phase B ticket.
- **Combination with fronts.** The author prefers B, `breeze × (1 − front_speed / ceiling)`. This needs the front speed before the sum, so the wind function must expose the front speed (and the ceiling from `data/wind.toml`) as an intermediate. Suggest wind Phase A returns it from an internal function, so this layer adds no second front query. Clamp the factor to 0..1.
- **Gust advection.** The wind doc advects gusts with the *front* wind at the query point (one level, no recursion). Joining the breeze to that advection is a one-line change in the gust coordinates, but it makes the gust position depend on `L`, so every gust sample needs a gradient lookup. See open question 2.
- **Seed and data.** `L` adds no randomness, so no new seed constant is needed. New keys (`breeze_radius`, `breeze_cell`, `breeze_step`, `breeze_k`, `night_ratio`, `breeze_cap`) go in `data/wind.toml`, read as the wind keys are.
- **Wasm size.** `game-web` already links `terrain`, and `generate_coarse_heightmap` is public there, but the lattice code may not be linked into `game-web` today. Report the `game-web` gzipped size before and after, as `wind.md` asks.
- **Direction test.** "A straight stretch of coast on a fixed seed" is checkable. Suggest the test finds a shoreline by scanning the coarse heightmap, rather than hard-coding coordinates, so it survives generator changes.
- **"Weaker breeze at night" and "calm spell".** Checkable once Phase B exists: assert `|s(night)| < |s(day)|` by `night_ratio`, `s` continuous across dawn and dusk (no jump above a small step), and `s = 0` at some time between. `[manual check]` only for whether it feels right.
- **World output:** unchanged. No block, height, biome or decoration depends on the breeze. Add a test, as `wind.md` does, that climate and heightmap output for a fixed seed are identical with the breeze module linked and queried.
- **SPEC.md.** Beta has no wind. Nothing here states Beta behaviour; the only Beta fact used is the `top_y >= SEA_LEVEL` land test, which `generate_coarse_heightmap`'s own documentation gives. No `spec-question` is needed.

## Idea (author's text)

# Mince Raft: Land and sea breeze

Status: idea
Scope: game (client gets it through the existing wind uniforms)

Wind (`wind.md`, accepted from #1725) comes from drifting fronts and gusts, which are the same over land and sea. Real coasts have their own wind. Land warms faster than water by day, so the breeze blows in off the sea. At night the land cools faster, so a weaker breeze blows back out. This idea adds that as a third layer of the same wind function: a breeze that points across the coastline, strongest at the shore, fading inland and out to sea. Its sign and strength follow the time of day. For a raft game, it makes coasts feel different from open water and gives the player something to read about where land is. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `docs/design/wind.md` (accepted): `wind(seed, x, z, t)` returns `(dx, dz)` in blocks per second from fronts (pressure noise, gradient rotated 90°) and gusts. `t` is `f64` ticks (decided in #1732). Exact to visual accuracy across platforms, with a 1e-9 tolerance (decided in #1731). It's built from `+ - * /`, `floor`, `sqrt` where possible. `data/wind.toml` holds its tunables. Phase B drives sway, trunks and snow from one wind sample at the player. Its "later" list names the land/sea breeze.
- `#1748` (idea): day and night cycle, proposing `time_of_day(tick)` and `daylight(t)` as pure functions in the game crate.
- `terrain::generate_coarse_heightmap(seed, x0, z0, ...)`: world-aligned lattice samples with `top_y`, where land iff `top_y >= SEA_LEVEL`. It matches the full generator's height and is much cheaper than a full heightmap. The game crate already depends on `terrain`.
- There's no land/water field in the game crate and no notion of "distance to coast".

## 2. Want

- Want: **a land fraction field** `L(x, z)` in 0..1: the share of land in a smoothed neighbourhood around a point, from the generator's land/water test (`top_y >= SEA_LEVEL`). It's a pure function of seed and position, using generated terrain only, so player-built blocks and rafts never change it. Smoothing radius (tens of blocks; how far the breeze reaches from the shore) and lattice step go in `data/wind.toml`.
  - Options for computing it cheaply:
    - A. A coarse world-aligned grid of land fractions, one value per cell (e.g. 16 or 32 blocks), computed on demand from `generate_coarse_heightmap` and cached in the game crate by cell. Bilinear between cells. Values depend only on seed and cell, so the cache can't change results.
    - B. Sample the coarse heightmap fresh around each query. No cache, but many samples per query.
    - C. Read land/water from regions the client has already streamed. Cheapest, but undefined outside loaded regions, and the sim and client could disagree.
    - **A preferred.**
- Want: **the breeze vector** `b = k · s(t) · ∇L`, added to the front + gust wind.
  - `∇L` points from sea toward land, so a positive `s` is the daytime sea breeze (onshore) and a negative `s` is the night land breeze (offshore).
  - Unlike fronts, the breeze runs *down* its pressure difference, across the coast, not along it. So it's added as a vector, not folded into the front pressure field.
  - `∇L` is largest at the shore and falls to zero well inland and far out to sea, so there's no explicit distance cutoff.
  - The gradient is by finite differences, like the fronts, with no trig.
  - `k`, the night/day strength ratio and a cap go in `data/wind.toml`.
- Want: **time of day drives `s(t)`.** While there's no day cycle, `s` is a constant daytime value (the world is always afternoon now). When #1748 lands, `s(t)` comes from its `daylight`, with a smooth turn at dawn and dusk and a calm spell between. The turn shouldn't be a flip.
- Want: **the fronts still win when they're strong.** On a calm day the breeze is the main wind at the coast. In a storm it's a small nudge. Options:
  - A. Plain sum, then the existing speed ceiling.
  - B. Scale the breeze by `1 − front_speed / ceiling`.
  - **B preferred**, so storms aren't bent by the coastline.
- Want: **gusts follow the total wind.** The wind doc advects gusts with the front wind. Decide whether the breeze joins that advection, so gusts roll inland on a sea breeze.
- Want: **nothing new on the client.** The breeze arrives through the same per-frame wind sample and uniforms as wind Phase B. The debug wind readout gains a breeze line (the `L` and `b` at the player) so it can be checked.

## 3. Phases (suggested)

- Phase A: `L(x, z)` with its cell cache, the breeze term with constant daytime `s`, data, tests. Depends on wind Phase A.
- Phase B: `s(t)` from the day cycle. Depends on #1748's Phase A (time of day and daylight). Skip it if #1748 isn't accepted.

## 4. Things to check

- **Determinism.** `L` and the breeze depend only on seed, world position and time, never on region origin, load order, what the client has streamed or the cache's state. Tests:
  - Overlapping and cold-cache queries agree.
  - A replay reproduces the breeze.
  - The cache is bounded (evict far cells) and eviction never changes a value.
- **Direction test.** On a fixed seed, find a straight stretch of coast. By day the breeze points onto land, by night off it. At the shoreline it's stronger than a smoothing radius inland or out to sea.
- **Cost.** Per query: four bilinear lookups for `∇L`, plus a cell fill on a cache miss. A cell fill runs `generate_coarse_heightmap` over one cell, so check the first query in new terrain doesn't hitch (`FrameTimes`). Fill cells near the player ahead of time if it does.
- **What counts as water.** `top_y < SEA_LEVEL` covers ocean and lakes at sea level. Frozen sea is ice on top. Decide whether ice counts as water (it still borders land) or as land (frozen ground doesn't heat like open water), or sits in between.
- **Islands and small lakes.** A small island or pond smaller than the smoothing radius should give a weak breeze, not a strong one. Smoothing does that, but check one example of each.
- **Seeding.** `L` reads generator output and adds no randomness. Climate, terrain and decoration are unchanged. **World output: unchanged.**
- **Wasm size** before and after (the coarse heightmap path may pull in generator code `game-web` didn't link before).
- **Not Beta behaviour.** Beta has no wind. Everything here is a project choice, and nothing goes in SPEC.md.

## Phases (reconciler)

The author's suggested phases in section 3 are used as written; the header lists them.

**Phase A.** The land fraction and the breeze with a constant daytime strength. Covers: the `L(x, z)` field in a new game-crate module (cell fill from `generate_coarse_heightmap` over a fixed window, bilinear lookup, bounded cell cache); the `∇L` finite differences; the breeze term `k · s · ∇L` scaled by `1 − front_speed / ceiling` (or the plain sum, per open question 3) and clamped by the cap, added into the wind function; the new keys in `data/wind.toml`; the breeze line on the wind readout. Waits on wind Phase A (and on the readout from wind Phase B for the line). Done when unit tests show: cold-cache, warm-cache and overlapping queries agree; eviction changes no value; a replay reproduces the breeze; on a fixed seed the breeze at a shoreline found by scanning the coarse heightmap points onto land and is stronger there than one smoothing radius inland or out to sea; a small island and a small pond give a weaker breeze than a long coast; a strong front leaves the breeze near zero; climate and heightmap output for a fixed seed are unchanged; and `./build.sh` succeeds with the `game-web` wasm size reported before and after.

**Phase B.** Time of day. Covers: `s(t)` from #1748's `daylight`, with a smooth turn at dawn and dusk, a calm spell between, and the night strength ratio from `data/wind.toml`. Waits on #1748 being accepted and its Phase A built; skip it if that does not happen. Done when tests show: `s` is positive by day and negative at night, `|s|` at night is the ratio times the day value, `s` is continuous across dawn and dusk and passes through zero, and the direction test from Phase A flips sign between day and night.

## Open questions

1. **Does ice count as water for `L`?**
   - A. Yes, water. This is what the coarse heightmap gives with no extra code (ice sits above the seabed `top_y`). Simplest.
   - B. Land, or in between. Needs column data beyond the coarse heightmap, so a different, costlier source.
2. **Do gusts advect with the total wind, including the breeze?**
   - A. No: gusts keep advecting with the front wind, as in `wind.md`. Cheapest, one gradient lookup per query.
   - B. Yes: gusts roll inland on a sea breeze. Needs the breeze at each gust sample's position, and a change to the wind Phase A test "gusts advect with the wind".
3. **How does the breeze combine with the fronts?**
   - A. Plain sum, then the speed ceiling.
   - B. Breeze scaled by `1 − front_speed / ceiling` (the author's preference). Needs the front speed exposed inside the wind function.
4. **If #1748 is not accepted, should Phase B wait or be dropped?**
   - A. Wait; leave Phase B `todo`.
   - B. Drop it and keep the constant daytime `s`.
