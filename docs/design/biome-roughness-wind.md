---
title: Biome roughness for wind
status: accepted
scope: game
source: "#1791"
superseded_by: ""
phases:
  A: todo
---

## Current state

Checked against `master` (5a68a3b).

- **Wind is not built yet.** `docs/design/wind.md` is accepted with both phases `todo`. There is no wind module in `crates/game/src/`, no `data/wind.toml`, and no wind export in `crates/game-web/src/lib.rs`. Its "Later, separate ideas" list names biome roughness (`wind.md`, line 90). This idea adds a layer to a function that does not exist, so Phase A here waits on wind Phase A.
- **Breeze doc.** `docs/design/land-sea-breeze.md` is accepted (both phases `todo`) and specifies the cell cache this idea wants to share. Not built: no `L(x, z)`, no cell cache, no `breeze_*` keys.
- **#1788 (terrain shelter and exposure)** is an idea issue only. There is no `docs/design/` file for it, so the `y` input and the smoothed ground height are not accepted yet.
- **Coarse heightmap.** `terrain::generate_coarse_heightmap(seed, x0, z0, step, nx, nz)` (`crates/terrain/src/lib.rs:445`) returns `[top_y, biome, gaps, gap_h]` per sample on the world-aligned 4-block lattice, with no decoration, so it is a pure function of seed and position. Land iff `top_y >= SEA_LEVEL` (`SEA_LEVEL` is 64, `lib.rs:52`). Nothing in game code calls it yet.
- **Single column biome.** `Generator::biome(x, z)` is at `crates/terrain/src/lib.rs:638`; `climate::biome_at(seed, x, z)` is at `crates/terrain/src/climate.rs:122`.
- **Biomes.** `data/biomes.toml` has the 11 `[[biome]]` entries the author lists (Rainforest, Swampland, Seasonal Forest, Forest, Savanna, Shrubland, Taiga, Desert, Plains, Ice Desert, Tundra). `crates/terrain/build.rs` reads them into `BIOME_INFO` (`crates/terrain/src/data.rs`); `Biome::info()` indexes it by discriminant. The biome byte in the heightmap is that discriminant.
- **Tree count.** SPEC step 13 and 9.1 describe the tree-density noise and per-biome terms; `decorate.rs` uses `BIOME_INFO` `tree_bonus`. No public function returns a chunk's tree count without decorating (only needed for option B).
- **Sea ice.** `docs/design/sea-ice-floes.md` (accepted) adds floes; the coarse heightmap reads the density lattice, so ice never changes `top_y` and frozen sea counts as water there.
- **Plan #1860 (Phase A) built:** the `[roughness]` table and drag keys in `data/wind.toml` (#1861, `crates/game/src/wind_roughness_config.rs`, `wind_config.rs`), the pure speed multiplier with smoothstep height easing (#1862, `crates/game/src/wind_drag.rs`), and the seeded forest, plains and sea behaviour tests (#1863, #1912). The fill `RoughnessParams`/`fill_cell`/`roughness_at` is in `crates/game/src/wind_roughness.rs` (#1811).
- **Not built:** the roughness table, `R(x, z)`, the speed multiplier, the readout fields. World output is untouched by this idea.

## Reconciliation notes

