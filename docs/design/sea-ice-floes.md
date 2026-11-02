---
title: Dither sea ice into open water
status: accepted
scope: generator
source: "#1597"
superseded_by: ""
phases:
  A: done
  B: todo
---

## Current state

- **Phase A done (plan #1629).** `floe_ice` block and `is_sea_ice` (#1688), layout and mesh/viewer accept it (#1707, #1734), crack textures with client-side variants (#1786), `data/sea_ice.toml` and `Biases::hard_ice_edge` (#1673), and the floe rule with determinism, subset, clumping and no-snow tests in `crates/terrain/src/tests_ice.rs` (#1847). Phase B is still `todo`; the notes below describing the hard edge predate Phase A.
- **Hard edge today.** `crates/terrain/src/lib.rs` (~line 902) sets `frozen = t < 0.5 && !open_water(wx, wz)`. The density fill puts `ICE` at `y = SEA_LEVEL - 1` when `frozen`, and `Generator::surface` does the same for a beach/stone-patch water top. `t` is the unquantised column temperature after the temperature biases.
- **`open_water` hook exists.** `trait OpenWater` and `Biases::open_water` (`lib.rs`, ~lines 165-210); `Layout::with_biases` (`crates/layout/src/region.rs`) builds the `Biases`. `Biases` derives `Default` ("none").
- **Ice is one block.** `ice` is block id 7 in `data/blocks.toml` (opacity 3, `collision = "full"`, not `opaque_cube`); `crates/terrain/src/lib.rs` has `const ICE`. Places that compare against it: the snow pass and `decorate.rs:704` (`below == ICE`), the column scan at `lib.rs:553-554`, `crates/layout/src/derived.rs:61` (`frozen[i] = surface == Block::Ice`), and `tests_veg.rs` (~line 110, ~284).
- **Habitats already follow the block.** `frozen` in `crates/layout/src/derived.rs` comes from the surface block, not T, so Phase B's "consumers follow the block" is mostly true already. `data/habitats.toml` has `frozen` first, then `warm_shallows`, `deep_water`, `rocky_shore`, `cold_open`, `temperate_open`. No `broken_ice` habitat exists.
- **Level file.** `crates/layout/src/level.rs` has `Climate { temp_start, temp_end }`, an optional `[climate]` table that only records what was used (#846, `world-layout.md`). It has no switch today.
- **Tests that assume the hard edge.** `check_snow_ice` in `crates/terrain/src/tests_veg.rs` (~line 262, "unfrozen sea" and "ice in warm column") and `open_water_skips_sea_ice_only_on_its_columns`; `crates/layout/src/channel_tests.rs:281` counts ice outside channels; `derived_tests.rs` asserts `frozen == (top == Ice)`.
- **Art.** `ice` (batch 13, `art/PROMPT.md`) is one 8x8 tile. There are no floe or crack textures.
- **Snow.** SPEC 13's snow rule excludes ice; that stays true if the floe block is also excluded.

## Reconciliation notes

- **A hook, not a SPEC change; no `decision` issue needed.** SPEC 3.6 and 13 describe Beta's hard `T < 0.5`; this doc layers a project choice on top, as `open_water` does, and the maintainer settled the open points (section 5). Suggested shape: a new `Biases` field beside `open_water` (for example `hard_ice_edge: bool`, default `false` so the `Default` derive keeps the band **on**, per decision 5) and the band limits in a new `data/sea_ice.toml` (`t_ice`, floe noise scale and threshold) read by `build.rs`. Never edit SPEC.md.
- **Cold side only keeps the old limit.** Every column with `T >= 0.5` stays water, and a floe column is always a column that was ice before. Test: with the band on, the ice set is a subset of the hard-edge ice set.
- **Floe block proposal (decision 2).** One new block `floe_ice` in `data/blocks.toml`, same physics as `ice`, placed instead of `ice` inside the band where the floe noise says "ice". Solid ice below `t_ice` stays `ice`. Cracks vary per column through texture variants `floe_ice_0..2` chosen **client-side** by a hash of world (x, z), so terrain adds one id, not several. Every `== ICE` site must accept `floe_ice` too; a small `is_sea_ice(b)` helper avoids missing one. Rejected alternative: reuse `ice` and vary only the texture, because then neither the client nor the snow/habitat code can tell band ice from solid pack.
- **Floe noise.** Option C: `t + k * n(x, z)` against 0.5, inside the band only, with `n` one extra high-frequency sample per column. `crates/terrain` is `no_std` with no runtime dependencies, so `n` must come from the crate's existing noise code (or `hash_f64`-based value noise), from the seed and world (x, z) alone. No change to the density lattice.
- **Determinism.** Needs a test: overlapping regions agree, with and without decoration, over a window containing band columns. The decision cannot depend on region origin or size.
- **Two places decide ice.** The fill and the surface pass (`lib.rs:993`, beach/stone-patch water top) both use `frozen`. Compute the per-column result (ice, `floe_ice` or water) once and pass it to both, so a column cannot be ice in one and water in the other.
- **`open_water` still wins.** Route channels stay water; the floe decision is skipped there.
- **Habitats (decision 3).** `frozen` keeps meaning "surface is `ice`" (solid pack). Add `broken_ice` for water columns in the band, both under floes and in the gaps. `HabitatInput` has `temperature`, but the band depends on `t_ice` and the level switch, so a `temp_min`/`temp_max` rule cannot express it exactly. Suggest a `broken_ice = true` rule field set by the layout from the band limits (exposed by `terrain` as a `pub const` or fn) and the level switch. No species lists `broken_ice` yet, so fish are unchanged until one does.
- **Level switch (decision 4).** `[climate]` is `Option<Climate>` and today only records the bias. A new key (suggest `sea_ice_band`, bool, default `true` when absent so old level files keep working) is read by `crates/layout` and turned into the `Biases` field in `Layout::with_biases`; `seed_output.rs` writes it. This is a deliberate exception to "`[climate]` only records what was used" in `world-layout.md`; that doc should note it when this lands.
- **Terrain default is band-on (decision 5).** Callers relying on hard edges (existing tests, the `stats` example, `crates/web`, the viewer) change output in band columns. `stats` should report solid-ice and floe-ice counts, and the PR needs before/after output (CLAUDE.md "Say whether world output changes").
- **Wasm size.** One noise sample and a few constants, so small. PRs report gzipped wasm size before and after.
- **Art (decision 2).** New textures go in `art/PROMPT.md` and are drawn with the batch tools in `art/tools/` (never by hand-editing the generated JSON), in the project palette. `validate.py` must print `ok` with N > 0.
- **[manual check]** "Reads as broken pack ice, not static" and "cracks, broken edges, pressure ridges" are visual. Measurable proxy: in a band window, the fraction of ice columns whose four neighbours are also ice exceeds that of independent random columns of the same ice density, and the open gaps form connected components larger than one column.
- **[manual check]** Viewer and flyover look. Measurable part: `node viewer/flyover/preview.mjs 12345` still plans a route and `node --test client/test/*.test.mjs` passes.
- **Other docs.** `world-layout.md` (route channels, `[climate]`) and `fish-and-fishing.md` (habitats) overlap but do not conflict.

---

# Mince Raft: Dither sea ice into open water

Status: idea
Scope: generator

Sea ice ends on a hard line. The sea surface at `y = SEA_LEVEL - 1` freezes exactly where the column's T < 0.5 (SPEC 3.6, 13), so sailing from cold to warm water crosses a single smooth contour: solid ice on one side, open water on the other. That edge follows the climate noise exactly, so it looks drawn on. This idea breaks up the edge so the ice thins out across a band of temperature: solid pack, then broken floes, then scattered pieces, then open water. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `crates/terrain/src/lib.rs` (around line 902): `frozen = t < 0.5 && !open_water(wx, wz)`, then the fill and the surface pass put `ICE` at `SEA_LEVEL - 1` when `frozen`.
- `biases.open_water` already overrides freezing (route channels stay open, world-layout).
- `data/habitats.toml` has a `frozen` habitat ("surface is ice").
- The snow pass never puts snow on sea ice (SPEC 13).

## 2. Want

- Want: a transition band in T, e.g. `[t_ice, t_open]` around 0.5, set in `data/*.toml` (not magic numbers). Below `t_ice` the surface is solid ice as now. Above `t_open` it is open water. In between, a column is ice with a probability that goes from 1 down to 0 across the band.
- Want: the dither is decided per column from the seed and the world (x, z) only, so overlapping regions agree (CLAUDE.md determinism invariant), with and without decoration.
- Want: it reads as broken ice, not static. Options for the pattern:
  - A. Per-column hash compared against the probability (white noise). Simplest. Looks like salt and pepper.
  - B. Ordered dither (a fixed Bayer or blue-noise tile keyed by world coords). Even, but can look regular from above.
  - C. Small high-frequency noise added to T before the threshold (only inside the band). Gives clumped floes with ragged edges and gaps between them. **Preferred.**
  - Combinations (C for the floe shapes, plus A for a few loose single blocks at the warm end).
- Want: it is cheap. One extra noise sample per column only where T is inside the band; no change to the density lattice.
- Want: route channels stay open: `open_water` still wins over dithered ice.

> **Reconciler:** Section 5 (decision 1) supersedes `t_open`: the band is `[t_ice, 0.5)` and the outer limit is today's edge, so there is no `t_open` key. Options A and B are not used; C is the rule.

## 3. Things to check

- **This changes Beta behaviour.** SPEC 3.6 and 13 say a hard T < 0.5. This is a project choice layered on top (like `open_water`), not a SPEC change. The reconciler should say whether it belongs as a new hook beside `open_water` or needs a `decision` issue first. Don't take any fact about the original's ice from anywhere but SPEC.
- **World output changes** in every column inside the band. Before/after `stats` (ice count) are needed, and the existing sea-ice tests (`crates/terrain/src/tests_veg.rs`, the "unfrozen sea" / "ice in warm column" asserts) will need to allow the band.
- **Snow.** Dithered-open columns have water on top, so no snow there; dithered-ice columns keep the existing no-snow-on-sea-ice rule. Check nothing else assumes ice ⇔ T < 0.5.
- **Habitats.** `frozen` should follow the actual block (ice or not), not T, so fish under a gap in the floes see open water. Or a new `broken_ice` habitat for the band. Open question.
- **Route validation / climate stats.** Anything in `crates/layout` that counts or avoids ice by T should count the block instead.
- **Viewer and flyover.** Should look right with no change; check the flyover's water/ice handling still plans a route.

> **Reconciler:** Answered in Reconciliation notes: a new `Biases` hook, no `decision` issue. Habitats: `frozen` is already block-based (`derived.rs:61`); decision 3 adds `broken_ice`. Nothing in `crates/layout` counts ice by T that I found; the ice-counting tests read the block. The viewer renders blocks by `data/blocks.toml` colour, so `floe_ice` needs an entry there.

## 4. Phases

**A. Band and dither.** The band data, the dither rule (option C unless decided otherwise), the determinism test across overlapping regions, updated sea-ice tests. Done when `stats` shows ice only below `t_open`, all of it below `t_ice` is still ice, and the region-overlap test passes.

**B. Consumers follow the block.** Habitats, layout checks and any T-based ice assumption read the block instead. Done when a column inside the band classifies by its surface block.

## 5. Decided (maintainer)

1. **Cold side only.** Today's ice edge (T = 0.5) is the outer limit: nothing freezes that doesn't freeze now. The band is `[t_ice, 0.5)`; colder than `t_ice` is solid ice, inside the band the ice breaks up into floes and gaps.
2. **Real floes, with a texture.** Not salt-and-pepper blocks. Inside the band the ice forms floes (clumped shapes with gaps of open water between them, e.g. option C's noise), and the floe ice uses its own textures: visible cracks, broken edges, pressure ridges, so it reads as broken pack ice up close and from above. This needs new art in `art/` (listed in `art/PROMPT.md`, drawn with the batch tools, project-own palette) and probably a new block (e.g. `floe_ice`) or texture variants of `ice` chosen per column; the reconciler should propose which. Cracks may vary per block (a few variants picked by a per-column hash) so the pattern doesn't tile visibly.
3. **New `broken_ice` habitat** for water columns inside the band (open gaps and under floes), separate from `frozen`.
4. **Levels can turn it off.** A key in the level file's `[climate]` table disables the band (hard edge at 0.5, as today). Default to be proposed by the reconciler.
5. **On everywhere by default.** The terrain crate's default (`Biases::default()`, no level) uses the band too; it is not opt-in from the game. The hard-edge sea-ice asserts in `crates/terrain/src/tests_veg.rs` change in Phase A to allow the band. A level can still turn it off (answer 4).

These change the phases: Phase A also covers the floe block/texture art, and Phase B adds the `broken_ice` habitat and the level switch.

## Phases (reconciler)

**Phase A.** Terrain, block and art. The band data (`data/sea_ice.toml`), the floe rule (option C), the `floe_ice` block and its texture variants (art batch plus an `art/PROMPT.md` entry), the `Biases` field for the hard edge (band on by default), the `is_sea_ice` helper at every `== ICE` site, determinism tests, and the updated hard-edge asserts in `tests_veg.rs`. Done when: `stats` shows no ice at `T >= 0.5`; every column with `T < t_ice` is still `ice`; band columns are `floe_ice` or water; the ice set is a subset of the hard-edge ice set; the clumping proxy holds; overlapping regions agree with and without decoration; no snow lies on `floe_ice`; `validate.py` prints `ok`. If it won't fit in 200 reviewable lines, the planner splits the art from the terrain code.

**Phase B.** Habitat and level switch. The `broken_ice` habitat (`data/habitats.toml`, `crates/layout/src/habitat*.rs`, `derived.rs`), layout checks that count ice (`channel_tests.rs`, `derived_tests.rs`) following the block, and the `[climate]` key that turns the band off (`level.rs`, `seed_output.rs`, `Layout::with_biases`). Done when: a band column classifies `broken_ice` and a solid-ice column `frozen`; a level with the key off generates the same world as the hard edge; a level without the key gets the band; a level file round-trips the key.

## Open questions

1. **Default `t_ice`.** The band is `[t_ice, 0.5)`.
   - A. 0.40 (proposed).
   - B. 0.35: wider, longer transition.
   - C. 0.45: narrow, close to today's look.
   Tunable in `data/sea_ice.toml`; Phase A's `stats` shows how many columns land in the band.
2. **Where the texture variant is chosen.**
   - A. Client-side, from a world-position hash (proposed): one block id, no terrain cost.
   - B. Terrain places `floe_ice_0..2` as separate blocks: the variant is world data, but it triples ids and every `== ICE` site.
3. **`broken_ice` when the level switch is off.**
   - A. No band, so no `broken_ice` (proposed).
   - B. Keep the habitat over the same T range even with a hard edge.
4. **Level key name and default.**
   - A. `sea_ice_band` (bool), default `true` when absent (proposed).
   - B. `hard_ice_edge`, default `false`; same meaning, inverted.
