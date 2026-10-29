---
title: Biome colour blending, biome flora and foliage variation
status: accepted
scope: both
source: ""
superseded_by: ""
phases:
  A: done
  B: done
  C: done
  D: done
  E: todo
  F: todo
---

# Biome colour blending, biome flora and foliage variation

## Current state

Nothing in phases A to F is built yet. What exists and is relied on:

- `viewer/mesh.js` has `columnTints` (per-column grass and leaf tints from
  `ramp3(biome.grass | leaf, humidity)`, no blending), `tintFor`, `CROSS`
  and the 16-byte vertex (`VERTEX_BYTES`, `POS_SCALE`, `UV_SCALE`). One tint
  per column; F2 is accurate.
- `client/tiles.js` hard-codes `NAMES` and `TINTED`, uploads with
  `gl.generateMipmap` and `REPEAT` wrap on both axes, one sampler state for
  all passes. F1 is accurate.
- `client/scene.js` uses `cutoff = msaa ? 1/255 : 0.5` with
  `SAMPLE_ALPHA_TO_COVERAGE`, and sets `skyColor` from the spawn chunk
  centre only (lines near 169 and 184). F1 and F2 are accurate.
- `viewer/index.html` also holds its own copy of the block id table
  (line ~219), alongside `viewer/mesh.js`'s `B`; C.2's single tile list
  should cover it too.
- `crates/terrain/src/decorate.rs` picks the biome per feature at its own
  column (deviation B3 in SPEC) and runs snow last, per column.
- `data/blocks.toml` has `tall_grass`, `fern`, `dandelion` (collision
  `none`, opacity 0); `data/biomes.toml` has `grass_ramp`, `leaf_ramp`,
  `sky_ramp` per biome. No `[blend]`, `water_ramp`, `dry_ramp` or
  `flora.toml` yet.
