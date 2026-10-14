# minceraft

The world generator for **Mince Raft**: a Beta-1.7.3-style overworld terrain
generator written in Rust, compiled to WebAssembly, with a small browser
viewer that renders it using the game's own 8×8 tileset.

The generator is a clean-room implementation written from [`SPEC.md`](SPEC.md),
a prose description of the algorithm. No Mojang code or assets are in this
repository (see [Clean-room rules](#clean-room-rules)).

## Quick start

```sh
./build.sh --serve        # build the wasm, then serve the viewer
# open http://localhost:8000
```

Requirements: Rust with the `wasm32-unknown-unknown` target, `wasm-pack`,
and `wasm-opt` (binaryen). On macOS:

```sh
rustup target add wasm32-unknown-unknown
arch -arm64 brew install wasm-pack binaryen
```

`build.sh` options:

| Command | Does |
|---|---|
| `./build.sh` | export textures, build the wasm, run `wasm-opt -Oz`, print its size |
| `./build.sh --test` | run `cargo test` first |
| `./build.sh --serve` | serve `viewer/` on port 8000 afterwards |

## Repository layout

```
SPEC.md               algorithm spec the generator is implemented from
build.sh              build script (see above)
art/
  PROMPT.md           prompt and palette for the pixel-art textures
  batches_*.json      the tileset, one file per group of batches
  tools/              sheet builder, validator, export_textures.py
crates/
  terrain/            the generator: no_std + alloc, zero dependencies
    examples/stats.rs native timing, terrain statistics, PPM heightmap
  web/                wasm-bindgen wrapper used by the viewer
viewer/
  index.html          single-file viewer (no frameworks)
  pkg/                build output (gitignored)
  textures.json       full tileset exported by build.sh (gitignored)
```

## The terrain crate

```rust
use terrain::{generate_region, Block};

let region = generate_region(seed, x0, z0, width, depth);
// region.blocks: Vec<u8>, region.height == 128
// index = ((z * width) + x) * 128 + y
```

- **Blocks:** `Air = 0, Stone = 1, Dirt = 2, Grass = 3, Sand = 4, Gravel = 5, Water = 6`.
- **Determinism:** a block's value depends only on the seed and its world
  coordinate, never on the region origin or size. Adjacent or overlapping
  regions always agree.
- **Heightmaps:** `generate_heightmap` returns the top block and height per
  column without building the full block array.
- **Density bias (extension point):** `generate_region_with_bias` takes a
  list of `DensityBias` terms (any `Fn(i32, i32, i32) -> f64`). Their sum is
  added to the terrain density just before the solid/air test, in world block
  coordinates. Positive values add land and negative values carve it away.
  This is where island bumps, spawn water and route channels will go.

What it generates: 3D-noise terrain with overhangs and floating chunks,
sea level at y = 64, grass and dirt over stone, sand and gravel beaches
around y = 60–65, and sandy deserts. Caves, ores, trees, lakes and other
decoration are out of scope.

### Tests and native tools

```sh
cargo test                     # noise ranges, determinism, overlap, sea fill
cargo run -p terrain --profile bench-native --example stats -- <seed> <x0> <z0> [out.ppm]
```

## The viewer

`viewer/index.html` loads the wasm module and shows:

- **Map:** a 512×512 top-down view, coloured by top block, shaded by slope
  and height, with water tinted by depth. Hover to see a column's block and
  height.
- **3D view:** a WebGL2 render of a 64×64 area. Click the map to move it,
  drag to orbit and scroll to zoom. Schools of fish swim in the sea (see
  [Fish](#fish)).
- **Stats:** map generation time and the `.wasm` size, raw and gzipped.

URL parameters: `?seed=9&x0=100000&z0=100000&sx=150&sz=190&dist=70&yaw=0.8&pitch=0.15`
(`sx`/`sz` place the 3D area within the map; `dist`, `yaw` and `pitch` set
the camera). Text seeds are hashed to a number.

### Textures in the 3D view

The viewer loads the full Mince Raft tileset. `build.sh` runs
`art/tools/export_textures.py`, which:

- merges every `art/batches_*.json` file in order;
- adds the palette from the table in `art/PROMPT.md`;
- checks each grid and fails on unknown palette indices or duplicate names;
- writes everything, including the creature and person models, to
  `viewer/textures.json`.

The 3D view currently uses these tiles from it:

| Block / object | Top | Sides | Bottom |
|---|---|---|---|
| Grass | `grass_top` | `grass_side` | `dirt` |
| Dirt, stone, sand, gravel | own tile | own tile | own tile |
| Water | `water_0`–`3`, animated | `water_flow_0`–`3`, animated | `water_0`–`3` |
| Sun | `sun_sky`, a sprite low in the north-west sky | | |
| Fish | salmon, tuna and mackerel models from Batch 11 | | |

How it works:

- At startup the viewer turns those tiles from palette strings into RGBA and
  uploads them as one `TEXTURE_2D_ARRAY` with 16×16 layers. 8×8 tiles are
  scaled up 2×; the 16×16 sky tiles fit as they are. `.` pixels are
  transparent, and the shader discards them.
- Model faces get one layer each at their native size, in the layer's
  top-left corner. The rest of the layer repeats the face's edge pixels so
  mipmaps don't bleed transparency into small faces.
- Each vertex carries a UV, a layer index and a shade. The water and
  flowing-water animations are runs of four layers. Every 300 ms the shader
  adds the current frame (0–3) to their layer.
- The sun is a camera-facing sprite, drawn first with no fog or depth so the
  terrain covers it. The orbit camera looks down, so orbit low (or use a small
  `pitch`) to see it.
- Filtering is nearest-neighbour up close, with mipmaps so distant faces
  don't shimmer.
- If `textures.json` is missing, the viewer falls back to flat colours and
  says so above the 3D view.

To change a texture, edit the `art/batches_*.json` file that holds it and
rerun `./build.sh`. To use another tile in the 3D view, add its name to
`TILES` in `viewer/index.html`. Block faces are assigned in `tileFor()`. A
tile must be square with a side that divides 16 (4, 8 or 16).

The rest of the tileset isn't drawn yet. That covers the raft and
workstation blocks, crops, items, particles, moon phases, inventory UI, and
the squid, player and villager models.

### Fish

Each time the 3D area is built, the viewer spawns schools of 3–5 salmon,
tuna or mackerel. It aims for one fish per ~70 sea-surface columns, up to 40
in all.

- **Models:** each fish is its Batch 11 model, following the face
  conventions in `art/PROMPT.md`. The body is a box; the tail and dorsal fin
  are flat planes drawn from both sides. The right side reuses the left-side
  texture, mirrored. One model pixel is 1/8 block, so a salmon is one block
  long.
- **Swimming:**
  - Each school picks a new heading every few seconds.
  - Fish steer toward their school's heading, drift back if they stray, and
    keep a little space between schoolmates.
  - A fish checks the block just ahead of its nose. If that's land, or above
    the water surface (y ≈ 63.3), the whole school turns around.
  - Each fish wiggles its body and sweeps its tail.
- **Drawing:** fish are drawn after the terrain and before the translucent
  water, so the sea tints them. Water opacity is 0.45, low enough to see them.
- **Reduced motion:** if the browser asks for it, fish and water stay still
  and the scene only redraws when you move the camera.

Fish movement uses `Math.random`, so it isn't tied to the world seed. It's
viewer-only; the terrain crate knows nothing about fish.

## Performance

For a 512×512 region (all 128 blocks of height):

| | Time |
|---|---|
| Native, release | ~185 ms |
| wasm in Node, warm | ~280 ms |

The `.wasm` is about 31 KB raw and 12 KB gzipped. The terrain crate is built
at `opt-level = 3` while the rest of the release profile uses `"z"`. That
costs about 6 KB but makes generation about 40% faster.

## Clean-room rules

- `./work/` holds the unpacked Beta jar, decompiled classes and notes used
  to write `SPEC.md`. It is gitignored and must never be copied out.
- `SPEC.md` describes the algorithm in prose and formulas only; no Java was
  pasted or transliterated into it.
- The Rust code was written from `SPEC.md` alone, by a fresh agent that did
  not have access to `./work/`.
- `*.jar` is gitignored.
- Exact seed parity with Beta is not a goal. The spec lists the original's
  quirks and which ones were deliberately left out.
