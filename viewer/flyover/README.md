# Flyover video

The 3D view's **flyover** mode makes a short film of the current map. It opens
on an island, just above the water (a raft's-eye view), rises over the island
and heads out across the sea, passing the most interesting terrain on the
map. It ends with a climb and a look back. The route and the camera are drawn
on the map. You can play the film in the page or record it to a video file.

```
./build.sh --serve      # then open http://localhost:8000/?seed=2026&fly=1
node viewer/flyover/preview.mjs 2026 0 0 route.ppm   # plan only, no browser
```

| File | Does |
|---|---|
| `plan.js` | Pure planning (runs in Node, a worker or the page): survey, island choice, features, tour, camera path, visibility schedule, framing check |
| `player.js` | Loads the chunks the schedule says are coming into view, runs the clock, records video |
| `preview.mjs` | Prints a plan and draws the route over the survey as a PPM image |
| `../worker.js`, `../pool.js` | Background workers and their shared priority queue: map tiles, plans, chunks |
| `../index.html` | The 3D view's flyover mode: frames, captions, loading card, route on the map, recording |

## How it works

1. **Background preparation.** Each time the map changes, the page asks a
   worker for a plan of that map (`prepareFlyover`). When the plan arrives, a
   `FlyoverPlayer` starts rendering the chunks for the first 10 seconds.
   - **Priority:** the plan and chunk jobs run after the map tiles.
   - **Starting:** the toggle plays at once if those chunks are ready.
     Otherwise it shows a "loading flyover render… n/m" card and starts when
     they're in.
   - **Switching back:** going back to chunk preview only pauses the flight,
     so returning resumes it.
2. **Survey** (`survey`): `coarse_heightmap` samples a 2048×2048-block square
   centred on the map, one sample every 8 blocks. Each sample gives terrain
   height, biome, and the count and height of overhang gaps. It takes about
   0.6 s.
3. **Islands** (`findIslands`): land components that don't touch the survey's
   edge, between 1,200 and 60,000 m². Each gets a score: size (best around
   16,000 m²), relief, woodland share, biome variety, open water around it,
   and a penalty for snow and for distance from the map.
4. **Features** (`findFeatures`) over the whole survey:
   - **peaks:** a local maximum over 24 blocks with at least 14 blocks of
     prominence within 48;
   - **cliffs:** a drop of 12 or more blocks between samples, clustered;
   - **overhangs and floating rock:** samples with an air gap of 3 or more
     blocks above sea level;
   - **channels:** water with land close on two opposite sides, at least 48
     blocks long;
   - **lakes:** water that doesn't reach the survey edge;
   - **biomes:** the middle of large deserts, tundra, taiga, rainforest and
     swamp.
5. **Choice, kept on the map** (`planData` with `bounds`).
   - **On the map:** only islands entirely on the map and features on it
     count, and the whole route must stay on it (up to six islands are
     tried).
   - **Scoring:** each island's score includes the features within 420 blocks
     of it. The winner takes up to four features: best first, at least 110
     blocks apart and at most two of a kind.
   - **Fallback:** if no route fits, it plans freely and returns `moved` and
     `mapOrigin`, and the page moves the map to the route when you open the
     flyover. In every case I tried (seven maps), a route fit.
6. **Route** (`keyframes`): an open tour from the island, built by nearest
   neighbour and then 2-opt. Each kind of feature has its own shot:
   - fly down a channel low over the water;
   - stand off a cliff or overhang on its open side and slide across it;
   - pass a peak from 75 blocks out, near summit height, so it shows against
     the sky;
   - skim across a lake;
   - glide over a biome.

   Each feature shot carries a `look` point, the place it must show.
7. **Path** (`buildPath`):
   - **Shape:** centripetal Catmull-Rom through the keyframe eyes and targets.
   - **Clearance:** a second survey, every 4 blocks over the route, sets the
     minimum height (+7 blocks where trees may stand). The lift is a running
     max followed by a Gaussian, so climbs start early.
   - **Timing:** each keyframe has its own speed nearby, with a 30 blocks/s
     cruise between features and eased start and end.
   - **View direction:** smoothed, and its pitch clamped to about −25°…+9°.
     Within 1.5 s of each feature shot the camera then turns fully onto the
     shot's `look` point, easing back out by 3.5 s.
8. **Framing check** (`featureCoverage`): for each feature, the planner
   measures how long its point is in the middle 80% of the frame, within 140
   blocks and not hidden by terrain. A feature in view for under 2.5 s is
   swapped for the next candidate (twice at most). Each plan carries the
   result (`plan.coverage`, with the `first` second each feature is in view),
   and the page lists it under the view as chapters you can seek to.
9. **Visibility schedule** (`visibilitySchedule`): from the path sampled every
   0.25 s, each 64×64 chunk whose box meets the 16:9 view frustum and lies
   within `VIEW.fogEnd` of the eye (straight-line distance to the box) gets a
   `first` and `last` time in view, padded by 1 s. The box runs from the
   survey's lowest to highest height over the chunk (at least sea level) plus
   `TREE_CLEARANCE` (24 blocks: the survey sees only ground, and the tallest
   trees stand well over 10 blocks above it; a loose box costs a few extra
   chunks, a tight one pops trees in). Chunks within `near` blocks always
   count. That's 72–141 chunks
   per flight. The player loads chunks in order of `first`, keeps 8 s ahead,
   and frees each chunk 2 s after its `last`. The clock only runs while
   everything in view now is loaded (after a start or seek, everything for
   the next stretch), so the film never shows holes. While it waits, the
   recorder is paused too.
10. **Frames:**
    - **Rendering:** frames render into the hidden WebGL canvas at
      1280×720. Fog is by straight-line distance from the eye (`camPos`, the curve is
      in `viewer/fog.js`): a smoothstep from half of `VIEW.fogEnd` that
      reaches full sky colour at exactly `VIEW.fogEnd` (196 blocks).
      The sky follows the biome under the camera.
    - **Captions:** each frame is copied into the `#flyout` 2D canvas, which
      adds the captions and fades.
    - **Recording:** `#flyout` is what `MediaRecorder` records (H.264 MP4 in
      Chrome and Safari, otherwise WebM).

## Things that took a while to find out

These cost the most time while building the feature. Read them before
changing it.

### Islands in this terrain are small and flat

My first idea was a tour of the island itself. A full scan of seed 12345
(2048² blocks around the origin) showed why that wouldn't work. The world is
**66% land**: Beta-style terrain is mostly continents with seas between
them. Only 29 land components over 300 m² don't touch the scan's edge. The
largest is about 16,000 m², roughly 126 blocks across, and **the highest
point on any island is y = 77**. Other seeds are the same. The terrain that
looks good on film (cliffs of 30–50 blocks, peaks around y 95–115,
overhangs) is almost always on the mainland. So the island is the opening
shot and the tour goes out from it. An island is only "appropriate" if
there's something worth flying to from it, which is why `rankIslands` counts
nearby features.

### A full-resolution survey is far too slow; the density lattice is not

`heightmap` over 2048² blocks takes **7.2 s** in wasm, too slow for a button.
The terrain is defined on a **4×8×4 lattice** (SPEC 3.1). At a column that
sits on a lattice corner, trilinear interpolation reduces to linear
interpolation in y alone. So the height there can be read off the 17 lattice
densities exactly.

Making this cheap needed only a small change:
- `OctavePerlin::fill_grid` already takes arbitrary coordinate lists, so the
  lattice builder was generalised to take a stride (`lattice_grid(.., step)`).
- `generate_coarse_heightmap` walks each sampled lattice column.

At 8-block steps a 2048² survey takes **~0.5 s**, about 14× faster. The test
`coarse_heightmap_matches_full_terrain` checks its heights and gap counts
against `generate_region`, allowing a 2% mismatch where the surface pass
turns a top block at sea level into water. The same survey also draws the
map's rough draft.

### Overhangs don't show in any heightmap

A heightmap only stores the top block, so arches and floating rock are
invisible in it. The survey's walk down each column was already there, so it
also counts air gaps above sea level that have solid ground both above and
below them (`gaps`, `gap_h`).

### After changing Rust, rebuild the wasm before trusting any output

Partway through, the survey grew from 2 to 4 bytes per sample, but
`viewer/pkg` was still the old build. Read as 4-byte records, the old output
came out as a map with **half of it ocean at height 0 and the other half all
"overhang"**. If a survey map looks banded or split in half, run
`./build.sh` first.

### Mesh memory decides the view distance

Decorated 64×64 chunks have **10,000 to 60,000 faces each**, about 25,000 on
average. The old vertex format (240 bytes per face) would need 6–14 MB per
chunk. `mesh.js` writes **quads with 16-byte vertices**, 64 bytes per face:
- int16 positions in 1/16 block;
- uint8 uv, layer and shade;
- normalised uint8 tint.

The shader rescales them with an `unpack` uniform and places each chunk with
an `offset` uniform. Every quad has the same index pattern, so the page
keeps **one shared index buffer** (`quadIndices`) instead of 24 bytes of
indices per face in every chunk.

### Fast leaves in the distance

Leaves are most of a forest chunk's faces (70% on seed 12345), and three
quarters of leaf faces face another leaf. Each schedule entry has a `full`
time: the first time the camera comes within `FULL_DETAIL` (80) blocks of
the chunk. Until then the chunk is meshed with **fast leaves**
(`meshBlocks(.., { fast: true })`):
- leaves hide their neighbours like solid blocks;
- they use `leaves_fast`, the leaves tile with its holes filled in dark
  green;
- they join the opaque part.

That's about 14,000 faces instead of 32,000 for a forest chunk, and 0.9 MB
instead of 2 MB. The player requests the full-detail mesh `LOOKAHEAD`
seconds before `full` and swaps it in when it arrives. The clock waits for
it, as it waits for any chunk.

### Drawing order and culling

Each frame `renderFly` draws only the chunks whose box is inside the view
frustum and nearer than solid fog. They're drawn nearest first:
1. the opaque parts, with a shader that never discards, so the GPU can
   reject hidden pixels before shading them;
2. the cut-outs (leaves, plants);
3. water and ice, back to front.

### The far terrain is a separate program with its own fog

`renderFly` draws the survey's far mesh (`farmesh.js`) after the sky and
before the chunks, with `FAR_FOG_END` (500) instead of `VIEW.fogEnd`. The
fragment shader discards anything whose horizontal distance from the eye is
under `VIEW.fogEnd - 16`, so the chunks never fight it for depth; the
chunks' culling and fog are untouched. The far mesh also has to be drawn
with face culling off (its skirt faces outward, its surface faces up, and
the winding was not checked against GL's convention). For now it is built
once from the plan's 2048-block survey, so it only reaches 500 blocks while
the camera is within about 500 blocks of the survey's edge.

### Chunk seams: mesh with a one-block ring

Meshing each chunk on its own draws walls of water and stone along every
border. Each worker generates 66×66 columns and meshes only the inner 64×64
(`meshBlocks(.., size, pad = 1)`). This is safe because a block depends only
on the seed and its world position, so the ring matches the neighbour
exactly.

### Ray-tracing for occlusion saves nothing; limiting to the view does

I measured both against the first player's "everything within 200 blocks of
the next 6 s of path":

| Seed | Within 200 blocks | In the view frustum | In view and not hidden by terrain |
|---|---|---|---|
| 12345 | 182 | 150 | 150 |
| 9 | 153 | 110 | 110 |
| 42 | 154 | 117 | 117 |
| 2026 | 150 | 80 | 80 |
| 7 | 206 | 139 | 139 |
| 100 | 186 | 114 | 114 |

The occlusion test ray-marched a 5×5 grid of points per chunk over a 4-block
height survey. The camera flies above the ground looking at most about 25°
down, so some part of nearly every chunk in view is visible, and occlusion
removed nothing. The frustum alone saves 18–47%. Rendering the whole flight
up front doesn't fit in memory either: 80–150 chunks at ~2 MB is
160–300 MB. Hence the schedule: render the opening in the background, stream
the rest just ahead.

### "Points at the feature" has to be measured, not assumed

The first paths aimed only through the keyframe targets, then smoothed the
view direction heavily. Measured over 10 seeds (40 features):
- several peaks were never closer than 18–26° to the frame centre;
- five features were in good view for only 2.1–2.5 s;
- an overhang in seed 77 got 0.6 s.

The explicit aim (step 7) brought every feature to 0–1° from centre with
2.8–19 s in view. The swap rule catches the rest: seed 77's hidden overhang
is replaced by a peak. `featureCoverage` is the check to rerun after
changing any shot.

The check is geometric, though. Centred isn't the same as well composed:
- A peak shot from almost on top didn't read as a peak, hence the 75-block
  stand-off.
- An overhang under a forest is seen from above, because clearance has to
  lift the camera over the trees.
- A "peak" that is a tree-covered hill is dull however it's framed.

### Two playback bugs from the background opening

- **The opening never finished (stuck at 21 of 24 chunks).** The player's
  per-frame update only looked 8 s ahead, so it cancelled the opening jobs
  for chunks first seen between 8 and 10 s. Updates now keep the whole
  gated stretch while waiting to start.
- **Clicking the toggle while the page was still starting** prepared a
  flyover for the placeholder seed. The real map then replaced it, and
  `startFlyover` gave up. It now waits for start-up, and re-asks if the map
  changed while it waited.

### Workers: real parallelism, slow headless threads

Generating the map's four tiles in four Node worker threads took 998 ms
against 3,823 ms on one thread, so the pool is worth it. Headless Chrome on
a loaded 8-core Mac was a poor guide to real speed:
- four workers did four times the work in the same wall time, so they do run
  in parallel;
- each thread ran 5–8× slower than Node;
- the first jobs in fresh workers were slower still, about 21 s for the
  first map against 8 s once warm;
- passing one compiled `WebAssembly.Module` to every worker (now done)
  didn't change that.

Judge speed in a normal browser window.

### Chunk generation needs the page's tile table

The workers can't call the page's `tileFor()`, because the page owns the
texture array. The page turns it into a 768-entry face-to-layer table
(`faceLayers`) and sends it to each worker once, with the compiled wasm
module.

### Recording: MediaRecorder stamps frames with wall-clock time

The first player capped each frame's time step at 50 ms. On a slow page the
path then advanced more slowly than real time, and the video came out in slow
motion (17.7 s for 10.7 s of flight). The clock now follows real time and
only caps hitches over 250 ms. Stalls are cut out by pausing the recorder.
After the fix, 11 s of flight recorded as a 10.1 s 1280×720 MP4. A
frame-exact export would need WebCodecs `VideoEncoder` and a muxer, which
isn't done here.

### Captions have to be composited

`captureStream` records a single canvas, and WebGL has no text. Each frame
is drawn into a 2D canvas in the same task (so `preserveDrawingBuffer` isn't
needed), and the captions go on top.

### Pacing: first plans were 3–5 minutes long

Blending speeds only between neighbouring keyframes made long transits
crawl, so flights came out at 170–290 s. Two changes fixed it:
- a `cruise` speed (30 blocks/s) away from keyframes;
- per-shot speeds raised by 1.4×.

Feature scores also needed caps: a large overhang cluster once scored 5,399.

### The box test is not a superset of the old point test

On seeds 12345, 9 and 42 every chunk the old 25-point test scheduled is still
scheduled, but a few intervals got shorter (by 0.25 to 0.5 s, and once 3.9 s
on seed 9). The new distance limit is 3D, so a chunk far below a high camera
now drops out sooner; the old one measured only the horizontal distance.

### Testing in a browser without the extension

The Claude-in-Chrome extension wasn't connected, and there's no Puppeteer.
The working setup:
- launch the installed Chrome with `--headless=new --remote-debugging-port=N`;
- drive it over the DevTools protocol with Node 22's built-in `WebSocket`
  (`Page.navigate`, `Runtime.evaluate` with `awaitPromise`,
  `Page.captureScreenshot`);
- for stills at chosen times, `window.flyover()` returns the prepared
  flyover once the view is in flyover mode. Wait for its `player`, then call
  `seek(t)` and `play()`, wait for `state === 'playing'`, pause, and save
  `#flyout.toDataURL()`.

Give scripted runs generous timeouts (see the previous section).

## Tuning

- **Which island and features:** `findIslands`, `chooseFeatures` (`max`,
  `radius`, `gap`) and the map margins in `planData`.
- **What counts as a feature:** the thresholds in `findFeatures`.
- **The shots:** `keyframes` (offsets, heights, speeds, `look` points) and
  `buildPath` (`clearance`, `cruise`, pitch clamps, aim window).
- **The framing bar:** `MIN_VIEW` in `planData`, and `featureCoverage`'s
  `far` and `frac`.
- **Biome colour blending:** flyover chunks blend tints over a radius of
  2 (`FLYOVER_BLEND` in `player.js`), not the legend's 3 (a 5x5 window
  instead of 7x7). The camera is far from the ground, so the smaller window
  is hard to tell apart and cheaper to compute. Every flyover chunk uses
  the same radius, so neighbours meet without a seam.
- **Streaming:** `OPENING`, `LOOKAHEAD` and `BEHIND` in `player.js`, and
  `VIEW` in `plan.js`. `VIEW` must match the renderer, so `FLY_FOG` in
  `index.html` is derived from it.

## Limits

- The survey's 4-block clearance grid can miss a thin spire or an unusually
  tall tree near the camera.
- Fish aren't shown in the flyover.
- Water animation ignores `prefers-reduced-motion` here, because the flight
  only plays when someone asks for it.
- Text seeds work in the viewer (they are hashed). `preview.mjs` takes
  numeric seeds only.
