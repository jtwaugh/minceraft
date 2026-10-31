---
title: Swedish summer flora and environmental niches
status: accepted
scope: generator
source: "#1686"
phases:
  A: todo
  B: todo
  C: todo
  D: todo
  E: todo
  F: todo
---

## Current state

- `data/flora.toml` has 22 entries (the author's list is right); shapes `cross`, `tall_cross`, `ground_cover`, `pad`, `bush` are the `FloraShape` enum in `crates/terrain/src/flora.rs`. Flora is generated into `FLORA` by `crates/terrain/build.rs`; constants live in `[settings]` of the same file.
- `flora::place` is column-local; `holds` reads `FLORA_COAST_RADIUS` (3) and `FLORA_NEAR_WATER_RADIUS` (1) columns around. `FloraRequires` is `NearWater | Canopy | Coast`, one per entry. `Canopy` is `under_canopy`: highest grass with air above and leaves anywhere higher in the column (binary; only `jungle_bush` uses it).
- Decoration pads regions by `MARGIN_CELLS = 4` cells (64 blocks) plus `READ_REACH = 8` (`crates/terrain/src/decorate.rs`). Every radius the doc proposes for terrain-side layers (4/8/16 rings, 48-block exposure ray) fits.
- Placement already picks per column: eligible entries, a colour-noise winner weighted by chance, then a patch-noise and hash roll. A niche response multiplies the chance at the "eligible" step.
- `crates/layout/src/derived.rs` has `shore_distance`, capped at `SHORE_DISTANCE_CAP = 64` and padded by `SHORE_PAD` so it is region-independent. It is a layout product (habitats, fish), not an input to flora today.
- `Biases` (`crates/terrain/src/lib.rs`) is a struct of slices of trait objects (`DensityBias`, `OpenWater`, ...) plus options. There is no per-column flora input yet, so the hydrology hook of 3.4 is a new field or trait of the same shape.
- `berry_bush` exists (Taiga 0.012, Shrubland 0.015); `frost_grass` has `snow_stays = true`, the model for the evergreen berries and lichen.
- Not present: any `niche` table, light/slope/aspect/TPI/edge/exposure/wetness/disturbance layers, any of the section 2 plants, and any season clock.

## Reconciliation notes

- **Daisy, bluebell, cattail naming.** Correct as the author says. Nothing in code needs changing; the Swedish names would only be display names (`data/items.toml`, lang), not block ids. Renaming ids is not proposed and should not be done here.
- **`berry_bush` split.** The doc says it "could be split". Splitting changes world output for Taiga and Shrubland; do it in the same PR that adds the real berries (lingon, blåbär) so `stats` shows one clear change.
- **SPEC check.** SPEC.md Part II section 9 and section 12 cover Beta vegetation (survival rules, mushrooms use light < 13). None of the section 2 plants or section 3 layers are Beta behaviour, so they are this project's own and need no SPEC change. The doc does not restate Beta facts. "Flora disabled equals today" relies on the flora pass writing only into air, which `flora.rs` documents; keep it that way (the disturbance layer in 3.5 adds plants only, as the author says).
- **Invariant: determinism.** Layers 3.1 to 3.3 are safe if computed from undecorated surface height and tree leaves, but **light and forest edge read leaf blocks**, and leaves come from tree decoration, which runs in the same pass and is itself margin-padded. Flora must run after trees (it does) and read only leaves inside the margin. Cone radii and the edge distance cap must be small enough that tree-cell reach plus the cone stays inside `MARGIN_CELLS * 16`; add this as a test against the constants.
- **Invariant: `no_std`, no runtime deps.** Layers 3.1 to 3.3 and the niche format fit in `crates/terrain` (arrays and `alloc`, `noise.rs`). Per-island hydrology (3.4) needs a priority queue and island bounds; it belongs in `crates/layout` (std allowed), as the author says. Terrain only receives a raster through the new hook.
- **3.4 hydrology: determinism holds only inside layout.** The viewer and any caller of `generate_decorated` without layout fall back to 3.3, so the same seed gives different flora with and without a layout. This is acceptable (as with other `Biases`) but must be stated in the PR, and the fallback must be the code path the terrain tests exercise. The island bounding box must come from the level, never from the requested region (the author says this; the test should compare two requested regions over one island).
- **3.4 hydrology: size.** A raster per island must reach the browser client. Report wasm size and the raster's memory for the largest island (65k cells at 256x256, 2 bytes per field is about 130 KB per field). Use `u8` or `u16` quantised fields, not `f32`.
- **Layout cost.** `docs/design/world-layout.md` already generates island terrain for validation (see `validation_decorated.rs`); reuse that cache rather than generating it again. [manual check] "layout can cache it" must be confirmed against `cache.rs` in Phase E.
- **Human presence (3.5)** depends on `villages-and-villagers.md`, which is accepted but whose structures are not all built. Phase F must take it from whatever village data exists then, and ship without it if not.
- **Overlap: `shaded-grass-under-trees.md` (accepted, #1610).** It defines a client-side shade tint from the fraction of nearby columns with leaves above. The doc's `light` layer is a generator-side measure of the same idea. They should share one definition of "fraction of open sky in a small cone" if both are built; otherwise they will disagree. Suggest the generator's `light` is defined first (Phase A) and #1610 reuses its radius from `data/flora.toml`.
- **Overlap: `sea-flora-and-shellfish.md` (accepted, #1529).** The author already defers bladderwrack and eelgrass there. `vass` (reed) and `fackelblomster` at the water's edge sit on the boundary; they are land-side plants here, placed on ground next to water, not on water blocks (`pad` shapes).
- **Overlap: `picking-plants.md` (done) and `cooking-stations.md`.** Berries and mushrooms need an item and a rule (open question 1). `data/items.toml` has `[forage]` and `[[pickable]]`; `data/interactions.toml` maps tall grass to `forage`. Any pickable/forage addition is a game-side change in `crates/game`, not in this doc's generator scope. The planner should plan decorative-only plants first and the rule as a separate phase after the question is answered.
- **Overlap: `biome-flora.md` (done).** Its D.4 (#1071) decision (new flora decorative, not pickable) holds until open question 1 is answered. `requires` stays valid (backward compatible).
- **Art.** Each plant is new 8x8 art per `art/PROMPT.md`, with `validate.py` run from `art/`. About 40 plants is a lot of art; the first plants (Phase D) should be a handful, with the rest following in later phases or follow-up tickets. Each PR is limited to 200 reviewable lines.
- **Tint.** Existing tints are `none | grass | leaf | dry`. Lingon, blåbär and heather would reuse `leaf` or `dry`; a new tint is a viewer change and not required.
- **Per-column cost.** Layers add per-column work to the flora pass. Compute each layer lazily, only when some entry's niche uses it, and cache per column. Acceptance is measurable: `stats` timing before and after, and wasm gzip size, in each PR. [manual check] "plausible landscape" claims (bogs in hollows, sun-facing slopes) are visual; the measurable version is a test that places a plant on a hand-built terrain where the niche holds and not where it fails.
- **Season texture swap (question 2)** is client-side and needs no generator change.

---

# Mince Raft: Swedish summer flora and environmental niches

Status: idea
Scope: generator (and layout for the hydrology in 3.4)

Flora today is placed by biome, ground block and patch noise, plus three yes/no conditions (`canopy`, `coast`, `near_water`). That is enough for a few plants per biome but not for a landscape where every plant sits where it would actually grow. This idea does two things: adds a set of plants from the Swedish summer (wild strawberries, bilberries, cloudberries, midsummer meadow flowers, archipelago shore plants), and gives the flora pass continuous environmental layers (light, wetness, slope, exposure and so on) so each plant can describe its niche instead of a biome list alone. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `data/flora.toml` has 22 entries: daisy, bluebell, tall_grass_double, fern, fern_double, berry_bush, dry_grass, dry_grass_tall, leaf_litter, cattail, lily_pad, marsh_orchid, bush, jungle_bush, large_fern, broadleaf, cactus_flower, frost_grass, three tulips, dune_grass. Shapes: `cross`, `tall_cross`, `ground_cover`, `pad`, `bush`.
- `crates/terrain/src/flora.rs` `place` is column-local apart from `holds`, which reads up to `FLORA_COAST_RADIUS` / `FLORA_NEAR_WATER_RADIUS` columns around. That works because decoration runs inside `MARGIN_CELLS = 4` (64 blocks) of padding (`decorate.rs`).
- `requires` is a single enum (`Canopy`, `Coast`, `NearWater`) per entry.
- `crates/layout/src/derived.rs` already computes `shore_distance`, padded by `SHORE_PAD` so it does not depend on region origin. That padding pattern is the model for anything wider than the decoration margin.
- Layout feeds terrain through `Biases` (`generate_region_with_biases`); a new per-column field from layout would use the same hook.
- biome-flora D.4 (#1071): new flora is decorative, not pickable. picking-plants gives flowers a `pick` rule; berries here want a `forage`-style rule (see open question 1).
- Several existing plants are already Swedish under generic names: `daisy` is prästkrage (oxeye daisy), `bluebell` is blåklocka (harebell), `cattail` is kaveldun. `berry_bush` could be split into the real berries below.

## 2. Plants

Grouped by when they show in a Swedish summer. There is no season clock in the game (see open question 2), so for now all of them are present all year. Shapes are existing ones. The "Niche" column uses the layers in section 3.

### 2.1 Försommar (June, up to midsummer)

| Plant | English | Biomes | Shape | Niche |
|---|---|---|---|---|
| hundkex | cow parsley | Plains, Forest | tall_cross | forest edge, human presence, rich soil |
| lupin | garden lupine | Plains | tall_cross | open, human presence; colour-noise drifts like the tulips |
| midsommarblomster | wood cranesbill | Forest, Plains | cross | partial shade, moist |
| smörblomma | buttercup | Plains | cross | open, moist meadow |
| strandtrift | sea thrift | any coast | cross | on stone, low above sea, high exposure |
| fetknopp | biting stonecrop | Plains, Shrubland, coast | ground_cover | thin soil or bare stone, dry, sunny |
| linnea | twinflower | Taiga | ground_cover | deep conifer shade, north-facing, moist |

### 2.2 Högsommar (July)

| Plant | English | Biomes | Shape | Niche |
|---|---|---|---|---|
| smultron | wild strawberry | Forest, Plains, Seasonal Forest | cross | forest edge, sun-facing slope, dry to medium |
| timotej | timothy grass | Plains | tall_cross | open meadow |
| blåbär | bilberry | Taiga, Forest | ground_cover | partial shade under conifers, medium wet |
| hallon | wild raspberry | Forest, Taiga | bush | clearings and disturbance |
| hjortron | cloudberry | Swampland, Tundra, Taiga | cross | bog (filled depression), open |
| mjölkört | fireweed | Taiga, Forest | tall_cross | clearings, disturbance (old burns) |
| johannesört | St John's wort | Plains, Shrubland | cross | open, dry, sunny |
| älgört | meadowsweet | Swampland, Forest | tall_cross | wet, seeps, sheltered |
| fackelblomster | purple loosestrife | any, by water | tall_cross | next to water, sheltered |
| vass | common reed | any coast or lake | tall_cross | water's edge, sheltered coves |
| fläder | elder | Seasonal Forest, Plains | bush | human presence, rich soil |
| brännässla | stinging nettle | Forest, Plains | cross | human presence, rich soil, clearings |
| kantarell | chanterelle | Forest, Taiga | cross | under canopy, medium wet |

### 2.3 Sensommar (August into September)

| Plant | English | Biomes | Shape | Niche |
|---|---|---|---|---|
| lingon | lingonberry | Taiga, Shrubland | ground_cover | dry conifer heath, thin soil; evergreen, so stays under snow like `frost_grass` |
| ljung | heather | Shrubland, coast | ground_cover or bush | dry, open, thin soil, exposed |
| kråkbär | crowberry | Tundra, coast | ground_cover | stone or thin soil, exposed |
| tranbär | cranberry | Swampland | ground_cover | bog |
| åkerbär | arctic bramble | Taiga, Tundra | cross | moist, open; rare |
| havtorn | sea buckthorn | coast | bush | exposed shore |
| nypon | rosehip | coast, Shrubland | bush | shore, sand |
| renlav | reindeer lichen | Taiga, Tundra | ground_cover | dry, thin soil, open; stays under snow |
| vitmossa | sphagnum | Swampland | ground_cover | bog; the ground hjortron and tranbär sit in |

A spring set for later, if seasons arrive: vitsippa (wood anemone carpets), blåsippa (hepatica), gullviva (cowslip), ramslök (wild garlic), kabbleka (marsh marigold), liljekonvalj (lily of the valley). Blåstång (bladderwrack) and ålgräs (eelgrass) belong to the underwater flora doc (#1466).

All art is new 8x8 palette art per `art/PROMPT.md`.

> **Reconciler:** The underwater flora doc is `sea-flora-and-shellfish.md` (#1529); #1466 is not in the repo. Other Beta-facing reuse: none of these plants are Beta plants, so none inherit SPEC section 12 survival rules.

## 3. Environmental layers

Every layer is a function of the seed and the world column only, never of region origin or size. They are grouped by how far they read, which decides where they can be computed.

### 3.1 Column only

- **Height above sea.** Shore zonation: bare rock and lichen just above the water, then crowberry and thrift, then heath.
- **Soil depth.** Dirt blocks before stone. Thin soil over rock (hällmark) for stonecrop, lichen, crowberry, heather; deep soil for meadow and nettle.
- **Light.** Replaces the yes/no `canopy`: the fraction of open sky, from leaf blocks above the column and in a small cone around it. Full sun, partial shade and deep shade become ranges.

### 3.2 Neighbourhood (inside the existing decoration margin)

- **Slope**, from central differences on the surface height.
- **Sun-facing (aspect).** How much the slope faces a fixed noon-sun direction from data. Warm slopes for wild strawberry and St John's wort, cool slopes for twinflower and ferns.
- **Hollow or ridge (TPI).** Column height minus the mean height of rings at, say, 4, 8 and 16 blocks. Hollows collect water, cold air and litter; ridges are dry and exposed.
- **Forest edge.** Distance to the nearest leaf column, only where the column itself is open. Wild strawberry, raspberry and cow parsley live in that band.
- **Exposure.** A ray from the column in a prevailing wind direction (data) counting water columns before land, capped (say 48), combined with height above sea and `shore_distance`. Exposed shores for thrift, sea buckthorn, crowberry; sheltered coves for reed and meadowsweet.
- **Neighbouring tree kind.** Conifer leaves around mean acidic ground (lingon, bilberry, heather); birch or oak mean richer ground (cow parsley, nettle).

> **Reconciler:** `shore_distance` is a layout product (`crates/layout/src/derived.rs`), not available inside `crates/terrain`. Exposure in terrain must use its own water-ray count; combining with `shore_distance` means passing it in through the same hook as the hydrology raster (Phase E), or leaving it out until then.

### 3.3 Wetness, windowed proxy (in `crates/terrain`)

A wetness index from local data: low slope plus multi-scale hollowness, nudged by the biome humidity and by distance to existing water. Fits in the margin and gives plausible bogs in hollows and dry knolls. It cannot make continuous drainage lines.

### 3.4 Wetness, per-island hydrology (in `crates/layout`)

True wetness depends on upstream area, which is unbounded, so a per-chunk computation would break determinism. Islands are a natural closed domain: every catchment drains to the sea. Per island in the level:

1. Take its bounding box plus padding. The box comes from the layout, not from the requested region, so the result is the same for any region.
2. Build the undecorated heightmap (trees don't move water).
3. Fill depressions (priority flood). Fill depth marks ponds and bogs.
4. Route flow (D8, or D∞ for smoother output) and accumulate upstream area.
5. Output a per-column raster: upstream area, fill depth, wetness index.

The raster reaches the flora pass through a new hook alongside `Biases`. Columns on land outside any layout island fall back to 3.3. A 256x256 island is 65k cells; layout already generates island terrain for validation and can cache it.

This gives the wetland plants real places: bogs in filled depressions (sphagnum, cranberry, cloudberry), seeps along high-accumulation lines on gentle slopes (meadowsweet), dry knolls with tiny catchments (heather, lingon). Carving actual streams would change terrain blocks and is out of scope here; the raster could later also feed fish habitats and crop growth.

### 3.5 Other layers

- **Disturbance.** Low-frequency noise for old burns and storm-felled patches: fireweed and raspberry. Adds plants only; it removes no trees, so Beta placement is unchanged.
- **Human presence.** Distance to village structures (`villages-and-villagers.md`) and landing spots: nettle, lupin, cow parsley, elder.
- **Frost pockets.** The hollow measure read as cold-air pooling: hardy plants in hollows, tender ones on slopes.

## 4. Niches in data

Each `flora.toml` entry gains an optional `niche` table of preferred ranges per layer. Each range is a trapezoid response (1 inside, falling to 0 over a soft margin from data), and the product multiplies the entry's biome density. Patch and colour noise still decide drifts and winners.

```toml
[[flora]]
name = "smultron"
biomes = { Forest = 0.03, Plains = 0.02 }
niche = { light = [0.5, 0.9], edge = [1, 6], sun_facing = [0.3, 1.0], wetness = [0.1, 0.5] }
```

Existing `requires` conditions could become niche ranges (`canopy` as low `light`, `near_water` as small distance to water) or stay as they are.

> **Reconciler:** `build.rs` validates flora entries (unknown block, biome and `requires` values are rejected; see the validator tests in `flora.rs`). The `niche` table needs its layer names validated there, and a four-value form (`[lo, hi]` plus a per-layer margin from `[settings]`) so margins stay data. Keep `requires` as is; converting `canopy` would change `jungle_bush` output and is not needed.

## 5. Constraints

- World output changes: air becomes new plant blocks. `stats` before and after per PR; "flora disabled equals today" still holds.
- Every layer gets the overlapping-regions determinism test.
- `crates/terrain` stays `no_std` + `alloc`; noise and hashing use `noise.rs`. Layer constants (radii, wind direction, sun direction, response margins) live in `data/*.toml`.
- Report wasm size; layers add per-column work to the flora pass, so also report `stats` timing.

## 6. Suggested order

1. Column layers, slope, sun-facing, hollow, and the `niche` format.
2. Forest edge and exposure.
3. Windowed wetness (3.3).
4. First plants from section 2 that need only 1 to 3.
5. Per-island hydrology (3.4) and the wetland plants.
6. Disturbance, human presence, and the rest of the plants.

## Phases (reconciler)

Follows the author's order in section 6. Each phase is one or more PRs of at most 200 reviewable lines, with `stats`, wasm size and a determinism test.

**Phase A.** Column layers and the niche format (section 6 step 1; sections 3.1, slope, sun-facing and hollow of 3.2, and 4). Add the optional `niche` table to `flora.toml` and `build.rs` validation, the trapezoid response, height above sea, soil depth, light (cone), slope, sun-facing and hollow, with their constants in `data/flora.toml`. Layers are computed only for entries that use them. Done when a hand-built-terrain test shows a plant placed where its niche holds and not where it fails, overlapping regions agree, and no entry using no niche changes output.

**Phase B.** Forest edge, exposure and neighbouring tree kind (step 2; rest of 3.2). Done when each layer has a unit test on hand-built terrain and the determinism test, and the margin test shows the reads stay inside `MARGIN_CELLS * 16`.

**Phase C.** Windowed wetness (step 3; 3.3). Done when a hollow with low slope scores wetter than a ridge in a test, and the wetness layer is usable in `niche`.

**Phase D.** First plants that need only layers A to C (step 4): from section 2 the ones whose niche uses light, height above sea, soil depth, slope, aspect, hollow, edge, exposure or windowed wetness (for example smultron, blåbär, lingon, ljung, kråkbär, fetknopp, strandtrift, linnea, johannesört, smörblomma, midsommarblomster), with art. Split into several tickets by group so each fits 200 lines; includes the `berry_bush` split. Done when each plant has art that passes `validate.py`, appears in `stats` in its intended biome and niche, and decorative-only status is stated (open question 1 is not required).

**Phase E.** Per-island hydrology (step 5; 3.4), then the wetland plants (vitmossa, tranbär, hjortron, älgört, fackelblomster, vass). Done when the raster is identical for two different requested regions over one island, the flora hook takes it and falls back to 3.3 off layout, and the wetland plants appear in filled depressions in a test. Report raster size and layout timing.

**Phase F.** Disturbance, human presence and the remaining plants (step 6; 3.5): fireweed, raspberry, hundkex, lupin, fläder, brännässla, kantarell, åkerbär, havtorn, nypon, timotej, renlav. Human presence uses the village data that exists then and is skipped otherwise. Done when the layers have determinism tests and each plant has art and appears in `stats`.

## Open questions

1. **Berries and mushrooms.** Decorative only (as #1071), a `forage` rule that leaves the plant and gives an item on a cooldown (like tall grass), or `pick` like flowers? Foraged berries could be cooking ingredients (lingonsylt, blåbärssoppa, fläderblomssaft).
   - A. Decorative only (planned as written; no game change).
   - B. `forage` rule (uses `[forage]` in `data/items.toml` and a rule in `data/interactions.toml`; a game-side phase after F).
   - C. `pick` (as flowers; the plant is removed and does not regrow without a seed loop).
2. **Seasons.** No season clock exists. Leave all plants year-round, or add a client-side day-of-year texture swap (flower, berry, bare) that changes looks without changing world output?
   - A. Year-round (planned as written).
   - B. Client texture swap, as a separate design doc.
3. **Stone ground near water.** `coast` checks sand only, but the archipelago look is bare granite. Extend it, or let exposure plus height above sea cover thrift and crowberry on stone?
   - A. Let exposure plus height above sea cover it (no change to `coast`; `ground` for these entries lists stone).
   - B. Extend `coast` to accept stone (changes `Coast` for existing entries such as `dune_grass`, and so world output).
4. **One doc or two?** The layers (section 3 and 4) and the plants (section 2) could be separate docs, the plants depending on the layers.
   - A. One doc with phases (as reconciled here; layers in A to C and E, plants in D to F).
   - B. Split into a layers doc and a plants doc.
5. **(Reconciler) Shared light definition with `shaded-grass-under-trees.md`.** Should the client tint and the generator's `light` layer use one definition (cone radius and weighting from `data/`)?
   - A. Yes, define it in Phase A and have #1610 reuse it.
   - B. No, keep them separate (they may disagree on edge cases).
6. **(Reconciler) Fallback outside layout.** Flora differs with and without layout hydrology. Accept that, or keep hydrology-dependent plants out of non-layout callers (the viewer)?
   - A. Accept; the fallback is 3.3.
   - B. Gate hydrology plants behind the raster; they simply do not appear without it.