- **Phase A still incomplete (plan #1860).** The plan's three tickets are closed, but they cover only the data table, the multiplier function and its tests. Still missing: the bounded shared cell cache (`wind_roughness.rs` says "There is no cache here"); wiring `R` and the multiplier into the wind function, applied to the mean and gust terms separately before the floor and ceiling (nothing in `crates/game/src/` calls the multiplier from the wind function); and the `R` and multiplier fields on the debug wind readout (`data/commands.toml`, `crates/game/src/commands.rs`). The plan deferred these until wind Phase A and Phase B close.

- **Depends on unbuilt docs.** Phase A needs wind Phase A (the `wind` function, `data/wind.toml`, export). The planner can plan this doc as soon as it is merged, but the factory should not start its tickets before wind Phase A's are closed; suggest the first ticket's `Depends on` points at that plan's last ticket. The debug readout fields need wind Phase B's readout.
- **Shared cell cache has no owner yet.** Both this doc and `land-sea-breeze.md` want to build it, and #1788 would too. Whichever of the three is built first builds it as `land-sea-breeze.md` specifies (cell size and sample step as separate keys, a fixed fill window, bounded cache, cold/warm tests); the others add a value per cell. `R` is one more per-cell value on that cache rather than a second cache, and this doc's ticket builds the cache only if the breeze (or shelter) has not. (Decided in #1812.)
- **Table placement.** Author's option A (`[roughness]` in `data/wind.toml`) is consistent with CLAUDE.md ("data lives in `data/*.toml`") and keeps `crates/terrain` and its build script untouched. The coverage test needs the biome names: use `BIOME_INFO` names from the `terrain` crate (the game crate already depends on it) and compare them with the table's keys. Confirm the exact field name on `BiomeInfo` when writing the test.
- **Water is not a biome.** The heightmap's biome byte is the climate biome even over sea, so the water value is a twelfth, separate `water` key beside the biome keys in `[roughness]` (not a nested table), which the coverage test must exempt from the "names a real biome" half. Water is decided by `top_y < SEA_LEVEL`, which also covers frozen sea (as in `land-sea-breeze.md`), answering the author's "confirm" item. (Decided in #1813.)
- **Smoothing window.** As in `land-sea-breeze.md`, a cell value must depend only on the seed and the cell, so the neighbourhood (tens of blocks) reaches into adjacent cells. Fill from a fixed window around the cell centre, never one cut by what is cached. `[manual check]`: "negligible cost" for the fill; measurable form: a cell fill is no slower than the breeze's own fill plus a small constant on the native bench profile, with cold and warm caches asserted equal.
- **Floats and determinism.** `R` is a mean of table lookups and the multiplier uses `+ - * /` and a clamp; the height easing `g(h)` should be a polynomial (for example smoothstep), not trig, to stay within the exact-arithmetic rule from `wind.md`'s determinism note.
- **Height input.** Without #1788's `y`, the multiplier uses a fixed near-ground height (full effect), as the author says. Suggest the wind function takes `h` as an optional parameter only once #1788 lands; Phase A hard-codes full effect and the "above the blending height" test calls the multiplier function directly with an explicit `h`.
- **Combining with shelter (#1788).** The author asks that combined slowing respects the floor. Suggest the floor is applied once, to the product of all speed multipliers, not per layer. Not testable until #1788 is accepted; leave that test to it.
- **Layer order.** The author puts roughness after fronts, breeze and shelter and before the floor and ceiling. The breeze doc currently adds its term and clamps with a cap; both docs should agree where the clamp lives. Settle in whichever wind ticket lands first.
- **Gust damping.** The multiplier acts on the mean and gust parts separately, so the wind function must keep them as two terms until the roughness step. That is a small constraint on wind Phase A's structure; flag it in that plan.
- **SPEC.md.** Beta has no wind. Nothing here states Beta behaviour, and option B's tree-count rule is the only SPEC reference (later phase). No `spec-question` is needed.
- **World output:** unchanged. Add a test, as `wind.md` does, that climate and heightmap output for a fixed seed are identical with the roughness module linked and queried.
- **Debug readout.** Adds `R` and the multiplier; commands live in `data/commands.toml` and `crates/game/src/commands.rs` (see `debug-console.md`).

## Idea (author's text)

# Mince Raft: Biome roughness for wind

Status: idea
Scope: game (client gets it through the existing wind uniforms)

Wind (`wind.md`) blows just as hard through a rainforest as across a desert or the open sea. Real ground cover slows the wind near the ground: forests are calm under the canopy, grassland less so, and bare sand, ice and water barely at all. This idea adds a per-biome **roughness** to the wind function. Wind near the ground is slower over rough biomes and recovers with height above the ground. Forests then feel still, plains and coasts breezy, and the sea is the windiest place of all, which suits a raft game. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `docs/design/wind.md` (accepted): `wind(seed, x, z, t) -> (dx, dz)` in blocks per second, from fronts and gusts, with tunables in `data/wind.toml`. Visual-accuracy cross-platform (#1731), `f64` tick time (#1732). Its "later" list names biome roughness.
- `docs/design/land-sea-breeze.md` (accepted): a land fraction field on a cached, world-aligned cell grid in the game crate, filled from `terrain::generate_coarse_heightmap`. The cache is a pure memo, not sim state.
- #1788 (idea): terrain shelter and exposure. Proposes adding a height `y` to the wind function and sharing the same cell cache for a smoothed ground height.
- `terrain::generate_coarse_heightmap` returns `[top_y, biome, gaps, gap_h]` per lattice sample, so a breeze or shelter cell fill already has each sample's biome for free. `Generator::biome(x, z)` gives a single column's biome.
- `data/biomes.toml` has 11 biomes (Rainforest, Swampland, Seasonal Forest, Forest, Savanna, Shrubland, Taiga, Desert, Plains, Ice Desert, Tundra). It's read at compile time by `crates/terrain/build.rs` into `BIOME_INFO`. Each biome has `tree_bonus` and `tree_noise`, which set how many trees it gets (SPEC 9.1, and the tree-density noise in the tree count rule, step 13).
- Nothing in the wind work knows about biomes yet.

## 2. Want

- Want: **a roughness value per biome**, 0..1, where 0 is smooth and 1 is the roughest, plus one for **open water**. Rough guide: Rainforest and Forest high, Seasonal Forest and Taiga and Swampland fairly high, Shrubland and Savanna middling, Plains and Tundra low, Desert and Ice Desert very low, water lowest. The numbers are the maintainer's to tune. Where it lives:
  - A. A `[roughness]` table in `data/wind.toml` keyed by biome name, read by the game crate. A test checks every biome in `biomes.toml` has an entry and every entry names a real biome.
  - B. A `roughness` field on each biome in `data/biomes.toml`, carried into `BIOME_INFO` by `terrain/build.rs`.
  - **A preferred.** It keeps wind out of the terrain crate and its build script, and touching `biomes.toml` would make the change look like a terrain change when no world output moves.
- Want: **a smoothed roughness field** `R(x, z)`: the mean roughness of the samples in a neighbourhood. Water samples (`top_y < SEA_LEVEL`) use the water value, whatever their biome. Smoothing means forest edges ease in over tens of blocks rather than switching at a biome border.
  - **Reuse the breeze cell cache:** one cell fill already has `top_y` and the biome for every sample, so adding `R` costs one table lookup per sample.
  - If neither the breeze nor shelter has been built yet, this idea builds the shared cache itself, as specified in `land-sea-breeze.md`.
- Want: **optionally, real tree density.** Biome alone says "forest" even where a forest is sparse. Options:
  - A. Biome only. Simple, and smoothing hides the rest.
  - B. Scale the biome's roughness by its tree count from the generator's tree-count rule (biome term plus tree-density noise, SPEC step 13 and 9.1), evaluated per chunk. Closer to the trees the player sees, but needs a new public `terrain` function returning a chunk's tree count without decorating it.
  - C. Count leaves in decorated regions. Exact, but needs decoration, which is too expensive for wind.
  - **A preferred** for the first phase. B can come later if the forests feel wrong.
- Want: **the effect.** Speed multiplier `f = 1 − R · drag · g(h)`, clamped to a floor (wind never stops entirely from roughness alone). `h` is the height above the local ground and `g` eases from 1 at the ground to 0 at a blending height. `drag`, the floor and the blending height (around canopy height, e.g. 8–20 blocks) go in data. In Phase A gusts are undamped (gust damping ratio 0): roughness applies to the mean wind only. Damping gusts can be added later if forests feel dead. (Decided in #1814.) `g(h)` is a smoothstep, continuous in value and slope at both ends. (Decided in #1815.)
  - **Height input:** if #1788's `y` input is accepted, `h = y − ground`. Otherwise, assume a fixed near-ground height (full effect), which is what the player standing on the ground sees anyway.
- Want: **order of the layers.** After fronts, breeze and terrain shelter and before the speed floor and ceiling. Roughness only changes speed, never direction.
- Want: **client.** Nothing new: the effect arrives through the wind sample and uniforms. If wind option B (the per-cell texture proposed in #1788) lands, roughness shows across the view, so a forest in the distance sways less than the meadow next to it. Until then, it applies to the player's own spot.
- Want: the debug wind readout gains `R` and the roughness multiplier at the player.

## 3. Phases (suggested)

- Phase A: the roughness table, `R` on the shared cell cache, the speed multiplier with its height easing, data, tests. Depends on wind Phase A (and builds the cell cache if the breeze hasn't yet).
- Later, separate: tree-density scaling (option B above), and player-built or player-cut forests counting (needs a live block-based field, not generator output).

## 4. Things to check

- **Determinism.** `R` depends only on seed and world position (generated terrain only), never on region origin, streaming or cache state. Tests:
  - Overlapping and cold-cache queries agree.
  - Cache eviction never changes a value.
  - A replay reproduces the result.
- **Behaviour tests on a fixed seed:**
  - Wind deep in a forest is slower than on nearby plains under the same fronts.
  - Wind over open sea is faster than both.
  - Crossing a forest edge, the multiplier changes smoothly over about the smoothing radius, not at the biome border.
  - Above the blending height, roughness has no effect.
- **Biome table coverage.** A test fails if a biome is added to `biomes.toml` without a roughness entry, so new biomes can't silently get zero.
- **Interaction with shelter (#1788).** Both slow the wind. Check that a sheltered valley forest doesn't go dead still, i.e. the combined result still respects the floor.
- **Snow and frozen ground.** Ice Desert and Tundra are snowy and smooth. Frozen sea is water (as `land-sea-breeze.md` notes ice counts as water through `top_y`), so it gets the water value. Fine as is, but confirm.
- **Cost.** One table lookup per sample during a cell fill, one bilinear lookup per query. Negligible on top of the breeze cache.
- **Seeding.** Reads generator output and adds no randomness. **World output: unchanged.** Option A doesn't touch `biomes.toml` or the terrain crate.
- **Wasm size** before and after.
- **Not Beta behaviour.** Beta has no wind. The tree-count rule is read from SPEC only for option B. Everything else is a project choice, and nothing goes in SPEC.md.

## Phases (reconciler)

The author's phases in section 3 are used as written; the header lists only Phase A. The "Later, separate" items (tree-density scaling, player-built forests) are not phases: they need a new `terrain` function or a live block field and are separate ideas.

**Phase A.** The roughness table, `R` and the speed multiplier. Covers: the `[roughness]` table and water key, `drag`, floor, blending height and gust ratio in `data/wind.toml`; `R(x, z)` as a per-cell value on the shared cell cache (building the cache per `land-sea-breeze.md` if the breeze has not); the multiplier `f = 1 − R · drag · g(h)` clamped to the floor, applied to the mean and gust terms separately, after the other layers and before the floor and ceiling; the `R` and multiplier fields on the wind readout. Waits on wind Phase A (and on the readout from wind Phase B for the fields). Done when tests show: every biome in `BIOME_INFO` has a table entry and every entry (except water) names a real biome; cold-cache, warm-cache and overlapping queries agree and eviction changes no value; a replay reproduces the result; on a fixed seed, found by scanning the coarse heightmap, wind deep in a forest is slower than on nearby plains and open sea is faster than both; the multiplier changes smoothly across a forest edge over about the smoothing radius; at or above the blending height the multiplier is 1; the multiplier never goes below the floor; climate and heightmap output for a fixed seed are unchanged; and `./build.sh` succeeds with the `game-web` wasm size reported before and after.

## Open questions

1. **Who builds the shared cell cache?**
   - A. Whichever of the breeze, shelter or this doc is planned first builds it as `land-sea-breeze.md` specifies; the others add a value per cell.
   - B. Plan the breeze's Phase A first and make this doc's ticket depend on it.
   - C. This doc builds a roughness-only cache now and merges it with the breeze's later. Duplicates work; not suggested.
2. **Where does the water value live in the table?**
   - A. A separate `water` key beside the biome keys, exempt from the "names a real biome" test.
   - B. A nested `[roughness.biome]` table and a top-level `water` value.
