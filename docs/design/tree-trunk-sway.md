---
title: Tree trunks sway in the wind
status: accepted
scope: game
source: "#1609"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Checked against `master` (93cee34).

- **Sway maths exists** in `client/sway.js`: `SWAY_AMPLITUDE` 0.08, `SWAY_SPEED`, `SWAY_WAVE`, `swayPhase(nowMs, reducedMotion)` (returns 0 under reduced motion) and `swayOffset(weight, x, z, phase, amplitude)`. It was built in `biome-flora.md` Phase C.5 (PRs #1067, #1068).
- **The shader mirror** is in the `program(cut)` vertex shader in `client/scene.js` (~lines 95-120): the sway lines are emitted only when `cut` is true, so only `cutProg` sways. `swayPhase` is set only for `cutProg` (~line 601); `solidProg` has no sway uniform. The shader reads the weight from attribute `top` (location 5).
- **Byte 14** is written by both meshers: `crates/mesh/src/mesh.rs` and `crates/mesh/src/shapes.rs` (`LEAF_WEIGHT` = 64) for Rust, `viewer/mesh.js` (`LEAF_WEIGHT`, `swaysUnder`, header comment ~line 13) for JS. Opaque faces, logs included, get weight 0. `viewer/mesh.js` documents that the client fades only weights >= 128 (plants), so any new weight on a cutout vertex must stay below 128 or the fade must be told apart.
- **Greedy merging of opaque faces** is in `crates/mesh/src/greedy.rs`, called from `mesh.rs` (~line 273): an untinted, non-cutout-leaf cube face whose four AO levels are equal goes into a per-slice grid and is merged with equal neighbours (same layer, shade, AO, variant `vr`). Logs are untinted, so they merge today, sideways and vertically.
- **Tree shapes** are in SPEC 11 (11.5 Big oak has diagonal branches). Nothing in `crates/mesh` knows which logs belong to which tree.
- **Reduced motion** is already handled by `swayPhase`. Plant fade is in `client/plantfade.js`, the pattern the idea names for the shader mirror.
- **Not built:** no trunk lean, no opaque-program sway, no base-height lookup.

## Reconciliation notes

- **Opaque program needs a branch.** The idea says logs use the opaque program, which ignores byte 14. True: `program(false)` omits the sway block and `solidProg` has no `swayPhase` uniform. Suggest keeping one shader source and enabling the lean only for the opaque program, driven by a uniform-free constant amplitude plus the phase uniform. The idea's check "weight 0 must cost nothing visible" holds if the lean multiplies by the weight; the frame-time cost on every opaque vertex should be measured with FrameTimes (`[manual check]`, proposed measurable form: median frame time of a fixed flyover seed within 3% of before).
- **Byte 14 meaning is overloaded.** On cutout vertices it is a plant/leaf weight (plants >= 128 fade and sink, leaves 64 never sink). On opaque vertices it is free, so a log can store "height above trunk base" there without clashing, but the mesh header comment in `viewer/mesh.js` and `crates/mesh/src/quads.rs` must say byte 14 means a different thing per layer. Suggest storing lean weight 0..255 as (height above base) / `TRUNK_LEAN_MAX_HEIGHT` clamped. Big oak trunks are the tallest case; the value of the constant comes from SPEC 11.5 and is not stated here.
- **Greedy merging.** The idea is right that merged log quads have only four vertices. Merging requires equal `layer, shade, ao, vr`; it does not compare weight. Suggest: logs are not entered into the greedy grid (the same exclusion as `cut_leaf` at `mesh.rs` ~line 273), or the cell gains a weight and merging is allowed only along y within a trunk. The first is smallest; the cost is more log quads (trunks are a tiny share of faces). Both meshers (Rust and the JS reference in `viewer/mesh.js`) must stay in agreement, since the JS one is the parity reference.
- **Per-vertex weights without merging** make the lean exact: each log block's bottom vertices get weight of its own base height, top vertices that plus one block. The weight is then piecewise linear along the trunk, and the whole tree moves together.
- **Finding the base.** The mesher takes a region's blocks. Scanning down from a log through log blocks (and, for branches, to the nearest log below within the trunk) depends only on world blocks, so the determinism invariant holds only if the scan is inside the meshed data. A trunk that crosses a region's vertical or horizontal edge would get a different base in different regions, and overlapping regions would then disagree. Suggest capping the scan at the region and, for any log whose scan reaches the region edge without finding non-log, using the height above the region floor of the run; or, simpler, define the weight from height above the **ground** (the first non-log, non-leaf block below the column's lowest log) with a padded neighbour read. Needs a test that two overlapping regions give identical byte 14 for the same log (`[manual check]` is not enough; this is testable).
- **Branches (SPEC 11.5).** A diagonal branch log has no log directly beneath it in the same column. Suggest weight = world height above the trunk base of the tree's column, found by looking down the nearest vertical run at the branch's own (x, z) and falling back to its y minus the tree's base y. Without a tree identity in the mesher, the simplest rule is "height above the lowest log in the connected log cluster within a small radius"; this is an open question.
- **Canopy agreement.** Option B (leaves also get the lean for their height above the tree base) needs the same base lookup for leaf vertices, which are cutout vertices that already use byte 14 for `LEAF_WEIGHT`. That means a second weight channel or a change to leaf encoding. Byte 14 is the only free-looking attribute; bytes 13 (AO) and 15 (water depth) are used, but byte 15 is 0 off water, so leaves could carry the base-relative height there. This needs a decision (open question 1).
- **Same wave as plants.** `swayOffset` and the shader use the world position of the vertex, so the lean should call the same function with a smaller `amplitude` (its parameter exists for this). Add a named `TRUNK_SWAY_AMPLITUDE` beside the others in `sway.js`. The shader would then mirror it as a constant, as `SWAY_AMPLITUDE` is today.
- **Collision, picking, world output.** Unchanged, as the idea says; no `crates/terrain` change, so the `no_std` and determinism invariants are untouched. The wasm size of `crates/mesh` may grow slightly; report gzipped sizes in the PR.
- **"Well under a pixel" is a `[manual check]`.** Measurable version: at `SWAY_AMPLITUDE` x `TRUNK_SWAY_FRACTION`, the largest top-of-trunk offset is <= 0.03 blocks (unit test on the constant), which at 1 block = 16 texels is half a texel.
- **Test approach (CLAUDE.md).** Pure maths and weight encoding are unit-testable; the shader mirror gets a test in the `plantfade.js` pattern. No screenshots.

## Idea (author's text)

# Mince Raft: Tree trunks sway in the wind

Status: idea
Scope: client

Cross plants sway from the base up and cutout leaves sway with a constant weight (biome-flora C.5, C.5b), but log blocks never move. A canopy drifting over a rock-still trunk reads slightly wrong up close. This idea adds a very small lean to tree trunks, growing with height above the tree's base, so the whole tree moves together in the wind. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `client/sway.js`: `swayOffset(weight, x, z, phase)`, amplitude `SWAY_AMPLITUDE` (0.08 blocks at weight 1), mirrored by the cutout vertex shader in `client/scene.js`.
- Mesh byte 14 is the sway weight (`viewer/mesh.js` header, `crates/mesh/src/mesh.rs`): 255 at the top of cross plants, 0 at their base, `LEAF_WEIGHT` on every leaf vertex, 0 elsewhere.
- Logs are opaque and go through the opaque program, which ignores byte 14. Opaque faces are greedy-meshed (Viewer E.2), so one quad can span many blocks of trunk height.
- Reduced motion already zeroes the phase (`swayPhase`).

## 2. Want

- Want: trunks lean, not wobble. Offset is 0 at the bottom log of a trunk (where it meets the ground) and grows with height up the trunk, so the base stays planted.
- Want: it is subtle. The top of a tall trunk moves well under a pixel's worth at normal viewing distance; a fraction of the leaf amplitude, set as a named constant beside the others in `sway.js`. `[manual check]`
- Want: trunk and canopy agree. Leaves at the trunk top should not visibly slide off it. Options:
  - A. Leaves keep their own constant-weight sway, trunk lean added on top; accept a little relative motion. Simplest.
  - B. Leaves also get the trunk lean for their height above the tree base, plus their own flutter. **Preferred**, if the mesher can find the base cheaply.
  - C. Only leaves near the trunk get the lean.
- Want: same wave as plants (same `phase`, world-position term), so a gust looks like it crosses the forest, not each tree on its own clock.
- Want: off under reduced motion, like the rest.
- Want: the pure maths stays in `sway.js` with a unit test; the shader mirrors it (the `plantfade.js` pattern).

## 3. Things to check

- **Greedy meshing.** A greedy log quad spanning several blocks has only four vertices, so a lean that grows with height is linear across it, which is fine for a straight lean; but log quads merged *sideways* across neighbouring trunks, or merged with non-log faces, would carry the wrong weights. Logs may need to be excluded from merging, or merged only vertically within one trunk.
- **Finding the base.** The weight needs "height above the lowest log of this trunk". The mesher sees the region's blocks; a trunk crossing a chunk/region boundary vertically is rare but the base lookup must still depend only on world blocks (determinism invariant).
- **Branches.** Big oak branches (SPEC 11.5) are diagonal logs. They should lean with the trunk at their height, not on their own.
- **Opaque program cost.** Adding a vertex offset to the opaque shader touches every opaque face. Weight 0 must cost nothing visible, and the frame-time numbers (FrameTimes) should not move.
- **Collision and picking** use the unmoved blocks; the lean is visual only and small enough that this shouldn't matter.
- **World output:** unchanged. Render only.

## Phases (reconciler)

**Phase A.** Trunk lean with leaves left as they are (option A). Covers: `TRUNK_SWAY_AMPLITUDE` and the weight maths in `client/sway.js` with a unit test; log faces taken out of greedy merging and given a base-relative weight in byte 14 in both `crates/mesh` and `viewer/mesh.js`, with a determinism test that overlapping regions agree; the opaque shader program applying the lean with the shared `swayPhase` uniform, zero under reduced motion. Done when the sway and mesh tests pass, the lean of a base log is 0 and grows with height, `./build.sh` succeeds with sizes reported, and FrameTimes on a fixed seed is within the measurable bound above.

**Phase B.** Canopy agreement (option B, or C if the base lookup for leaves proves too costly): leaf vertices also receive the trunk lean for their height above the tree base, plus their own flutter. Waits on open question 1. Done when a test shows a leaf adjacent to a trunk top gets the same lean offset as that trunk top, within 0.005 blocks.

## Open questions

1. **How do leaves get the trunk lean (Phase B)?** Leaf vertices already use byte 14 for `LEAF_WEIGHT`.
   - A. Stay with option A of the idea: leaves keep their flutter only, no extra channel.
   - B. Carry the base-relative height for leaves in byte 15 (free off water) and add it in the cutout shader.
   - C. Option C: lean only leaves within one block of a log, same channel as B.
2. **How is a branch's base found (Phase A)?**
   - A. Height above the lowest log of the connected log cluster within a fixed radius.
   - B. Height above the ground at that column, from the first non-log block below the cluster.
3. **Region-edge trunks.** Should the mesher read padded neighbour blocks for the base scan, or clamp to the region?
   - A. Clamp at the region edge (lean may differ slightly across a seam; only if the test allows it, which the determinism invariant does not).
   - B. Read padded neighbours so the weight depends only on world blocks. Preferred by the invariant.
