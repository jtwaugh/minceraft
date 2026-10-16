---
title: World layout
status: accepted
scope: both
source: "#10"
superseded_by: ""
phases:
  A: done
  B: done
  C: done
  D: todo
  E: todo
  F: todo
  G: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **Phase A is done** (plan #17): base-height hook (#33), humidity hook (#34), `crates/layout` with `RadialBaseHeight` (#35), `Exclusion` as a `ProtectedMask` (#37), `ClimateBias` (#39), `Layout` and the `generate_region` wrapper (#40), `Channel` (#45, with #31, #27 and #43). The bullets below predate it.
- **Phase B is done** (plan #47): raft blocks (#57), `Level` and the level file loader (#56), structure templates and `raft_start` (#69), stamping and the raft mask (#73), `build_layout` (#74), validation config and island checks (#70), raft footprint and flat grass checks (#78), retry loop and the v0 level file (#80). The game-side criteria (loading the level, spawning on the deck, the sim building its raft entity) wait for the game crates; the layout crate returns the level and raft mask.
- **Phase C is done** (plan #89): derived water maps (#118), shore distance (#122), habitat rules and classifier (#120), habitat map per region (#125), sampling across seeds (#133), habitat viewer mode (#129) and `species_for` with the `fish.toml` loader (#130). Tests in `crates/layout/src/habitat_map_tests.rs` and `fish_tests.rs` cover the three criteria; `data/fish.toml` ships no species, as decided in #93.
- **Generator entry points.** `crates/terrain/src/lib.rs` has
  `generate_region(seed, x0, z0, w, d)`, `generate_decorated`,
  `generate_region_with_biases(.., &Biases, decorate)` and
  `generate_decorated_with_stats`. Regions are any `w x d` at any origin, not a
  fixed 128 x 128. Blocks are `u8`, indexed `(z * w + x) * HEIGHT + y`,
  `HEIGHT = 128`.
- **Sea level.** `SEA_LEVEL = 64` in one constant (`lib.rs:47`); a non-solid
  block with `y < SEA_LEVEL` is water. This matches the doc's convention.
- **Density bias hook (partly Phase A).** `DensityBias` trait
  (`f(x, y, z) -> f64`, any closure works), summed and added to the
  interpolated density before the `rho > 0` test (SPEC 6.3). Passed in
  `Biases.density`. An empty list is the default, so the "no biases means
  identical output" criterion already holds for the current entry points.
- **Temperature bias hook (part of Phase A).** `TemperatureBias`
  (`climate.rs`), added to T and clamped before density, biome lookup, sea ice,
  decoration and snow read it (`Climate::at`). There is **no humidity bias**.
- **Protected cells (close to `Exclusion`).** `ProtectedMask` and `ColumnMask`
  (`decorate.rs`, `Biases.protected`): cells decoration must not modify. It is
  a mask, not a circle or corridor, and terrain ignores it.
- **Climate and biome exports.** `Generator::climate/biome`, `climate_at`,
  `biome_at`; wasm `biome_map`, `temperature_map`, `humidity_map`,
  `heightmap`, `coarse_heightmap`, `legend` (`crates/web/src/lib.rs`).
- **Seamless decoration (most of Phase D's decoration part).** Decoration runs
  in world-aligned 16 x 16 cells over region plus margin, with a per-cell RNG
  from `(seed, cell x, cell z)`, then crops (SPEC 15, decided in #3). Output is
  independent of region origin and size. Tests: `tests.rs`, `tests_veg.rs`.
- **Sea ice.** Water at `y = SEA_LEVEL - 1` becomes ice when `t < 0.5`
  (`lib.rs`, SPEC 13). This is the only "frozen" signal available for habitats
  and navigability.
- **Data files.** `data/biomes.toml` and `data/blocks.toml`, read at compile
  time by `crates/terrain/build.rs`. Blocks include `ice` and the tree and
  plant blocks.
- **Viewer.** `viewer/` has map tiles, a 3D view and the flyover
  (`viewer/flyover/`), which plans a camera route over a survey of the map.
  It is a tour planner, not a gameplay route.

Not present: `Layout`, `Level`, `ClimateBias`/`Exclusion` types, a
layout builder, structures, raft blocks (`raft_planks`, `butcher_block`,
`griddle`, `crate`), raft mask, derived maps (water depth, shore distance,
habitat), `data/habitats.toml`, `validation.toml`, `scoring.toml`,
`fish.toml`, navigability, pathfinding, the seed search tool, the level file,
and any game, sim or streaming code.

## Reconciliation notes

1. **Base-height bias has no hook.** The doc's `RadialBaseHeight` and
   `Channel` change "base height before the vertical falloff in the density
   formula". The code and SPEC 6.3 only offer an additive term on the final
   density. Options: express islands and channels as additive density terms
   (fits today's code, but not the doc's wording or its "keeps Beta overhangs"
   claim), or add a per-column base-height hook. The second needs a SPEC 6.3
   change, which only the maintainer can make. Decided: add the hook
   (Decisions, item 1).
2. **Climate bias needs humidity.** `ClimateBias` carries `d_humid`; only
   temperature has a hook. Adding it is a small code change in `climate.rs`,
   but SPEC 6.3 and 7 mention only temperature biases. Decided: add it
   (Decisions, item 2).
3. **`Exclusion` versus `ProtectedMask`.** `ProtectedMask` already covers
   "decoration skips these cells". `Exclusion::Circle/Corridor` can be built as
   a `ProtectedMask` implementation. Suggest implementing it that way, not as
   a second mechanism.
4. **Region size.** The doc fixes regions at 128 x 128. The generator takes any
   size. Suggest keeping the generator unchanged and making 128 x 128 a
   convention of the layer above it (and of the game).
5. **Phase D's decoration scheme conflicts with the shipped one.** The doc
   proposes `hash(seed, x, z, kind)` candidates and a margin of 8. The repo
   already has a position-deterministic scheme (16 x 16 cells, per-cell RNG,
   margin) decided in #3 and described in SPEC 15, and it satisfies the doc's
   "any order gives the same blocks" criterion. Suggest treating that part of
   Phase D as done and not replacing it; replacing it would change world
   output for every seed. Decided: keep it (Decisions, item 6).
6. **Beta facts to check.** The doc says the density formula reads climate.
   SPEC 3 and 7 agree (SPEC 7: one shared climate source). The doc's
   `deep_water` habitat assumes Beta seabeds are shallow; that is a
   measurement, not a SPEC fact, and should come from `stats` output, not
   from the doc.
7. **CLAUDE.md invariants.**
   - `crates/terrain` must stay `no_std`, no runtime dependencies, and its
     data is compile-time TOML. The layout and level file (runtime TOML,
     serialization), validation, A* pathfinding, the layout builder, retry
     loop and the seed search tool (threads, files, CLI) all need `std` or
     dependencies. Suggest a new crate (for example `crates/layout`) that
     depends on `terrain`, with the CLI as a native binary in it. `terrain`
     keeps only the hooks (bias traits, structure stamping if it needs block
     ids).
   - `habitats.toml`, `validation.toml`, `scoring.toml` and
     `structures/*.toml` belong in `data/`. Only the crate that reads them
     decides compile-time versus runtime; tunables read at compile time by
     `terrain` would follow the `build.rs` pattern. The level file is runtime
     input by design.
   - Wasm size: anything the viewer needs (habitat map, navigability map,
     bias heatmap) must be weighed against the gzipped size. Keep layout
     logic out of the wasm unless the viewer needs it, and report the size
     in each PR.
   - Determinism: the "retry loop increments the seed" is fine as long as it
     lives above `terrain`. Each new bias needs the overlapping-regions test.
8. **New blocks and art.** The raft needs four new blocks (`raft_planks`,
   `butcher_block`, `griddle`, `crate`). They are not in `data/blocks.toml`
   and need textures in `art/` (this project's own, per CLAUDE.md). Tag the
   station blocks `[game]` if they only mean something to the sim; the ids and
   textures are still needed for the viewer.
9. **Overlap with the flyover.** The flyover (`viewer/flyover/plan.js`)
   already picks an island and a route for a camera. It does not conflict, but
   Phase E's "route" is a different thing (anchorages and channels). Keep the
   names distinct.
10. **Fish.** `data/fish.toml` and species spawn rules are not in SPEC (SPEC 14
    has a `squid` water spawn only). The fish spawner is game logic; the
    habitat classification is generator-side. Spawner and fish data are
    `[game]`.
11. **Scope split.** In this repo: Phase A, the layout crate and builder, the
    structure template and stamping (as data and block placement), raft mask
    output, validation, derived maps and habitat classification, navigability
    and pathfinding, route builder, seed search tool, the climate `AlongPath`
    bias, viewer debug modes. `[game]`: spawning the player on the deck, the
    raft entity and its travel, loading a level file in the game, the region
    cache and streaming, sim handling of region borders, the weekend screen,
    the fish spawner.
12. **Phase status.** No phase is done. Phase A is partly built (hooks above),
    and the decoration half of Phase D is done, so both stay `todo`.

## Mince Raft: World Layout Specification

> **Reconciler:** the text below is the author's, unchanged. Notes beside it
> are marked like this one.

Status: draft
Scope: the layer that sits between the Beta-style terrain generator and the game. It decides where things go (spawn, islands, raft, channels, climate shifts) and guarantees that every generated level is playable.

This document assumes the terrain crate already produces Beta 1.7.3 style terrain (density field, water fill, surface pass) and, once the biome work lands, climate, biomes, and decoration. Everything here is additive: it steers that generator rather than replacing it.

> **Reconciler:** the biome and decoration work has landed (SPEC Part II;
> `climate.rs`, `decorate.rs`). "Additive" matches the existing `Biases` hooks.

---

## 1. Concepts

**Layout.** A plain data description of one world: seed, levels, and the bias terms that shape terrain and climate. Layouts are serializable so the seed search tool can write them and the game can load them.

**Bias term.** A data record that nudges generation in a region. Density biases change where land and water are. Climate biases change temperature and humidity, which in turn change biomes and fish habitats. Exclusion zones keep decoration out of specific areas.

**Level.** One in-game week. Each level has an anchorage (where the raft sits), an island, and a spawn water zone.

**Raft mask.** The set of block cells that belong to the raft. Generation produces it, the sim consumes it.

**Habitat.** A per-column classification of water (cold open water, warm shallows, and so on) used to decide which fish spawn.

**Region.** The unit of generation and streaming. A pure function of `(seed, layout, region coordinates)`.

> **Reconciler:** "serializable" implies a serialization dependency, which
> `terrain` may not have (note 7). Layout types belong in a `std` crate.

---

## 2. Conventions

| Name | Default | Notes |
|---|---|---|
| World height | 128 | Beta height |
| `SEA_LEVEL` | 64 | Water fills every air cell with `y < SEA_LEVEL`. The raft deck sits at `y = SEA_LEVEL`. Adjust if the existing skeleton uses a different convention, but keep this definition in one constant. |
| Region size | 128 x 128 columns | 8 x 8 chunks. Full height. |
| Chunk size | 16 x 16 | Used only for compatibility with the Beta spec |
| Coordinates | `x` east, `z` south, `y` up | Integer block coordinates; bias math in `f64` |

> **Reconciler:** `SEA_LEVEL = 64`, `y < 64` is water, and `HEIGHT = 128`
> already match, defined once in `lib.rs`. Region size is not fixed in the
> generator (note 4). The doc's `x` east, `z` south, `y` up should be checked
> against the viewer's axes before the layout crate relies on it.

Helper functions used throughout:

```
smoothstep(e0, e1, x):
    t = clamp((x - e0) / (e1 - e0), 0, 1)
    return t * t * (3 - 2 * t)

falloff(d, r_inner, r_outer) = smoothstep(r_outer, r_inner, d)
    // 1.0 inside r_inner, 0.0 beyond r_outer, smooth between
```

---

## 3. Generation pipeline

Per region, in this order:

1. **Climate.** Compute temperature and humidity per column from Beta's climate noise, then apply climate bias terms. Derive the biome per column from the biased climate.
2. **Density.** Compute the Beta density field. Apply density bias terms (base height shifts and density clamps). Threshold to solid or air.
3. **Water fill.** Air below `SEA_LEVEL` becomes water.
4. **Surface pass.** Grass, dirt, sand, gravel, snow, ice per the Beta rules and biome.
5. **Decoration.** Trees and plants, skipping any cell inside an exclusion zone.
6. **Structure stamping.** Place the starting raft (and later, any other fixed structures). Record the raft mask.
7. **Derived maps.** Heightmap, water depth, bottom block, shore distance, habitat.
8. **Output.** A `Region` containing blocks, raft mask cells, derived maps, and metadata (seed, layout hash, region coordinates).

Climate must come first because the Beta density formula reads climate. Biases on climate therefore shift terrain shape slightly as well as biomes. That is expected.

> **Reconciler:** steps 1 to 5 exist in this order in `Generator` and
> `decorate.rs` (temperature bias applied inside `Climate::at`). Stamping
> after decoration means the stamp can overwrite trees; the existing
> `ProtectedMask` protects the raft footprint from decoration, so the order
> works as long as the mask covers the footprint. Steps 6 to 8 do not exist.

---

## 4. Data model

### 4.1 Bias terms

All bias terms are tagged records. Suggested Rust shape:

```rust
enum DensityBias {
    // Raises or lowers the base height around a point.
    // Preferred for islands and spawn water because it keeps Beta overhangs.
    RadialBaseHeight { center: [f64; 2], r_inner: f64, r_outer: f64, delta: f64 },

    // Carves a navigable channel along a polyline.
    Channel {
        points: Vec<[f64; 2]>,
        half_width: f64,       // fully cleared inside this distance
        falloff: f64,          // extra distance over which the effect fades
        depth_below_sea: f64,  // target seabed depth inside the channel
        clear_above: i32,      // blocks of guaranteed air above the water
    },
}

enum ClimateBias {
    Radial { center: [f64; 2], r_inner: f64, r_outer: f64, d_temp: f64, d_humid: f64 },
    // Linear gradient along a polyline, interpolated by arc length.
    AlongPath { points: Vec<[f64; 2]>, d_temp_start: f64, d_temp_end: f64,
                d_humid_start: f64, d_humid_end: f64, half_width: f64, falloff: f64 },
}

enum Exclusion {
    Circle { center: [f64; 2], radius: f64 },
    Corridor { points: Vec<[f64; 2]>, half_width: f64 },
}
```

> **Reconciler:** `DensityBias` is already a trait in `terrain`; these enums
> would live in the layout crate and implement it (and `TemperatureBias`,
> `ProtectedMask`). The name `DensityBias` is taken; pick another for the enum
> (for example `DensityTerm`). See notes 1 to 3.

How each density bias applies:

- `RadialBaseHeight`: before the vertical falloff in the density formula, `base_height += delta * falloff(d, r_inner, r_outer)`. Positive `delta` raises land, negative pushes it below sea level.
- `Channel`: let `d` be the distance from the column to the nearest point on the polyline and `w = falloff(d, half_width, half_width + falloff)`.
  - Lower the base height so the seabed sits about `depth_below_sea` below `SEA_LEVEL`, scaled by `w`. The term is target-relative: the base-height hook receives the column's unbiased base height as well, `f(x, z, base)` in blocks, and the term is `(target - base) * w` with `target = SEA_LEVEL - depth_below_sea`. This changes the hook's signature from Decision 1, which gave `f(x, z)`. (Decided in #19.)
  - For `target <= y < SEA_LEVEL + clear_above`, with `target` as above, clamp density to `min(density, lerp(density, -1.0, w))`. This carves the channel down to the seabed and removes overhangs above the waterline inside it, so the seabed and the water path hold on any terrain. The base-height term stays as decided and smooths the carve. (Decided in #42.)

> **Reconciler:** the second `Channel` bullet is a function of final density
> and `y`, and the existing additive hook does not receive the density value.
> It uses a new hook, `DensityClamp`, which receives `(x, y, z, density)` and
> returns the new density, applied just before the `rho > 0` test. The first
> bullet and `RadialBaseHeight` need the base-height hook (note 1).
> Beyond the density lattice, the lattice is 4 blocks horizontally (SPEC
> 3.1), so a channel narrower than a few blocks will alias; the default
> `half_width = 6` is fine. (Decided in #18.)

Climate biases add `d_temp` and `d_humid`, scaled by their falloff, then clamp to the climate's valid range.

### 4.2 Layout

```rust
struct Layout {
    seed: u64,
    density_biases: Vec<DensityBias>,
    climate_biases: Vec<ClimateBias>,
    exclusions: Vec<Exclusion>,
    structures: Vec<StructurePlacement>,
    levels: Vec<Level>,
}

struct Level {
    id: String,
    week: u32,
    anchorage: [i32; 2],      // where the raft sits
    island_center: [i32; 2],
    spawn_water_radius: f64,
}

struct StructurePlacement {
    structure: String,        // e.g. "raft_start"
    origin: [i32; 3],
    rotation: u8,             // 0..3 quarter turns
}
```

A `Level` is metadata for the game. The bias terms that actually shape the world are generated from levels by a layout builder (section 5), so there is one source of truth for terrain shaping.

### 4.3 Level file

Written by the seed search tool, loaded by the game. Example:

```toml
[world]
seed = 8812734419
sea_level = 64

[[level]]
id = "week1"
week = 1
anchorage = [0, 0]
island_center = [28, 6]
spawn_water_radius = 8.0

[[level]]
id = "week2"
week = 2
anchorage = [162, 41]
island_center = [180, 22]
spawn_water_radius = 8.0

[route]
points = [[0, 0], [80, 25], [162, 41]]

[climate]
temp_start = -0.25
temp_end = 0.15
```

> **Reconciler:** `sea_level` in the file duplicates the compile-time
> `SEA_LEVEL`. Suggest the loader reject a mismatch instead of honouring it.
> `temp_start = -0.25` is a temperature offset; check it against the T range
> (clamped to [0, 1]) once the humidity/temperature bias is built.

### 4.4 Structures

Structures are small block templates stored as data. `data/structures/raft_start.toml`:

```toml
size = [5, 2, 5]      # x, y, z
origin_offset = [-2, 0, -2]
raft = true           # every non-air cell joins the raft mask
palette = { P = "raft_planks", B = "butcher_block", G = "griddle", C = "crate", "." = "air" }
layers = [
  # y = 0 (deck)
  ["PPPPP", "PPPPP", "PPPPP", "PPPPP", "PPPPP"],
  # y = 1 (stations on deck)
  [".....", ".B.G.", ".....", "..C..", "....."],
]
```

> **Reconciler:** the four palette blocks do not exist (note 8). The deck at
> `y = SEA_LEVEL` sits one block above the top water cell
> (`SEA_LEVEL - 1`); the raft is stamped into air above the water, so it
> never needs to clear water. `rotation` is unused by the example; say whether
> `raft = true` structures may rotate (the mask would rotate with them).

### 4.5 Region output

```rust
struct Region {
    coords: [i32; 2],
    blocks: Vec<BlockId>,             // x-major, then z, then y, or match the skeleton
    raft_cells: Vec<[i32; 3]>,        // world coordinates
    heightmap: Vec<i32>,              // top solid block per column
    water_depth: Vec<u8>,             // 0 on land
    bottom_block: Vec<BlockId>,
    shore_distance: Vec<u8>,          // capped at 64
    climate: Vec<(f32, f32)>,         // temperature, humidity after biases
    biome: Vec<BiomeId>,
    habitat: Vec<HabitatId>,
    layout_hash: u64,
}
```

> **Reconciler:** `terrain::Region` already exists (`blocks`, `width`,
> `depth`, `height`, `x0`, `z0`) with `(z * w + x) * HEIGHT + y` indexing; use
> that layout. Suggest a different name here (for example `LayoutRegion`) that
> wraps it. `heightmap: Vec<i32>` differs from today's `u8` heightmap helpers
> (the height fits in a byte). A water-column `heightmap` of "top solid block"
> differs from the existing helper, which reports the top non-air block.

---

## 5. Phases

Each phase lists what it depends on, what it adds, and how to know it is done.

### Phase A: Bias framework

Depends on: terrain crate with density field and surface pass.

Adds:
- `Layout`, `DensityBias`, `ClimateBias`, `Exclusion` types.
- `generate_region(layout, region_coords)` replacing or wrapping the current entry point. Seed comes from the layout.
- Bias evaluation inside the density and climate steps.
- Exclusion checks inside decoration (a no-op until decoration exists).

> **Reconciler:** decoration exists, so the exclusion check is live, via
> `Biases.protected`. Wrap, do not replace: `generate_region` is used by the
> wasm crate, tests and `stats`.

Notes: keep bias evaluation cheap. Precompute, per region, the list of bias terms whose bounds intersect the region and skip the rest. For channels, store a bounding box per segment.

Done when:
- A layout with one `RadialBaseHeight` at positive `delta` produces land above sea level at the center for at least 95 percent of seeds tested.
- A layout with no biases produces byte-identical output to the generator before this change.
- Side views of a biased island still show overhangs and irregular coastlines.

> **Reconciler:** criterion 1 is testable (use a fixed set of, say, 200 seeds
> and `column_summary`). Criterion 2 is testable by comparing against the
> unchanged entry points; it holds today for an empty bias list. Criterion 3
> is `[manual check]`; a measurable version is "the island column set has at
> least N columns with an air gap above sea level under solid block", using the
> `gaps` count that `generate_coarse_heightmap` already reports.

### Phase B: Single-level layout (the v0 world)

Depends on: A.

Adds:
- A layout builder: `build_layout(world_config, levels) -> Layout`. For each level it emits:
  - `RadialBaseHeight` at the island center, positive delta (default `r_inner = 8`, `r_outer = 20`, delta tuned so the center sits about 6 blocks above sea level).
  - `RadialBaseHeight` at the anchorage, negative delta, so water at the anchorage is at least 3 deep within `spawn_water_radius`.
  - `Exclusion::Circle` at the anchorage covering the raft footprint plus 2 blocks.
  - A `StructurePlacement` for `raft_start` at the anchorage, deck at `SEA_LEVEL`.
- Structure stamping step and raft mask output.
- Validation (section 6.1) and a retry loop that increments the seed until a level passes, with a cap (default 64 attempts) and a clear error if the cap is hit.

Default v0 placement: anchorage at the world origin, island center 24 to 32 blocks away in a direction derived from the seed.

Done when:
- The game loads one level from a level file and the player spawns standing on the raft deck.
- The sim receives the raft mask and can build its raft entity from it.
- Validation passes on the shipped seed and on 100 random seeds after retries, with a median of 2 or fewer attempts.

> **Reconciler:** `[game]`: "the game loads one level", "the player spawns
> standing on the raft deck", "the sim receives the raft mask and can build its
> raft entity". In this repo the testable version is "the layout crate returns
> a level and a raft mask whose cells all lie at the stamped positions, and
> the mask is written into the output". The retry criterion (100 seeds,
> median at most 2 attempts) is testable but depends on tuned deltas; it
> measures the author's tuning as much as the code. "A direction derived from
> the seed" must be a hash of the seed, not a running RNG, to keep the layout
> a pure function. Island centre 6 blocks above sea level and 150 flat
> columns (section 6.1) at `r_outer = 20` (about 1250 columns) are consistent
> sizes, but both depend on the density hook (note 1).

### Phase C: Habitat layer

Depends on: A, and the climate module from the biome work.

Adds:
- Derived maps: water depth, bottom block, shore distance (a distance transform from land columns, capped at 64; the distance is Euclidean between column centres, rounded to the nearest integer), frozen surface flag. (Decided in #91.)
- Habitat classification, driven by `data/habitats.toml`. Rules are checked in order and the first match wins:

```toml
[[habitat]]
id = "frozen"
frozen = true

[[habitat]]
id = "warm_shallows"
temp_min = 0.6
depth_max = 6
bottom = ["sand"]

[[habitat]]
id = "deep_water"
depth_min = 14
shore_distance_min = 24

[[habitat]]
id = "rocky_shore"
depth_max = 8
shore_distance_max = 6
bottom = ["gravel", "stone"]

[[habitat]]
id = "cold_open"
temp_max = 0.4

[[habitat]]
id = "temperate_open"   # fallback
```

- Fish spawn query: `species_for(column) -> weighted list` filtering `data/fish.toml` by habitat, temperature, depth, and time of day.
  Time of day is left out of Phase C: the query filters by habitat,
  temperature and depth only, and time of day is added when a day/night cycle
  exists. Phase C ships the `fish.toml` schema and its loader only, with
  test-only species and no shipped species list. The query lives in
  `crates/layout`. (Decided in #93.)
- Viewer mode showing habitats.

Done when:
- Every water column gets exactly one habitat.
- Over a large sample, each habitat appears in some seed, and the warm shallows only appear in warm climates.
- The fish spawner, given a column, returns only species whose ranges contain that column.

> **Reconciler:** "frozen" can only mean the sea-ice rule that exists
> (`y = SEA_LEVEL - 1`, `t < 0.5`). `[game]`: `fish.toml`, `species_for`, time
> of day and the spawner (note 10); the generator side stops at the habitat
> map. "Warm climates" in criterion 2 needs a number; the doc's own
> `temp_min = 0.6` is the natural one. "Each habitat appears in some seed"
> is testable with `stats`-style sampling; `deep_water` may fail it (see the
> note below). Habitat rules are runtime data if the layout crate reads them,
> or compile-time if `terrain` does (note 7).

Note: Beta seabeds are not especially deep. If `deep_water` is rare, add a `RadialBaseHeight` with negative delta in open water far from islands, or a dedicated `DeepWater` density bias, rather than lowering global base height (which would also shrink islands).

If `deep_water` does not appear over a sample of seeds, lower its thresholds in `data/habitats.toml` until it makes up at least 1% of water columns over that sample; do not add a layout bias for it, and do not exempt it from the criterion above. (Decided in #94.)

The `cold_open` rule must be able to match water that is not frozen. If it does not appear over a sample of seeds, change its rule in `data/habitats.toml` (for example by raising `temp_max`) until it does, accepting that it shares the cold band with `frozen`; do not drop it and do not exempt it from the criterion above. (Decided in #128.)

### Phase D: Multi-region world

Depends on: B.

Adds:
- Region-keyed generation with a cache. Neighboring regions must match exactly at their shared edges.
- Seamless decoration across region borders (see below).
- Streaming in the renderer and sim: keep the player's region and its neighbors loaded, unload beyond a radius.
- Sim support for entities and the raft crossing region borders.

Seamless decoration. Beta decorates with a chunk offset so trees can spill into neighbors. Replace that with position-based determinism:
- Tree and plant candidates are derived from `hash(seed, x, z, kind)`, never from a running RNG.
- To decorate a region, evaluate candidates in the region plus a margin equal to the largest decoration radius (default 8 blocks). This needs terrain and surface for the margin columns too, which is cheap.
- Write only the cells that fall inside the region.

The result: any region generates identically regardless of the order in which regions are generated.

Done when:
- Generating regions in random order produces the same blocks as generating them in row order.
- A tree straddling a region border appears whole.
- Walking and rafting across borders shows no seams, pops, or collision gaps.

> **Reconciler:** the seamless-decoration half is already implemented with a
> different scheme (note 5); the "random order equals row order" and
> "straddling tree" criteria can be satisfied by tests over the existing
> function, and probably already are (`tests.rs`, `tests_veg.rs`; check
> before planning). `[game]`: the cache in the renderer/sim, streaming, and
> entities crossing borders. The last criterion is `[manual check]`; a
> measurable version is the existing overlapping-region equality test applied
> to layout-biased regions, plus a check that a layout's biases are the same
> function of world coordinates whichever region is requested.

### Phase E: Routes

Depends on: B and D.

Adds:
- Route builder: from the first anchorage, place K further anchorages. Default spacing 150 plus or minus 30 blocks, heading changing by at most 30 degrees per step, so the route bends but never doubles back.
- For each route segment, a `Channel` bias (default `half_width = 6`, `falloff = 6`, `depth_below_sea = 4`, `clear_above = 4`).
- For each anchorage, the Phase B level biases (island, spawn water, exclusion), with the island placed beside the channel, not in it.
- Navigability map (section 6.2) and pathfinding between consecutive anchorages to confirm the channel works.
- Corridor generation: only regions within a margin of the route are generated. Regions outside the corridor render as open ocean at sea level, or not at all if the camera never sees them.
- Raft travel: between levels, the raft entity follows the pathfound route. This plays during the weekend screen.

Done when:
- For 100 random seeds, every consecutive anchorage pair has a navigable path.
- No route channel cell has solid blocks within `clear_above` of the water surface.
- The raft can travel the whole route in the sim without collision.

> **Reconciler:** `[game]`: raft travel, the weekend screen, "the raft can
> travel ... in the sim", and rendering regions outside the corridor. In this
> repo the third criterion becomes "A* with the eroded navigable set finds a
> path" (already criterion 1). "Regions outside the corridor render as open
> ocean at sea level" is a new generator mode (a flat-water region) that the
> doc does not specify; it is a choice for the maintainer (open question 5).
> Heading change "at most 30 degrees" and spacing "150 plus or minus 30" make
> a route of K anchorages deterministic from the seed only if the draws use
> hashes of `(seed, step)`.

### Phase F: Seed search tool

Depends on: E.

Adds a native command-line tool sharing the terrain crate:

```
mince-seeds --levels 5 --candidates 5000 --top 20 --out levels.toml
```

Stages:
1. Prefilter. For each candidate seed, build the layout and generate a low-resolution map: density sampled every 4 blocks, only for `y` between `SEA_LEVEL - 8` and `SEA_LEVEL + 8`, only in the corridor. Reject seeds where any anchorage area is solid or any island center is underwater.
2. Full check. Generate the corridor at full resolution for survivors and run all validation (section 6).
3. Score (section 6.3) and keep the top N.
4. Write the best layout's levels and route to `levels.toml`, with scores in comments for later review.

Use all CPU cores; seeds are independent.

Done when:
- 5,000 candidates finish in a few minutes on a laptop.
- The written file loads in the game and every level validates.

> **Reconciler:** needs `std`, threads and file output, so it is a native
> binary outside `terrain` (note 7). `generate_coarse_heightmap` already does a
> cheap column survey and may serve the prefilter; a prefilter that samples
> density only for a y-band and a corridor needs a new entry point. "A few
> minutes on a laptop" is `[manual check]`; a measurable version is a
> seeds-per-second figure on `bench-native` and a budget per candidate. The
> game loading the file is `[game]`; here the criterion is "the loader in the
> layout crate round-trips the file and every level validates".

### Phase G: Biomes on the route (the deferred Phase 4)

Depends on: C, E, F, and the biome and vegetation work.

Adds:
- Validation additions (section 6.1): flat grass tiles free of decoration, and at least one tree per island.
- An `AlongPath` climate bias on the route, from `temp_start` to `temp_end`, so the trip moves from cold water to warm.
- Scoring additions: climate gradient along the route and biome variety across anchorage islands.
- Fish habitats automatically follow, since they read the biased climate.

Done when:
- Over 100 searched worlds, the first anchorage is in a cold biome and the last in a warm one in at least 90 percent.
- No two consecutive islands share the same biome in the top-scored worlds, unless the route is shorter than the climate noise scale.

> **Reconciler:** the biome and vegetation work this depends on is done, so
> Phase G's dependency on it is met. "Cold" and "warm" biome need a definition
> (for example, a list of biomes in `data/biomes.toml` terms, or T thresholds).
> "The climate noise scale" is a number from SPEC 7 (frequency 0.025 for T);
> state it as a distance. Fish habitat following is a consequence, not work.

---

## 6. Validation, navigability, and scoring

### 6.1 Level validation

Run per level after generation. All thresholds live in `data/validation.toml`.

| Check | Default | Phase |
|---|---|---|
| Island land area above sea level, flood-filled from island center | at least 150 columns | B |
| Island does not reach the anchorage spawn water zone | true | B |
| Water depth under the raft footprint | at least 3 | B |
| Raft footprint free of solid blocks from `SEA_LEVEL` to `SEA_LEVEL + 4` | true | B |
| Flat grass tiles on the island (grass top, all 4 neighbors within 1 block of height) | at least 6 | B |
| Of those, tiles with no tree, plant, or cactus | at least 6 | G |
| Trees on the island | at least 1 | G |
| Anchorage reachable from previous anchorage (section 6.2) | true | E |

> **Reconciler:** the Phase B row for flat grass counts tiles regardless of
> decoration, while the Phase G row removes decorated ones; since decoration
> already exists, the Phase B and G variants would give different answers on
> the same world. Suggest deciding whether Phase B validates before or after
> decoration (open question 4). Rows "Of those" and "Trees on the island" are
> checkable today.

### 6.2 Navigability

A column is navigable if:
- It is water at `SEA_LEVEL - 1` with depth at least 2, and
- Every cell from `SEA_LEVEL` to `SEA_LEVEL + 3` is air (Beta overhangs make this check necessary), and
- It is not frozen, unless the level explicitly allows ice.

To account for raft size, erode the navigable set by the raft's half-width (default 2) before pathfinding. Use 8-connected A* with diagonal cost of sqrt 2.

> **Reconciler:** "every cell is air" will fail on trees whose leaves hang
> over water (mangrove-like cases aside, shoreline trees); say whether leaves,
> plants and `snow_layer` count as blocking. Sea ice sits at `SEA_LEVEL - 1`,
> so "water at `SEA_LEVEL - 1`" already excludes frozen columns.

### 6.3 Seed scoring

Each term is normalized to 0..1 and weighted. Weights live in `data/scoring.toml`.

| Term | Meaning | Weight |
|---|---|---|
| Spacing regularity | How close anchorage gaps are to the target spacing | 1.0 |
| Path directness | Straight-line distance over path length, per segment | 1.0 |
| Island quality | Area and flat-tile surplus above the minimums | 1.0 |
| Coastline interest | Ratio of island perimeter to sqrt of area | 0.5 |
| Extra islands | Count of non-anchorage islands visible from the route | 0.5 |
| Climate gradient | Temperature change from first to last anchorage, and monotonicity | 1.0 (Phase G) |
| Biome variety | Distinct biomes across anchorage islands | 1.0 (Phase G) |

> **Reconciler:** "visible from the route" has no definition (line of sight,
> or distance). Suggest a distance from the route centre line. "Normalized to
> 0..1" needs a cap for each surplus term. Scoring is judged by the
> maintainer's eye; the only automated check is that scores are
> deterministic for a seed `[manual check]`.

---

## 7. Mapping onto the existing skeleton

A checklist for adapting this spec to the current code:

- Find the current generation entry point and decide whether `generate_region(layout, coords)` replaces it or wraps it. Phase A's identical-output test depends on the no-bias path matching exactly.
- Locate where base height is computed in the density formula. `RadialBaseHeight` and `Channel` hook in there, before the vertical falloff.
- Locate the climate computation. Climate biases apply right after it, before biome lookup and before density reads climate.
- Check the existing block storage order and use it for `Region.blocks`.
- Check the sea level convention (`y < 64` water or `y <= 64` water) and set `SEA_LEVEL` to match.
- Identify how the sim currently receives the world, and add the raft mask to that handoff.
- Put all tunables (`habitats.toml`, `validation.toml`, `scoring.toml`, structure files) in the existing data directory if there is one.

> **Reconciler:** answers, in order: wrap `generate_region_with_biases`
> (`lib.rs`); the per-column terms are in SPEC 3.3 and the only hook is the
> additive one (note 1); climate is `Climate::at` in `climate.rs`; storage is
> `(z * w + x) * HEIGHT + y`; `y < 64` is water and `SEA_LEVEL` already
> matches; the sim does not exist in this repo (`[game]`); the data directory
> is `data/`.

---

## 8. Testing summary

- Determinism: same layout and region coordinates give identical bytes, across runs and regardless of generation order.
- No-bias parity: an empty layout reproduces the pre-layout generator exactly.
- Validation: every check in 6.1 has a unit test with a hand-built region that passes and one that fails.
- Navigability: a hand-built region with an overhang over water is not navigable; removing the overhang makes it navigable.
- Property tests: for random seeds, generated levels either pass validation or the retry loop finds one within the cap.
- Visual: viewer modes for terrain, biome, climate, habitat, navigability, and bias influence (a heatmap of summed bias strength) make tuning fast.

> **Reconciler:** the visual bullet is `[manual check]`; the viewer modes
> themselves are repo work (viewer has no frameworks, and every new wasm export
> counts against the size budget). Determinism tests should include layout
> biases, as CLAUDE.md requires for any new feature.

---

## 9. Open questions

- Should islands be able to sit on both sides of a channel, or always on one side? One side is simpler for validation; both sides looks more natural.
- Do levels later than week 1 need their own starting structures (a dock, a second raft, a wreck), or does the raft always arrive as-is from the previous week?
- How much of the corridor should be generated ahead of time versus on demand during raft travel? Ahead of time is simpler and the corridor is small.
- Should frozen water ever block the route, forcing a detour or an ice-breaking mechanic? Interesting, but it conflicts with the no-breaking rule unless fire melts ice.
- Is the climate noise scale in Beta large enough that a five-level route reliably crosses multiple biomes, or does Phase G need a stronger climate bias than expected? Phase F's statistics will answer this.

> **Reconciler:** the author's questions are kept as written. Two are `[game]`
> in nature (second-week structures, on-demand corridor generation, and the
> ice-breaking mechanic). The last one is answerable only by running the search
> tool, so it stays open until Phase F.

---

## Decisions

The reconciler's open questions, answered by the maintainer on 2026-10-15.
Plan and build against these.

1. **Base-height hook: B, add a per-column base-height hook.** Add a
   `BaseHeightBias` term, `f(x, z) -> f64` in blocks, alongside
   `DensityBias`. The sum of all terms, divided by 8 (blocks per lattice
   level), is added to `center` in SPEC 3.3 step 6, evaluated at each
   lattice column's world block coordinates. No terms means identical
   output. `RadialBaseHeight` and the seabed part of `Channel` use this
   hook; `Channel`'s overhang clamp near the waterline uses a density clamp
   hook, `DensityClamp` (`f(x, y, z, density) -> f64`, returns the new
   density, applied just before the `rho > 0` test). This keeps the
   overhangs the doc asks for. The hooks are project extension points, not
   Beta behaviour; SPEC 6.3 only describes the density hook and can be
   extended to list these in a separate SPEC-only change. (Decided in #18.)
2. **Humidity bias: A, add a humidity hook.** Add `HumidityBias`, mirroring
   `TemperatureBias`: summed, added to H and clamped to [0, 1] in
   `Climate`, before anything reads H (density, biome lookup, decoration).
   `ClimateBias` keeps `d_humid`.
3. **Where the layer lives: A, a new `crates/layout`.** A `std` crate that
   depends on `terrain` and may have runtime dependencies (TOML parsing,
   serialization, threads). It holds the `Layout` data types, the layout
   builder, structures, validation, derived maps, navigability, the route
   builder and the seed search binary. `crates/terrain` gets only the hooks
   (items 1 and 2, and anything stamping needs) and stays `no_std` with no
   runtime dependencies. Layout code stays out of the wasm unless the
   viewer needs it.
4. **Validation timing: B, keep the split as written.** Phase B validates
   flat tiles on undecorated terrain; Phase G adds the decorated checks.
5. **Outside the corridor (Phase E): A, render nothing.** Regions outside
   the corridor are not generated or drawn. Revisit when Phase E is
   planned.
6. **Phase D decoration: keep the shipped scheme.** The 16 x 16 cell scheme
   from SPEC 15 (decided in #3) stays; the doc's per-candidate hash and
   8-block margin are not built. The decoration part of Phase D counts as
   done; the rest of Phase D (region cache, streaming, border handling) is
   still to do.
7. **Habitat viewer mode (Phase C): A, a wasm export.** The viewer's
   habitat mode gets its map from the wasm: add a `crates/layout` wasm
   export, or move the habitat classifier into a wasm-safe module, so the
   viewer shows the same habitats as the game. The PR reports the gzipped
   wasm size. This is the case where the viewer needs layout code in the
   wasm (Decision 3). (Decided in #92.)
