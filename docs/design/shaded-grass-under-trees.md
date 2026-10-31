---
title: Shaded grass under trees
status: accepted
scope: both
source: "#1610"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Nothing of this idea is built. What exists and is relied on:

- `viewer/mesh.js` `columnTints(wasm, ctx, seed, wx, wz, size, R)` builds per-column `grass`, `leaf`, `water` and `dry` tints from `ramp3(biome.<ramp>, humidity)`, box-blurs them over `[blend] radius` (`data/biomes.toml`, `radius = 3`) with the exact integer `boxSum`, and averages them per block corner (`corners`). It takes no blocks: it reads only the biome, humidity and temperature maps.
- `meshBlocks(ctx, blocks, tints, size, pad, ...)` picks a tint per block through `tintFor` (~line 446) and uses `tints.corners` for per-vertex colour. `meshBlocks` already has the blocks it needs (`at(x, y, z)`, with `pad` columns around the area).
- `dry_ramp` exists for all 11 biomes in `data/biomes.toml` and arrives in the legend as `bi.dry`, so no biome is left without one.
- `data/flora.toml` documents `requires = "canopy"`; `crates/terrain/src/flora.rs` `under_canopy` is the generator's definition (highest grass with air above and leaves anywhere higher in the column). Only `jungle_bush` uses it. It runs at generation time and its result is not exposed to the mesher.
- Callers of `columnTints`: `viewer/worker.js` (blocks padded by 1 column), `client/scene.js` `meshRegion` (`padRegion`, 1 column; the test scene uses pad 0) and the flyover. So the viewer and flyover would get the effect from a shared change.
- Tests: `viewer/test/blend.test.mjs`, `blend_corners.test.mjs`, `client/test/meshorigin.test.mjs`.

## Reconciliation notes

