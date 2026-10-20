---
title: Viewer rendering quality and speed
status: accepted
scope: both
source: ""
superseded_by: ""
phases:
  A: done
  B: todo
  C: todo
  D: todo
  E: todo
---

## Current state

Checked against `master` (ba3a7e9). Phases B to E are not built and stay `todo`.

- **Phase A is done:** the viewer (PRs #248, #250, #252) and the client (PR #520, ticket #505) both use radial fog with the shared curve in `viewer/fog.js`, trilinear and anisotropic filtering, texel snapping and the tint weight (plan #242).

- **F1** holds: `viewer/index.html` sets `vdist = gl_Position.w` (~line 636)
  and fogs with a linear ramp from 0.55 to 1.15 × `fogFar` (~line 650).
  `renderFly` culls by radial distance against `VIEW.fogEnd`
  (`viewer/flyover/plan.js:574`, `index.html` ~1498); `FLY_FOG` is
  `VIEW.fogEnd / 1.15` (~1291).
- **F2** holds: the viewer's tile array uses `NEAREST` mag and
  `NEAREST_MIPMAP_LINEAR` min (~lines 832 to 833), with no anisotropy.
- **F3** holds: the viewer's shader tests `t.a < 0.9` for the tint (~649).
- **F4** holds in the viewer: it calls `gl.generateMipmap` (~831).
- **F6, F7** hold: `visibilitySchedule` (`plan.js:589`) tests 25 sample
  points per chunk; `meshBlocks` (`viewer/mesh.js:180`) maps arrays per
  face and `tintFor` calls `lerp3` per birch or spruce leaf.
- **Mesh format:** vertex byte 13 is reserved for AO (`mesh.js:10`), as
  Phase C assumes. `meshBlocks(ctx, blocks, tints, size, pad, {fast})` has
  no world-origin parameter yet (Phase B.3).
- **Stats panel** exists (`viewer/index.html`, `?debug=1`), as the
  acceptance criteria assume.
- `coarse_heightmap` is exported from `crates/web/src/lib.rs:53` and used
  by `plan.js` and `viewer/worker.js` (Phase D).

## Reconciliation notes

- **Phase A was incomplete (resolved by PR #520, now `done`): the viewer was done, the client was not**
  (checked after plan #242 closed; PRs #248, #250, #252). `viewer/index.html`
  now has radial fog (`camPos`, `distance(wp, camPos)`, `viewer/fog.js`),
  `LINEAR_MIPMAP_LINEAR` with anisotropy, `textureGrad` texel snapping and
  the tint weight. Still missing for `client/`, which #243 put in scope:
  `client/scene.js:79` still sets `vdist = gl_Position.w` with a linear
  `fogStart` to `fogEnd` ramp (F1; A.1, A.2), and its fragment shader has no
  texel snapping (A.4). F2 and F3 are already fixed in the client. Suggest a
  follow-up ticket for client radial fog, the fog curve and texel snapping,
  then marking Phase A `done`.
- **The `client/` renderer shares `viewer/mesh.js` and has its own
  shaders.** The doc names only `viewer/`. `client/scene.js` imports
  `meshBlocks` (so Phase B.3, C.1 and E.1 change the client's meshes too)
  and has its own shader pair, textures and samplers:
  - Fog uses `gl_Position.w` too (`client/scene.js:82`), so F1 applies
    there. It uses a linear ramp from `fogStart` to `fogEnd`, not the
    viewer's 0.55 to 1.15 ramp.
  - `client/samplers.js` already uses `LINEAR_MIPMAP_LINEAR` and
    anisotropy, so F2 is already fixed in the client.
  - `client/tiles.js` already avoids F3: the tint is a separate R8
    texture array and the colour texture is premultiplied. That is
    Open question 1, option B, already built for the client.
  - `client/tiles.js` still calls `gl.generateMipmap`, and the client
    cutoff has no `fwidth` sharpening. `biome-flora.md` Phase A (A.2, A.4)
    waits on Phase B.1 and B.2 here (see its Reconciliation notes).
  Phases A to C cover `client/` as well (see Scope below).
- **Vertex byte 13 is shared.** Phase C.1 stores AO there. Any other
  design doc that wants that byte must agree; no other doc claims it now.
- **Phase B.3 and determinism.** UV variation by world coordinate affects
  only meshes, not world output. `meshBlocks` needs the world origin
  parameter; the client's calls (`client/scene.js:260, 276`) must pass it
  or the client and the viewer will show different variants.
- **Phase D uses `coarse_heightmap`**, which is tested against full terrain
  in `crates/terrain/src/tests.rs`, so no generator change is needed.
- **Phase E.1, option B (mesher in wasm)** would grow the wasm; CLAUDE.md
  requires gzipped sizes in the PR. `crates/terrain` must stay `no_std`, so
  the mesher would go in `crates/web` or a new crate.
- **Invariants:** the doc keeps to plain ES modules with no framework, as
  CLAUDE.md requires, and changes no world output, so no `stats` diff is
  needed. Say so in each PR.
- **`[manual check]` criteria:** the doc already tags the visual ones.
  Measurable versions: Phase A could compare the fog factor at the frame
  edge with the centre for equal radial distance (a unit test on a
  extracted fog function); Phase D's 10% frame-time bound is measurable
  with the stats panel only on a real GPU, so tag it `[manual check]`
  too.
- **Overlap with other docs:** `biome-flora.md` depends on Phase B.1 and
  B.2 (see above). `basic-gameplay.md` and `world-layout.md` do not touch
  rendering.
- **Scope:** `both`. Phases A to C also apply to `client/scene.js`,
  `client/tiles.js` and `client/samplers.js`, not only to `viewer/`.
  (Decided in #243.)

# Viewer rendering quality and speed

The 3D preview and the flyover (`viewer/index.html`, `viewer/mesh.js`,
`viewer/flyover/`) show two visible defects: moiré and shimmer on block
textures, and half-fogged terrain that ends in hard chunk-shaped edges at
the sides of the frame. This doc fixes both, then adds lighting and
atmosphere and speeds up chunk streaming.

Nothing here changes world output. Every change is in the viewer, so no
block, height, biome or decoration differs for any seed.

## Findings

These were diagnosed by reading the code and confirmed by rendering seed
12345 in headless Chromium (SwiftShader), before and after a trial patch.

### F1. Fog and culling disagree about distance

- The fragment shader fogs by view depth: `vdist = gl_Position.w`.
- `renderFly` culls chunks, and `visibilitySchedule` drops them, by
  straight-line (radial) distance to the chunk box against `VIEW.fogEnd`.
- In the centre of the frame these agree. At the left and right edges of
  the 16:9 flyover view (horizontal half-angle about 44°), radial distance
  is about 1.39 × depth. A chunk dropped at radius 196 is only about 45%
  fogged there.
- Result: pale, partly fogged terrain that ends in a 64-block chunk edge
  with sky behind it. Seed 12345 at t = 50 s shows a mountain on the left
  cut off by a vertical edge.
- **Trial fix, tested:** fog by radial distance, `distance(worldPos,
  camPos)`. The cut mountain fades out correctly, and fog no longer shifts
  when the camera turns.

### F2. Point sampling inside mip levels

- `TEXTURE_MIN_FILTER` is `NEAREST_MIPMAP_LINEAR`. Each mip level is point
  sampled, which aliases at 1 to 2 texels per pixel. Those are the distances
  the flyover spends most of its time at, looking across the terrain at a
  low angle.
- There is no anisotropic filtering.
- **Trial fix, tested:** `LINEAR_MIPMAP_LINEAR` with
  `EXT_texture_filter_anisotropic` at up to 8×. This visibly reduced grain
  on distant grass and stone.
- **Side effect seen:** the sun sprite went soft. Some drivers ignore
  `NEAREST` magnification once anisotropy is on, so close-up pixel art needs
  the shader-side texel snapping in Phase A.

### F3. The tint marker shares the alpha channel

- Greyscale texels that take the climate tint are stored with alpha 200/255.
  The shader tests `t.a < 0.9`.
- Mipmapping and linear filtering average tinted texels (alpha 0.78) with
  untinted ones (alpha 1.0). On mixed tiles such as `grass_side`, filtered
  texels flip between tinted green and raw grey as the camera moves.
- **Trial fix, tested:** treat alpha as a tint weight,
  `tw = clamp((1 - a) / (1 - 200/255), 0, 1)`, and use
  `rgb * mix(1, tint, tw)`.

### F4. Foliage mipmaps lose colour and coverage

- `gl.generateMipmap` averages straight (non-premultiplied) RGBA. The
  transparent texels in leaves and cross plants are `[0,0,0,0]`, so mips get
  darker and their alpha falls.
- The cutout shader discards below alpha 0.5. As mips take over, leaf
  canopies and grass sprites thin out and sparkle.
- Fast leaves (`leaves_fast`) avoid this beyond `FULL_DETAIL` (80 blocks),
  but full-detail chunks closer in still have it.

### F5. Every tile repeats every block

- With F2 fixed, stone walls and grass seen at a low angle still show a
  regular 1-block pattern, because the same tile repeats on every block. No
  filter removes that; the texture has to vary from block to block.

### F6. Chunk visibility uses sample points, not boxes

- `visibilitySchedule` marks a chunk visible when one of 25 points, at the
  surface + 2 and + 10, lies in the view frustum.
- A tall peak or a tree on a hill can be in view while none of its chunk's
  points are, so it loads late and pops in.

### F7. The mesher allocates per face

- `meshBlocks` builds new arrays for every face (`f.c.map`, `p.map`,
  `faceUV`), and `tintFor` calls `lerp3` per birch or spruce leaf block.
  Chunks have 10,000 to 60,000 faces, so this garbage dominates the
  workers' time.

## Phase A: fog and texture filtering

Land the tested fixes for F1 to F3, and make close-ups crisp again.

1. **Radial fog.**
   - Add a `camPos` uniform. The vertex shader computes the world position
     once and sets `vdist = distance(wp, camPos)`.
   - `draw()` and `renderFly()` set `camPos` to the eye position in the
     same coordinates as the mesh offsets.
2. **Fog curve.** Replace the linear ramp from 0.55× to 1.15× `fogFar`
   with a smoothstep (or exp²) that reaches exactly 1 at `VIEW.fogEnd`.
   Keep `FLY_FOG` derived from `VIEW`, as the flyover README requires.
3. **Trilinear and anisotropic filtering.** `LINEAR_MIPMAP_LINEAR`, and
   anisotropy `min(8, MAX_TEXTURE_MAX_ANISOTROPY_EXT)` when the extension
   is present.
4. **Crisp pixels up close.** Sample with `LINEAR` everywhere, and snap
   UVs to texel centres in the fragment shader, using `textureGrad` with
   the unsnapped derivatives so mip selection doesn't change:

   ```glsl
   vec2 px = vuv.xy * 16.0;
   vec2 fw = max(fwidth(px), vec2(1e-4));
   vec2 snapped = (floor(px) + 0.5 + clamp((fract(px) - 0.5) / fw, -0.5, 0.5)) / 16.0;
   vec4 t = textureGrad(tex, vec3(snapped, vuv.z), dFdx(vuv.xy), dFdy(vuv.xy));
   ```

5. **Tint weight.** Apply the F3 change.

**Acceptance**
- Flyover seed 12345 at t = 50 s: no partly fogged terrain ending in a
  straight chunk edge. Compare before and after frames. `[manual check]`
- Preview with `?seed=12345&dist=60&pitch=0.12`: the sun and nearby block
  textures stay hard-edged; distant grass shows no grey speckle.
  `[manual check]`
- `preview.mjs` still plans a route.

## Phase B: texture variation and foliage mipmaps

Fix F4 and F5.

1. **Build mipmaps in JS** in `loadTextures` instead of
   `gl.generateMipmap`, upload each level with `texImage3D`/`texSubImage3D`:
   - average colour weighted by alpha, so transparent texels add no black;
   - **coverage-preserving alpha:** scale each level's alpha so the share
     of texels at or above 0.5 matches level 0 for that layer;
   - average the tint weight separately from coverage (see Open question
     1).
2. **Alpha to coverage** for the cutout pass: enable
   `SAMPLE_ALPHA_TO_COVERAGE` and sharpen alpha in the shader,
   `a = (a - 0.5) / max(fwidth(a), 1e-4) + 0.5`, in place of the hard
   discard. The context already requests MSAA.
3. **Per-block UV variation.** In `meshBlocks`, pick one of the 8
   rotations or mirror images of the face UVs for top faces of grass,
   sand, gravel, stone and snow, and mirror only (no rotation) for side
   faces, so their tops stay at the top. Choose it with a hash of the
   block's **world** coordinate:
   - `meshBlocks` needs the world origin of the area (the preview passes
     `wx, wz`; the worker passes `cx * CHUNK, cz * CHUNK`);
   - otherwise the pattern would change at chunk borders and between the
     preview and the flyover.
   - Grass sides keep their top strip aligned, so mirror them horizontally
     only.

**Acceptance**
- Forest canopies at 40 to 80 blocks keep their mass and don't sparkle as
  the camera moves. `[manual check]`
- No regular 1-block pattern on large stone or grass areas at low angles.
  `[manual check]`
- The same block shows the same UV variant in the preview and in a
  flyover chunk (unit test on `meshBlocks` with two origins covering the
  same world block).

## Phase C: lighting and atmosphere

1. **Vertex ambient occlusion.** For each face corner, count the 3
   neighbouring blocks (two sides and the diagonal) on the face's open side
   and store a level 0 to 3 in vertex byte 13 (currently padding).
   - Shade multiplier about `[1.0, 0.82, 0.68, 0.55]`.
   - Flip the quad's triangle diagonal when `ao[0] + ao[2] < ao[1] + ao[3]`,
     so the gradient doesn't crease.
   - The pad ring already gives correct neighbours across chunk borders.
2. **Sky gradient.**
   - Draw the sky as a full-screen pass: horizon colour toward the
     horizon, a slightly deeper colour at the zenith, warmer toward
     `SUN_DIR`.
   - Fog toward the sky colour in the fragment's view direction, not one
     flat `fogColor`, so fogged hills blend into the horizon band.
   - Keep the per-biome sky ramps as the base colour.
3. **Water.**
   - Colour and alpha by depth to the floor (the map's `terrainColor`
     already does this for its tiles).
   - Fresnel: blend toward sky colour at low angles. The sea surface is
     the plane y = 63, so the view angle is cheap to compute.
4. **Fog in linear colour.** Convert to linear before mixing with fog,
   back to sRGB after. Optional; judge by eye.

**Acceptance**
- Inner corners, the ground under trees and terrace steps visibly darken.
  `[manual check]`
- No AO seams at chunk borders in the flyover.
- Faces per chunk unchanged (AO only adds data to existing vertices).

## Phase D: distant terrain

Show terrain beyond the fog instead of empty sky.

1. Build a low-detail heightmap mesh from `coarse_heightmap` (the
   flyover's survey already samples every 8 blocks): one quad per sample,
   coloured from the biome's surface colour and water depth, with a skirt
   so its edges don't show.
2. Draw it before the chunks, with fog, and only beyond the chunk radius:
   discard fragments closer than about `VIEW.fogEnd - 16`, or offset its
   height down a little, so it never pokes through loaded chunks.
3. Push the fog's end further out for this layer only (for example 500
   blocks), so the far horizon shows mountains that fade out gently.
4. The visibility schedule and `VIEW.fogEnd` for chunks stay as they are.

**Acceptance**
- The flyover horizon shows distant land and mountains instead of a flat
  sky line. `[manual check]`
- No z-fighting or seams where the low-detail mesh meets loaded chunks.
- Frame time in the flyover rises by no more than about 10% (measure with
  the stats panel).

## Phase E: streaming speed

1. **Allocation-free mesher (F7).** Precompute face corner and UV tables
   per face and box shape; write vertices straight into the `Int16Array`
   and `Uint8Array` views; compute tints once per column, not per block.
   Moving `meshBlocks` into the wasm crate is the bigger option (see Open
   question 3).
2. **Greedy meshing for water tops**, then other opaque faces. Merge
   neighbouring faces with the same layer, shade, tint and AO.
   - UVs are uint8 in 1/16 block and the array uses `REPEAT`, so a merged
     quad can span up to 15 blocks.
   - Water tops at y = 63 are untinted and flat, so oceans collapse to a
     handful of quads.
   - Grass and leaves vary their tint per column, so they rarely merge;
     leave them alone.
3. **Box visibility in the schedule (F6).** Test each chunk's box (survey
   minimum to maximum height, plus tree clearance) against the frustum and
   `VIEW.fogEnd`, instead of 25 points.
4. **Half-resolution far chunks.** Beyond about 120 blocks, mesh chunks
   from 2×2×2 downsampled blocks (most common non-air block wins), stored
   with the same vertex format at double scale. About a quarter of the
   faces, and the fog hides the loss of detail. Swap to full detail as
   `full` does today for fast leaves.

**Acceptance**
- Median chunk mesh time per worker job falls by at least 2× (stats
  panel, same seed and route).
- An ocean chunk's water part has under 5% of its current face count.
- No chunk pops in visibly inside the fog distance on seeds 12345, 9, 42,
  2026, 7 and 100. `[manual check]`
- README's "Things that took a while to find out" gets a note for each
  surprise found along the way.

## Open questions

1. **Where should the tint marker live?**
   A. Keep it in alpha as a continuous weight (Phase A). Simple, but
   foliage coverage and tint share one channel, which constrains Phase B's
   mipmaps.
   B. A separate R8 tint-mask texture array sampled with the same UVs. One
   extra texture fetch; frees alpha for coverage only.
2. **Supersampling for recordings.** Render at 1.5× or 2× when recording
   and downsample, to clean up block silhouettes against the sky?
   A. Yes, recording only.
   B. Yes, a viewer setting.
   C. No.
3. **Mesher language.** A. Keep it in JS and remove the allocations.
   B. Move it to Rust/wasm, which shares code with the eventual game client
   but grows the wasm (sizes must be reported in PRs).
4. **Distant-terrain colour.** A. Biome surface colour only. B. Sample the
   decorated map tiles where they exist (more accurate near the map, but
   only covers the map's 1024×1024 blocks).
