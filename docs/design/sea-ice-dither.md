---
title: Dither sea ice into open water
status: accepted
scope: generator
source: "#1597"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

- Sea ice is one test per column: `let frozen = t < 0.5 && !biases.open_water.is_some_and(|o| o.open(wx, wz))` (`crates/terrain/src/lib.rs`, around line 902). The density fill puts `ICE` at `y = SEA_LEVEL - 1` when `frozen`, and `surface()` takes the same `frozen` flag for the beach and stone-patch water top. Both read one flag, so a dither only has to change how that flag is computed.
- `OpenWater` (`lib.rs`, around line 167) is the existing hook that overrides freezing; `Biases::open_water` carries it. Layout uses it to keep route channels open (`crates/layout/src/region.rs`, world-layout Phase G, #900).
- T arrives per column as `clim[cx]` after temperature biases are applied and clamped, so the band is measured on biased T.
- `data/habitats.toml` has a `frozen` rule, but `crates/layout/src/derived.rs` (line 61) already computes the frozen flag from the surface block (`col[surface] == Block::Ice`), not from T. `derived_tests.rs` asserts that. Phase B's habitat part is therefore already true.
- The snow pass skips ice by block (SPEC 13), not by T, so dithered ice and dithered open water need no snow change.
- Tests that assume ice iff T < 0.5: `check_snow_ice` in `crates/terrain/src/tests_veg.rs` (around line 263), `open_water_skips_sea_ice_only_on_its_columns` (same file), and `ice_outside` in `crates/layout/src/channel_tests.rs` (around line 281, a count, probably unaffected).
- `data/` has no climate or ice file yet, and the level file (`data/levels/v0.toml`) has no `[climate]` table in the repo checkout.

## Reconciliation notes

- **Beta mismatch, by design.** SPEC 3.6 and 13 say ice is exactly T < 0.5. The idea changes that inside a band. It is a project choice layered on top, like `open_water`; SPEC is not edited and no Beta fact is added. Everything the band does lives in this doc and `data/*.toml`. Suggest: build it as a generator setting in `crates/terrain` (a data-driven band in `data/`, default off for the Beta-parity tests), not a `decision` issue. The only real decisions are the open questions below, and the author already gave a preferred pattern (C).
- **Hook or built in?** The idea asks whether this is a new hook beside `open_water`. Suggest: a built-in rule gated by a plain `Biases`/generator option (band on or off), because the dither depends only on `(seed, x, z, T)`, all of which the terrain crate already has. A callback is needed only if the layout crate must vary it per column. `open_water` still wins, evaluated first.
- **Determinism and invariants.** The dither must be a pure function of seed, world `(x, z)` and T, with no region origin. A hashed per-column value (`hash_f64(seed, x, z, 0, k)` with a new unused `k`, as `surface()` does for `r1..r3`) satisfies this and keeps `crates/terrain` `no_std` with no dependencies. Option C needs a small high-frequency noise sampled per column; if a new noise generator is added, sample it only for columns whose T is inside the band, as the idea says. The band edges go in `data/*.toml` and are read by `build.rs`, not magic numbers.
- **Where the flag is decided.** `frozen` is computed once per column and used twice (fill and `surface()`), so compute the dither there and pass the result on. Do not recompute in `surface()`. Do not decide it from the density lattice.
- **Sea-ice tests assume a hard 0.5.** `check_snow_ice` will need to allow the band: ice required below `t_ice`, no ice above `t_open`, either inside it. The forced-cold test should keep working unchanged.
- **Open water, floe size and the cold test.** The idea's "reads as broken ice" is a visual judgment. `[manual check]` Proposed measurable version: in a fixed-seed region, inside the band the mean connected ice component size (4-connected columns) is at least N columns and the ice fraction falls monotonically with T across, say, five equal T slices.
- **Wasm size.** A band lookup and one noise sample per in-band column should be small; report the gzipped wasm size before and after in the PR (CLAUDE.md).
- **World output changes** in every in-band column. The PRs must say so and include before/after `stats` ice counts (CLAUDE.md, step 6).
- **Overlap with other docs.** `world-layout.md` (Phase G and the sea-ice-in-channels note around line 670) is the only doc touching sea ice. It keeps route channels open via `open_water`; this doc does not conflict as long as `open_water` still wins. `fish-and-fishing.md` and `sea-flora-and-shellfish.md` use the `frozen` habitat; nothing changes for them if habitats keep reading the block.
- **Layout checks by T.** Nothing in `crates/layout` was found counting ice by T; `derived.rs` reads the block. Phase B should confirm this by search rather than assume it.
- **Viewer and flyover.** `[manual check]` The flyover should still plan a route (`node viewer/flyover/preview.mjs 12345`). That command is a testable proxy; whether the ice looks right is visual.

## Mince Raft: Dither sea ice into open water

Status: idea
Scope: generator

Sea ice ends on a hard line. The sea surface at `y = SEA_LEVEL - 1` freezes exactly where the column's T < 0.5 (SPEC 3.6, 13), so sailing from cold to warm water crosses a single smooth contour: solid ice on one side, open water on the other. That edge follows the climate noise exactly, so it looks drawn on. This idea breaks up the edge so the ice thins out across a band of temperature: solid pack, then broken floes, then scattered pieces, then open water. Notes on the current code may be out of date; they are there to help the reconciler.

---

### 1. What exists

- `crates/terrain/src/lib.rs` (around line 902): `frozen = t < 0.5 && !open_water(wx, wz)`, then the fill and the surface pass put `ICE` at `SEA_LEVEL - 1` when `frozen`.
- `biases.open_water` already overrides freezing (route channels stay open, world-layout).
- `data/habitats.toml` has a `frozen` habitat ("surface is ice").
- The snow pass never puts snow on sea ice (SPEC 13).

> **Reconciler:** All four confirmed. The habitat line is stronger than the author thought: layout already derives `frozen` from the block (`crates/layout/src/derived.rs`).

### 2. Want

- Want: a transition band in T, e.g. `[t_ice, t_open]` around 0.5, set in `data/*.toml` (not magic numbers). Below `t_ice` the surface is solid ice as now. Above `t_open` it is open water. In between, a column is ice with a probability that goes from 1 down to 0 across the band.
- Want: the dither is decided per column from the seed and the world (x, z) only, so overlapping regions agree (CLAUDE.md determinism invariant), with and without decoration.
- Want: it reads as broken ice, not static. Options for the pattern:
  - A. Per-column hash compared against the probability (white noise). Simplest. Looks like salt and pepper.
  - B. Ordered dither (a fixed Bayer or blue-noise tile keyed by world coords). Even, but can look regular from above.
  - C. Small high-frequency noise added to T before the threshold (only inside the band). Gives clumped floes with ragged edges and gaps between them. **Preferred.**
  - Combinations (C for the floe shapes, plus A for a few loose single blocks at the warm end).
- Want: it is cheap. One extra noise sample per column only where T is inside the band; no change to the density lattice.
- Want: route channels stay open: `open_water` still wins over dithered ice.

> **Reconciler:** Option C can be done with a smooth noise of a few blocks' wavelength thresholded against T. A per-column hash (option A) needs only `hash_f64`; option C needs a new noise sample, so check its cost against the wasm size. The band should be clamped so `t_ice <= 0.5 <= t_open` does not have to hold (open question 1 decides).

### 3. Things to check

- **This changes Beta behaviour.** SPEC 3.6 and 13 say a hard T < 0.5. This is a project choice layered on top (like `open_water`), not a SPEC change. The reconciler should say whether it belongs as a new hook beside `open_water` or needs a `decision` issue first. Don't take any fact about the original's ice from anywhere but SPEC.
- **World output changes** in every column inside the band. Before/after `stats` (ice count) are needed, and the existing sea-ice tests (`crates/terrain/src/tests_veg.rs`, the "unfrozen sea" / "ice in warm column" asserts) will need to allow the band.
- **Snow.** Dithered-open columns have water on top, so no snow there; dithered-ice columns keep the existing no-snow-on-sea-ice rule. Check nothing else assumes ice ⇔ T < 0.5.
- **Habitats.** `frozen` should follow the actual block (ice or not), not T, so fish under a gap in the floes see open water. Or a new `broken_ice` habitat for the band. Open question.
- **Route validation / climate stats.** Anything in `crates/layout` that counts or avoids ice by T should count the block instead.
- **Viewer and flyover.** Should look right with no change; check the flyover's water/ice handling still plans a route.

> **Reconciler:** Hook vs decision: see the first two notes above (built in, no `decision` issue needed, SPEC untouched). Snow, habitats and layout checked: they already follow the block. The `lib.rs` comment on `surface()` ("the column's T < 0.5") will go stale once the flag is dithered; update it with the code.

### 4. Phases

**A. Band and dither.** The band data, the dither rule (option C unless decided otherwise), the determinism test across overlapping regions, updated sea-ice tests. Done when `stats` shows ice only below `t_open`, all of it below `t_ice` is still ice, and the region-overlap test passes.

**B. Consumers follow the block.** Habitats, layout checks and any T-based ice assumption read the block instead. Done when a column inside the band classifies by its surface block.

> **Reconciler:** Phase B is mostly done already (`derived.rs` and its test). What is left is a test that a column inside the band classifies by its block, plus a search for T-based ice assumptions (none found). If the habitat answer is a new `broken_ice` habitat (open question 3), that is the only new work, and it needs a data-only rule plus the fish data (`data/fish.toml`, whose comment says `frozen` columns match no species). Phase B can be planned together with A if it stays under the 200-line limit; the planner decides.

### 5. Open questions

1. Band width and placement: A. symmetric around 0.5 (ice reaches a bit warmer than today, gaps a bit colder), B. only on the warm side (all of today's ice stays, floes added beyond it), C. only on the cold side (today's ice edge is the outer limit).
2. Pattern: A, B, C or a mix (section 2).
3. Habitat in the band: A. `frozen` per block, B. new `broken_ice` habitat.
4. Should levels be able to turn the band off (a `[climate]` key in the level file), or is it global?

## Open questions

1. **Band placement.** Options: A. symmetric around 0.5; B. warm side only (all of today's ice stays); C. cold side only. Suggest B: it never removes ice from where the route and tests expect it, and `t < 0.5` stays the "solid" limit. Phase A's band data takes whichever is chosen.
2. **Pattern.** Options: A. per-column hash; B. ordered dither; C. noise added to T, preferred by the author. Suggest C, plus a few hashed loose blocks at the warm end only if C alone looks too clean.
3. **Habitat in the band.** Options: A. `frozen` per block (already how layout works); B. new `broken_ice` habitat. Suggest A for now.
4. **Per-level switch.** Options: A. global; B. a key in the level file. The level file has no `[climate]` table in this checkout (`data/levels/v0.toml`), though world-layout Phase G describes one, so check before relying on it. Suggest B only if a level needs the hard edge; otherwise global and data-driven.
5. **Default for parity tests.** Should the terrain crate's default (no level, plain `Biases::default()`) keep the Beta hard edge, with the band opted into by layout/game? Options: A. band on everywhere; B. band off by default, on from the game. Suggest B so the Beta-parity tests in `crates/terrain` stay as they are.