- **`columnTints` has no blocks.** The doc wants the shade fraction to go through "the existing tint blur". That blur runs on climate colours inside `columnTints`, before any blocks are known. Smallest way: compute a per-column leaf-above value from `blocks` in or next to `meshBlocks` (or pass `blocks` and `pad` to `columnTints`), then blur it the same way with an exact integer sum so the result depends only on the window. Keep it in the mesher, not the wasm generator; world output is unchanged (matches the doc's preference).
- **Padding is too small (the doc asks to check).** Callers pad blocks by 1 column. A 3×3 window needs 1, a 5×5 window needs 2, plus the blur radius if the shade value itself is blurred: `R + window` columns, with a full-height column in each. Today's pad does not cover that, so either raise the pad (`worker.js` and `client/scene.js` both call `padRegion`/generate with 1) or accept an edge that is exact only to the pad and use a smaller window. Determinism needs the result for a column to depend only on world blocks: border columns must be computed from enough padding, or the outer ring discarded as `columnTints` does with `padded`.
- **Full height is available.** Pad columns come with the whole `H = 128` column (`col(x, z) + y`), so "leaves above the surface" is checkable there. The scan is one pass per column, not per face, as the doc wants. Scan each column down from the top and stop at the first grass top; leaves are always above it, so `meshBlocks`'s `y0` cutoff (`minY - 8`) does not matter.
- **Definition of "under canopy".** `under_canopy` in `flora.rs` is binary and in Rust. The doc prefers option C (fraction of nearby columns). That differs from the generator's definition by design; the doc should say so in the code comment, and the mesher must not claim to match `flora.rs`. Not wired to `jungle_bush`.
- **Existing bug next to this idea.** In `tintFor` (`viewer/mesh.js`, ~lines 226 to 235) `dry_grass`, `dry_grass_tall_lower` and `dry_grass_tall_upper` appear in two `case` lists. The first (comment: "until the dry ramp exists") wins, so those blocks take the grass tint and the `dry` return is unreachable. `dry_ramp` now exists. Phase B reuses `dry` for flora, so fix it there or the shifted target is not visible on dry grass. It is a bug and Phase B fixes it. (Decided in #1649.)
- **Flora tints.** Tall grass, large fern, ferns and `broadleaf`/`bush` take `grass`, or `grass * 0.88` for ferns, or `leaf` for bushes (`tintFor`). "Follows the same shift" means applying the same mix to those; bushes use the leaf tint, so decide whether they shift (open question 2).
- **Light versus tint.** The client has vertex AO (`faceAO`, viewer-rendering Phase C) but no sky light. The doc's rule applies: this is a tint, applied on top of AO, and must not be reused as a light substitute. Shading by the Beta `0.75 + y/256` factor is unrelated.
- **Not Beta behaviour.** The doc states that SPEC gives grass colour as a function of climate only (see SPEC.md section 9, per-biome effects; not re-verified here). This doc layers a project choice on top and takes no fact from SPEC.md beyond that; nothing is added to SPEC.md.
- **Data.** The mix amount (and window size, cap) must be a key in `data/*.toml` read through the legend (`[blend]` is the natural place, e.g. `shade_mix`, `shade_radius`), not a literal in `mesh.js`. `crates/terrain/build.rs` already compiles `data/*.toml`; the legend must expose the new keys to JS. `crates/terrain` stays `no_std` and dependency-free; if only the legend changes, wasm size barely moves. Report gzipped wasm size if `crates/web` changes.
- **Player changes.** The client remeshes a region when blocks change (`meshRegion(..., remesh)`), so the shade is recomputed from the blocks each time, with nothing stored. Neighbour regions whose window crosses the edge are stale until remeshed; the existing `stale` queue only covers newly loaded neighbours, not edits. `[manual check]` in practice; see open question 4.
- **Overlap.** `docs/design/biome-flora.md` (Phase B blend, Phase D `dry_ramp`, Phase E `canopy`) is done and is the base; this doc changes none of its decisions. No conflict with `viewer-rendering.md`.
- **`[manual check]`:** "a little, not brown: the forest floor should still read as grass" and "reads in every biome that has trees and grass" are visual. Measurable proxy: a test that the shaded top-face tint equals `lerp3(grass, dry, shade_mix * f)` with `f` in `[0, 1]`, is `grass` exactly at `f = 0`, never goes past `shade_mix` of the way to `dry` (so the maximum shift is bounded by data), and that two overlapping areas agree on shared columns. The maintainer judges the look; per CLAUDE.md, don't judge it from screenshots.
- **World output:** unchanged (render-only). No `stats` before/after needed.

# Mince Raft: Shaded grass under trees

Status: idea
Scope: client (possibly generator; see 3)

Grass under a tree is the same colour as grass in open sun. Under real canopies the ground gets less light, so the grass there is thinner, yellower and patchier. This idea makes grass beneath trees a little more dead-looking: a gentle shift of its tint toward the biome's dry colour where the column is shaded by leaves above. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- Grass tint comes from `ramp3(biome.grass, humidity)` in `columnTints` (`viewer/mesh.js`), blurred over `[blend] radius` and averaged per vertex corner (biome-flora Phase B).
- `dry_ramp` per biome in `data/biomes.toml` (tan to straw) already tints dry grass flora (biome-flora Phase D).
- `data/flora.toml` has a `canopy` condition ("stands on grass under canopy") used by `jungle_bush`, so "is this column under leaves" already has a definition somewhere in the generator.

> **Reconciler:** All three are accurate, with two caveats. `columnTints` takes no blocks (see Reconciliation notes). And `dry_ramp` reaches the mesher as `tints.dry`, but `tintFor` currently returns the grass tint for dry grass because of a duplicated `case`.

## 2. Want

- Want: a grass top face under canopy takes a tint mixed part-way from its normal grass tint toward `dry_ramp`. A little, not brown: the forest floor should still read as grass. The mix amount lives in `data/*.toml`, not as a magic number.
- Want: graded, not on/off. Options for how shaded a column is:
  - A. Binary: any leaf block above the grass in the column. Simplest, but gives hard tree-shaped stains.
  - B. Count of leaf blocks above, capped (thicker canopy, deader grass).
  - C. Fraction of nearby columns (e.g. 3×3 or 5×5) with leaves above, then the existing tint blur smooths it. **Preferred**: soft edges, no single-block stains, and it reuses the blend pass.
- Want: grass flora on those columns (tall grass, ferns, bushes) follows the same shift, so the plants match the ground.
- Want: it reads in every biome that has trees and grass. Biomes with no `dry_ramp` (if any) are left unchanged.
- Want: optional and cheap. A column scan for leaves above the surface is one pass per column in the mesher, not per face.

> **Reconciler:** Option C is what the phases below build. Every biome has a `dry_ramp` (11 of 11), so the "no `dry_ramp`" case does not occur, but the mesher should still leave a column alone if `tints.dry` is missing, as `tintFor` already handles `dry ?? WHITE`. "Optional" is read as a data switch: `shade_mix = 0` turns it off with no other change. The test scene (`pad 0`) has no neighbours, so it gets the edge-limited result.

## 3. Things to check

- **Render-only or world change?** Doing it as a tint in the mesher changes no blocks (preferred). Swapping some shaded grass for a different block (coarse dirt, podzol-like) would change world output and needs before/after `stats`; the reconciler should keep that out unless the maintainer asks.
- **Determinism / borders.** A tree near a chunk or region edge can shade columns in the next region. The shade value for a column must depend only on world blocks, so the mesher needs the neighbouring columns' leaves (the tint blur already pads for its radius; check whether that padding includes the full height above the surface).
- **Light vs. tint.** If the client ever gets real sky light or ambient occlusion, this should be a tint effect on top of that, not a substitute; say so in the doc so the two don't fight.
- **Player-built changes.** If leaves are broken or placed later, the mesh rebuild should pick up the new shade; nothing to store.
- **Viewer and flyover** share `columnTints`, so they'd get it too. That's probably wanted; confirm.
- **Not Beta behaviour.** Beta grass colour depends only on climate (SPEC). This is a project choice layered on top; don't take facts about the original from anywhere but SPEC.
- **World output:** unchanged if tint-only.

> **Reconciler:** Answers, from the code. Render-only: kept out of the world, as preferred. Borders: the blur pads climate colour only, and the block padding is 1 column, so it does not yet cover a shade window (Reconciliation notes); full column height is present in the padded blocks. Light vs. tint: stated in the notes; AO exists and is separate. Player changes: the region remesh recomputes from blocks. Viewer and flyover: `worker.js` calls `columnTints` and `meshBlocks` the same way, so they share the change; the maintainer confirmed that is wanted: the viewer and flyover get the shaded tint too, with no client-only option. (Decided in #1627.)

## Phases (reconciler)

**Phase A.** The shade value and the grass top face. Add the data keys (`shade_mix`, and the window size) to `data/biomes.toml` `[blend]`, expose them through the legend, and raise the block padding the callers pass if the window needs it. The window is 3×3, and the block padding is raised to cover the window plus the blur, so region edges agree. Compute, per column, whether leaves lie above the top grass block (one scan per column), take the fraction of that over the window (option C), blur it with an exact integer sum so it depends only on world blocks, and mix the grass top face tint toward `tints.dry` by `shade_mix * fraction`, per corner like the existing tints. Done when: tests show a column under a tree gets a tint between grass and dry and an open column keeps its grass tint exactly; `shade_mix = 0` reproduces today's tints; a column's value is the same from two overlapping areas with different origins and sizes (the determinism test CLAUDE.md asks for); breaking or placing a leaf block and remeshing changes the result; `node --test` for viewer and client passes, `preview.mjs` still plans a route, and `./build.sh` succeeds with wasm size reported if it changed. (Decided in #1628.)

**Phase B.** Flora follows the ground. Apply the same mix to the tint of grass-tinted flora on shaded columns (`tall_grass`, double tall grass, `fern`, `large_fern`, `broadleaf`, and `bush` if open question 2 says yes), and fix the duplicated `dry_grass` case in `tintFor`: the grass-tint result for `dry_grass` and its tall halves is a bug, and they use the `dry` ramp. Done when: a test shows a plant on a shaded column takes the same fractional shift as the grass under it, a plant on an open column is unchanged, and dry grass takes the `dry` tint. (Decided in #1649.)

## Open questions

2. **Bushes and leaves.** `bush`, `berry_bush` and `jungle_bush` use the leaf tint, not the grass tint. Should they shift too?
   - A. No: only grass-tinted plants (tall grass, ferns, `broadleaf`).
   - B. Yes, toward the dry ramp, like the ground.
5. **Neighbour remesh on edits.** When a player breaks leaves at a region edge, should the neighbour region be remeshed too?
   - A. Yes, queue the adjacent regions through the existing `stale` path.
   - B. No: accept a seam until the neighbour next meshes.