- Phase A progress: A.1 and A.3 are built (PR #208, `client/samplers.js`: per-pass samplers, cutout clamp and LOD cap); A.5 is built (PR #210: plant top flag in vertex byte 14 and distance fade). Phase A is not done: A.2 and A.4 remain (see Reconciliation notes).
- Phase A is done: A.1, A.3 and A.5 in PRs #208 and #210 (`client/samplers.js`, plant top flag and fade); A.2 and A.4 arrived through `viewer-rendering.md` Phase B (`viewer/mips.js` `buildMips`, used by `client/tiles.js`; coverage-preserving alpha, `fwidth` alpha sharpening and cutoff 0.5 in `client/scene.js`). Plan #203's tickets #206 and #207 closed via #208 and #210.
- Phase B is done (plan #1028): `[blend] radius` and legend (PR #1042), blurred column tints (PR #1043), per-vertex corner tints (PR #1045), client `blend` setting and flyover radius (PR #1047), sky and fog easing toward the player (PR #1048), biome water tint ramp (PR #1049). The `[manual check]` item for Phase B remains for the maintainer.
- Phase C is done (plan #1052): variant art (PR #1061), shared tile list and `VARIANTS` (`viewer/tilelist.js`, PR #1062), texture variants in the mesher (PR #1063), plant offset (PR #1064), plant mirror, height, brightness and dark base (PR #1065), wind sway for plants and leaves (`client/sway.js`, PRs #1067 and #1068, as decided in #1053). C.1 came from `viewer-rendering.md` Phase B. The `[manual check]` items for Phase C remain for the maintainer.
- Phase D is mostly built (plan #1070): block ids (PR #1092), art (PRs #1090, #1091), cross and tall cross mesher shapes (PR #1095), `dry_ramp` (PR #1094), `ground_cover` (PR #1093), `bush` (PR #1099), `data/flora.toml` and loader (PR #1096), column-local placement pass (PR #1100), winner noise (PR #1103), two-block tall plants (PR #1104), first set and `stats` per biome (PR #1105). One item is missing, so Phase D stays `todo` (see Reconciliation notes).
- Phase D is done: the missing Taiga `fern_double` arrived in plan #1281 (ticket #1278, PR #1288: `fern_double_lower`/`fern_double_upper` blocks, art and the Taiga entry in `data/flora.toml`). The `[manual check]` items for Phase D, if any, remain for the maintainer.
- Phase E is mostly built (plan #1302): snow replaces `cross`/`tall_cross` flora (PR #1351), `requires` condition and cattail (PR #1354), marsh_orchid (PR #1371), lily_pad (PR #1368), `canopy` condition and jungle_bush (PR #1372), large_fern and broadleaf (PR #1374), Shrubland bush, berry_bush and dry_grass (PR #1376), tulips (PR #1401), cactus_flower (PR #1399), frost_grass (PR #1402). The coast `dune_grass` is missing, so Phase E stays `todo`.
- Art for the existing plants is in `art/batches_13_14.json`. None of the
  new tiles listed in C.2, D.5 or E exist.

## Reconciliation notes

- **Phase E still incomplete (plan #1302).** The Coast `dune_grass` entry (tall, dry-tinted, on sand at y 62 to 66 within 3 blocks of water, in every biome except Taiga, Tundra and Ice Desert, as decided in #1313 and spec'd in PR #1320) has no ticket in plan #1302 and no entry in `data/flora.toml`, block ids or art. Everything else in Phase E is built. Suggest one follow-up ticket for `dune_grass`: block ids, art, a column condition for sand near water, and the flora entry.

- **Phase D done (plan #1281).** The gap below was built in #1288. Original note (plan #1070 left Phase D unfinished): D.5 (as decided in #1073) asks for a Taiga `fern_double` (`tall_cross`, lower and upper tiles). `data/flora.toml` has the single-block `fern` in Taiga, but no `fern_double` entry, block ids or art exist, and plan #1070 had no ticket for it. Suggest one follow-up ticket: `fern_double_lower`/`fern_double_upper` blocks, art, and a Taiga entry in `data/flora.toml`. Everything else in D is built.

- **Phase A done (plan #203).** A.2 and A.4 were built under `viewer-rendering.md` Phase B (now `done`) as #205 decided: `client/tiles.js` calls `buildMips` from `viewer/mips.js` and `client/scene.js` sharpens alpha with `fwidth`. The `[manual check]` items for Phase A remain for the maintainer.

- **Header.** Was `idea`; now `accepted` (merging this PR accepts it).
  `scope: both` fits: A to C are client and viewer, D to F touch
  `crates/terrain` and the mesher.
- **Overlap with `viewer-rendering.md`.** Its Phase B (foliage mips,
  alpha-to-coverage, per-block UV variation) duplicates A.2, A.4 and C.1.
  `viewer-rendering.md` is still `idea`; see open question 1
  and resolve it before planning Phase A or C so both aren't built.
- **`no_std` invariant.** D's `data/flora.toml` is read by
  `crates/terrain/build.rs`, which is allowed. The noise and hash must use
  `crates/terrain/src/noise.rs` primitives; no new runtime dependency.
- **Determinism.** D.2 claims the flora pass never changes a Beta-placed
  block and never consumes the cell RNG. It also must write only to air, and
  the existing snow pass reads the surface, so the pass has to sit before
  snow as the doc says. It needs the Determinism test the doc already lists.
- **Wasm size.** D and E add blocks, data and mesher shapes; report gzipped
  wasm size per CLAUDE.md. A to C are JS only and don't change it.
- **World output.** A to C: none. D to E: air becomes plant blocks, so
  `stats` before and after is required in each PR; the "flora disabled
  equals today" test is the right guard.
- **Vertex byte 14.** A.5 and C.5 reserve byte 14 and viewer-rendering C
  reserves byte 13; the 16-byte vertex is full-ish. Whoever lands first
  should record the layout in `viewer/mesh.js`.
- **SPEC facts.** F4's table of tall grass, fern and dandelion biomes
  matches SPEC 9.1. Survival rules for new plants are this project's own
  and SPEC 12 doesn't cover them.
- **Game.** D.4 refers to "Basic gameplay 5" (targeting); that doc's
  Targeting section is 5, consistent.
- **`[manual check]` items** already tagged. Measurable versions: for A,
  compare the opaque pixel share of a plant quad at 20/40/80 blocks in a
  headless render against level 0 coverage; for B, the colour-step test
  already given is sufficient.
- No item is `[external]`.

The world reads as Beta in a way that's charming up close and flat from a
few chunks away: grass changes colour in a hard line at every biome border,
every biome has the same handful of plants, every grass tuft is the same
texture standing in the same spot of its block, and distant plants turn into
faint rectangles. Later versions of the game fixed each of these, and this
doc takes the ideas (not the numbers or the code) and fits them to this
project's palette, biomes and raft-at-sea setting.

Four things, in order of how cheap they are:

1. **Fix the plant planes** so cross-quad plants never show their rectangle.
2. **Blend biome colours** across borders and between columns.
3. **Vary textures and placement** of grass, terrain tops and plants.
4. **Biome-specific flora**: new plants, data-driven, placed per biome.

Phases A to C change only rendering (`viewer/mesh.js`, `client/`, the
viewer). Phases D and E change world output, but only by turning air into
new plant blocks; no block that Beta generation places today moves or
changes.

Inspiration is taken from what later versions visibly do (smooth biome
colour transitions, offset and varied plants, flower patches that cluster
by colour, ferns and berry bushes in taiga, lily pads in swamps, dry grass
in deserts, leaf litter in forests). Nothing here is to be taken from that
game's code, data or textures; all numbers below are this project's own
starting guesses, and all art is new 8x8 palette art per `art/PROMPT.md`.

## Findings

### F1. Plant sprites mipmap into rectangles

- `client/tiles.js` uploads 8x8 art upscaled to 16x16 and calls
  `gl.generateMipmap`. Levels 2 to 4 (4x4, 2x2, 1x1) average the sprite's
  coverage over the whole tile.
- `tall_grass` covers 29 of 64 texels. At the 1x1 level every texel of the
  quad has coverage about 0.45; at 2x2 the bottom half is about 0.6 and the
  top half about 0.3.
- With MSAA the cutout pass uses alpha-to-coverage and `cutoff = 1/255`, so
  that blurred coverage draws the whole quad as a screen-door rectangle.
  Without MSAA (`cutoff = 0.5`) the bottom half of the quad draws solid.
  Either way, at a distance you see the plane, not the plant.
- The array uses `REPEAT`, so filtered samples at a quad's top edge blend
  with the tile's bottom row (the opaque stems) and its left and right edges
  wrap into each other. That draws a faint line along the edges of each
  plane once mips kick in.
- There's no alpha sharpening, so coverage at mid distance is soft and
  shimmers as the camera moves.
- Leaves go through the same path, but `leaves_fast` hides most of it.

`docs/design/viewer-rendering.md` (F4, Phase B.1 and B.2) describes the
same mip problem for the viewer. The client has since split colour and tint
into separate premultiplied arrays (commit "mipmap fix"), which solved the
tint flicker but not coverage.

### F2. Biome colour changes in hard steps

- `columnTints` (in `viewer/mesh.js`, shared by the viewer, the flyover
  workers and the client) gives each column `ramp3(biome.grass, H)`.
- Inside a biome that is smooth, because H is. At a border the ramp itself
  switches, so the colour jumps in one block, along a staircase that follows
  the biome lookup's 64x64 quantisation.
- All four vertices of a face get the same tint, so even smooth climate
  shows as one flat colour per block.
- The client's sky and fog colour come from the spawn chunk's centre column
  once, so they never change as the player moves.

### F3. Every grass block and plant is identical

- Same tile, same orientation on every grass top: a regular 1-block grid at
  low angles (viewer-rendering F5 says the same).
- Cross plants always sit centred in the block at the same height, one
  texture per species, the same brightness, so a meadow reads as a grid of
  identical Xs.

### F4. Biomes differ in trees, not ground cover

- SPEC 9.1: tall grass only in Rainforest, Seasonal Forest, Forest, Taiga
  and Plains; ferns only in Rainforest; dandelions in four biomes; roses,
  sugar cane, pumpkins and mushrooms everywhere.
- Savanna, Shrubland, Swampland, Tundra and Ice Desert have bare grass
  apart from those universal plants.
- Decoration already picks the biome per feature at its own column
  (`crates/terrain/src/decorate.rs`, deviation B3), so new flora can follow
  borders exactly.

## Phase A: plant planes that never show

No world output change. Lands in the client and, through shared code, the
viewer.

Items 2 and 4 (the JS mip builder and the shader alpha sharpening), and
Phase C.1 (terrain top rotations and mirrors), are built by
`docs/design/viewer-rendering.md` Phase B.1, B.2 and B.3, which keeps them.
This doc depends on those and adds only the client side: using the shared
mip module in `client/tiles.js` and the sharpening in the client's cutout
program, plus the other items here. (Decided in #205.)

1. **Clamp, don't wrap.** Sample the cutout pass with `CLAMP_TO_EDGE`.
   Today no face uses UVs outside 0..1, but viewer-rendering Phase E plans
   greedy-merged water quads that rely on `REPEAT`, so use a WebGL2 sampler
   object per pass (`REPEAT` for opaque and water, `CLAMP_TO_EDGE` for
   cutout) rather than changing the texture's own wrap mode.
2. **Build mips in JS**, one shared module used by `client/tiles.js` and the
   viewer's loader:
   - colour averaged weighted by coverage (premultiplied, as now), so
     transparent texels add no black;
   - tint grey averaged the same way;
   - **coverage-preserving alpha:** per layer and per level, scale alpha so
     the share of texels at or above 0.5 matches level 0.
3. **Cap the blur.** Set `TEXTURE_MAX_LOD` on the cutout sampler to about
   2.0 (the 4x4 level). Past that the sprite has no shape left to keep.
4. **Sharpen in the shader.** In the cutout program, replace the raw
   coverage with `a = (a - 0.5) / max(fwidth(a), 1e-4) + 0.5`, clamp, use
   it for alpha-to-coverage, and put `cutoff` back to 0.5 on the sharpened
   value with or without MSAA.
5. **Fade plants out, don't let them alias.** Beyond a plant draw distance
   (start at 48 blocks; a client setting), shrink cross quads toward their
   base over 8 blocks in the vertex shader, so they vanish into the grass
   instead of turning into noise. This needs a per-vertex "top" flag (see
   Phase C.5, byte 14).

**Acceptance**
- Unit test on the mip builder: for `tall_grass`, `fern`, `dandelion` and
  `leaves`, the share of texels at or above 0.5 at each level is within 5%
  of level 0.
- Unit test: the cutout sampler is clamp and the opaque one repeat.
- A meadow in Plains, seed 12345, at 20, 40 and 80 blocks: no visible
  quad outlines or edge lines, no screen-door rectangles. `[manual check]`
- Forest canopies at 40 to 80 blocks keep their mass. `[manual check]`

## Phase B: biome colour blending

No world output change. All in `columnTints` and the mesher, so the viewer,
flyover and client get it together.

1. **Blend radius.** `columnTints` computes raw tints (grass, leaf, and any
   later ramps) for the area plus a ring of `R` columns, then box-blurs
   each with separable running sums (two passes, O(n) per column whatever
   `R` is). Inner columns get the blurred value.
   - `R` comes from data (`data/biomes.toml`, `[blend] radius`, default 3,
     so a 7x7 window) and can be overridden by a client setting, 0 to 7.
     The default stays 3, one value in the data. (Decided in #1029.)
   - The flyover workers may use a smaller radius for speed; say so in the
     flyover README.
2. **Per-vertex tint.** Each top-face corner takes the average of the 4
   blurred columns that meet at it. Side faces use the same corner values,
   so a grass block's side matches its top at the edge and its neighbour's
   side. The vertex format already carries tint per vertex; `Quads.quad`
   just needs to accept four.
3. **Determinism.** The tint at a column depends only on world columns in
   its window, never on the chunk origin, so adjacent chunks meet without a
   seam. The worker already fetches climate maps per area; fetch them for
   the padded area.
4. **Sky and fog follow the player.** Every few ticks, sample the sky ramp
   over a small window around the player's column and ease the client's
   `skyColor` toward it over about 2 seconds, so walking or rafting into a
   swamp slowly greys the sky.
5. **Water tint (optional, own ramp).** Add `water_ramp` per biome, mark
   `water_0` and `water_flow_0` as tinted texels, and blend like grass.
   Swamps murkier, rainforest greener, tundra slate. Ocean columns use the
   biome under them, which is already what the raft scene does.

**Acceptance**
- Unit test: `columnTints` for two overlapping areas gives identical
  blurred tints for every shared world column (both with R = 0 and R = 7).
- Unit test: across a known Plains / Forest border on seed 12345, the
  largest colour step between neighbouring columns with R = 3 is under a
  third of the step with R = 0.
- Mesh test: a grass block's top and side corners at the same world corner
  carry the same tint.
- Biome borders read as gradients, not lines, in the client and flyover.
  `[manual check]`

## Phase C: texture and placement variation

No world output change. Everything is chosen from a hash of the block's
**world** coordinate, so the preview, flyover and client agree, and it
never changes when a chunk is remeshed. `meshBlocks` gains the world origin
of its area, as viewer-rendering Phase B.3 also requires.

1. **Terrain tops.** Grass, dirt, sand, gravel, stone and snow tops pick
   one of the 8 rotations or mirror images of their UVs. Sides only mirror
   horizontally, so grass sides keep their green strip on top.
2. **Texture variants.** A block can have several tiles, in contiguous
   texture layers, picked by hash with weights from data:
   - new art: `grass_top_1`, `grass_top_2` (fewer flecks; one with a
     yellow fleck), `tall_grass_1` (short tuft), `tall_grass_2` (tall,
     seeded), `fern_1`, `dandelion_1`, `rose_1`;
   - the face table keeps the base layer; a per-block `variants` count and
     weight list (in `data/blocks.toml` or the art batch metadata) gives
     the offset;
   - `client/tiles.js` and the viewer each hard-code the tile name list.
     Derive both from one exported list so new tiles are added once.
3. **Plant offset.** Cross plants shift in x and z by up to 3/16 of a block
   (quantised to the vertex format's 1/16), from the hash. The quads span
   0.15 to 0.85, so they stay inside the column.
4. **Plant shape.** Per plant, from the hash:
   - mirror the texture (swap u);
   - tall grass and ferns scale height between 12/16 and 16/16;
   - brightness jitter of about ±6% on the tint;
   - bottom two vertices darker (about 0.8), a cheap stand-in for the
     ground occluding the base, which also sells the offset.
5. **Wind.** Store a sway weight in vertex byte 14 (0 at a
   plant's base, 255 at its top; byte 13 is planned for AO in
   viewer-rendering Phase C). The vertex shader moves weighted vertices by
   a small sine of time and world position. Leaves sway too, with a
   smaller weight. Phase A.5 reuses the same byte to know which vertices are
   tops. (Decided in #1053.)

**Acceptance**
- Mesh test: a block meshed in two areas with different origins gets the
  same UV variant, texture layer and offset.
- Mesh test: plant offsets keep every vertex inside its column.
- No regular 1-block pattern on large grass, sand or stone areas at low
  angles; meadows no longer look gridded. `[manual check]`
- `art/tools/validate.py` passes with the new batch.

## Phase D: flora framework and first plants

World output changes (air becomes new plant blocks). Report `stats` before
and after per CLAUDE.md.

1. **Data.** A new `data/flora.toml`, read by `crates/terrain/build.rs`:

   ```toml
   [[flora]]
   name = "daisy"
   block = "daisy"
   shape = "cross"            # cross | tall_cross | ground_cover | pad | bush
   ground = ["grass"]
   tint = "none"              # none | grass | leaf | dry
   cluster_scale = 24.0       # blocks per noise cell for patchiness
   biomes = { Plains = 0.020, Forest = 0.004 }   # chance per eligible column
   ```

   New block ids go in `data/blocks.toml` (collision none, opacity 0, like
   tall grass). Two-block plants get a lower and an upper id.
2. **Placement: a column-local pass.** After the four parity passes and
   before snow, for each column whose top block is valid ground with air
   above:
   - the column's biome (already per column) gives each flora entry's
     density;
   - a low-frequency noise per entry, from the seed and a fixed salt in
     data, multiplies density, so plants come in patches instead of
     evenly;
   - a second low-frequency noise picks *which* flower wins where several
     compete, so a meadow has drifts of one colour (the flower-field look)
     rather than confetti;
   - a hash of (seed, x, z, entry) makes the final roll.
   
   Because it reads only its own column and the noise, it has no
   cross-cell order, needs no margin, and never changes a block placed by
   a Beta pass. It also doesn't consume the cell RNG, so Beta features
   stay where they are for every seed.

   Snow runs after flora and replaces flora of shape `cross` and
   `tall_cross`; `ground_cover`, `bush` and `frost_grass` stay.
   (Decided in #1072.)
3. **Mesher shapes.** `tall_cross` (two stacked cross quads with lower and
   upper tiles), `ground_cover` (one tinted cutout quad lying 1/16 above the
   block below, up-facing), `pad` (the same, on a water surface),
   `bush` (an inset leaves-style cube). Plus the Phase C variation.
4. **Game.** New flora is targetable (Basic gameplay 5), has no collision
   and doesn't block swimming or walking. New flora drops nothing and is
   not pickable for now: it is decorative, and gets no `[pickable]` entry
   in `data/items.toml`. (Decided in #1071.)
5. **First set**, chosen to give the bare biomes something:
   - Plains: `daisy` (white, `e` with `f` centre), `bluebell` (blue,
     `b`/`a`), `tall_grass_double`.
   - Taiga: ferns in two sizes, the existing `fern` block as a flora entry
     and a new two-block `fern_double` (`tall_cross`, lower and upper
     tiles); `berry_bush` (bush shape, `c` dots). (Decided in #1073.)
   - Savanna and Desert: `dry_grass` and `dry_grass_tall`, tinted by a new
     per-biome `dry_ramp` (tan to straw).
   - Forest and Seasonal Forest: `leaf_litter` (ground cover, tinted with
     the column's **leaf** ramp, so Seasonal Forest litter comes out
     autumn orange for free).

**Acceptance**
- Determinism test: overlapping regions agree on every flora block, with
  and without decoration, and output is independent of region origin and
  size.
- Test: with flora disabled in data, every block equals today's output for
  seeds 12345, 9, 42.
- `stats` prints flora per biome; every new entry appears in its biomes and
  never outside them.
- Wasm size reported before and after.

## Phase E: the rest of the biomes

Same framework; mostly data and art.

- **Swampland:** `marsh_orchid` (blue, `a`/`b`), `lily_pad` (pad shape, on
  still water touching land within 4 blocks), `cattail` (tall cross, on
  grass or dirt next to water).
- **Rainforest:** `large_fern`, `broadleaf` (a big low cross), `jungle_bush`
  (bush on grass under canopy).
- **Shrubland:** `bush`, `berry_bush`, sparse `dry_grass`.
- **Seasonal Forest:** `tulip` in red, salmon and yellow (`c`, `d`, `f`),
  clustered by the colour noise.
- **Desert:** `cactus_flower` (small cross on top of a cactus's top block).
- **Tundra and Ice Desert:** `frost_grass`, sparse short tufts, pale tint;
  stays when snow falls (see D.2).
- **Coast** (any warm biome, sand at y 62 to 66 within 3 blocks of
  water): `dune_grass`, tall and dry-tinted. "Warm" means every biome
  except Taiga, Tundra and Ice Desert. There's no beach biome, so
  this is a column condition, not a biome; it gives every island a fringe
  the player sees from the raft. "Within 3 blocks of water" means a water
  block at the sand block's y or one block below (y-1 to y), in any column
  of the square of 3 columns around it (Chebyshev distance, not a circle).
  (Decided in #1313 and #1410.)

## Phase F: underwater flora (later)

Seagrass and kelp would suit a game about fishing from a raft, and the fish
habitats in `crates/layout` could use them as cover. They need a plant that
also draws as water around it ("waterlogged"): a block id the mesher draws
as both a water volume and a cross quad, with its own physics and swimming
rules. Big enough to deserve its own design pass; listed here so Phase D's
shapes leave room for it.

## Open questions

6. **Plant draw distance in the flyover.** Plants beyond about 80 blocks
   add faces without adding much.
   A. Don't mesh cross plants in `fast` chunks.
   B. Mesh them and rely on Phase A.5's fade.
