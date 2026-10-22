# Beta-style Overworld Terrain — Algorithm Specification

This document describes, in prose and math, how the Minecraft Beta 1.7.3
overworld shapes its terrain: noise, density field, interpolation, sea fill
and surface layering. It is written so the generator can be reimplemented
without reference to any original code. Bit-exact seed parity is **not** a
goal; where the original has quirks, each is described along with a
recommendation on whether to reproduce it.

Coordinate convention: X and Z are horizontal, Y is vertical (0 at the
bottom, 127 at the top). The world is 128 blocks tall.

---

## 1. Building blocks

### 1.1 Random number source

The original seeds a single linear-congruential generator with the world
seed and draws from it, in a fixed order, to build every noise generator.
Any good seeded PRNG works here (e.g. SplitMix64 / PCG / xorshift). Each
noise generator draws, per octave:

1. three uniform doubles in [0, 1), each multiplied by 256, which become
   that octave's X, Y and Z coordinate offsets;
2. a permutation of 0..255 (see 1.2).

Creation order in the original terrain generator (all from one RNG stream,
seeded with the world seed): low-main (16 octaves), high-main (16),
selector (8), sand/gravel (4), stone-depth (4), scale (10), depth (16).
The original then builds one more 8-octave generator, the tree-density
noise used by population (section 10.3). The three climate generators have their own RNGs,
seeded with `seed * 9871`, `seed * 39811` and `seed * 543321`.

An implementation may derive each generator's RNG independently from
`(seed, generator_id)`; ordering only matters for parity.

### 1.2 Improved Perlin noise (single octave)

This is Ken Perlin's 2002 "improved noise".

**Permutation table.** Start with the identity array `p[i] = i` for
i in 0..255. For i = 0..255, pick a uniform random index j in [i, 255],
swap `p[i]` and `p[j]`. Then duplicate it: `p[i + 256] = p[i]`, giving a
512-entry table so that hashed indices up to 511 need no masking.

**Offsets.** Before evaluation, the octave's own random offsets (ox, oy, oz),
each in [0, 256), are added to the input coordinates.

**Evaluation at (x, y, z)** (after offsets are added):

1. Integer cell: `X = floor(x)`, `Y = floor(y)`, `Z = floor(z)`; the
   wrapped indices are `X & 255`, etc.
2. Fractional parts: `fx = x - X`, `fy = y - Y`, `fz = z - Z`, in [0, 1).
3. Fade curve on each fractional part:
   `fade(t) = 6t^5 - 15t^4 + 10t^3`.
4. Corner hashing (classic improved-Perlin scheme, using the wrapped
   indices):
   - `A  = p[X] + Y`, `AA = p[A] + Z`, `AB = p[A + 1] + Z`
   - `B  = p[X + 1] + Y`, `BA = p[B] + Z`, `BB = p[B + 1] + Z`
   - the eight corner hashes are `p[AA]`, `p[BA]`, `p[AB]`, `p[BB]` for the
     z-low corners and `p[AA+1]`, `p[BA+1]`, `p[AB+1]`, `p[BB+1]` for the
     z-high corners. (A-prefixed is x-low, B-prefixed x-high; the second
     letter A/B is y-low/y-high.)
5. Gradient dot product for hash h and offset vector (dx, dy, dz) from the
   corner to the sample point: take `h & 15`.
   - first term u = dx if h < 8, else dy;
   - second term v = dy if h < 4; else dx if h is 12 or 14; else dz;
   - result = (±u) + (±v), where u is negated if bit 0 of h is set and v is
     negated if bit 1 is set.
   This is the standard 12-edge-gradient set (with 4 duplicates to reach 16).
6. Trilinear blend: lerp along x with weight fade(fx), then along y with
   fade(fy), then along z with fade(fz). `lerp(t, a, b) = a + t(b - a)`.

Output range is roughly [-1, 1].

**2D usage.** Several generators only need a 2D field over (x, z). The
original obtains this from a dedicated 2D code path equivalent to the 3D
noise evaluated on the plane y = 0 **without** adding the Y offset oy (see
Quirks Q2, Q3). A clean implementation should simply evaluate the 3D noise
at y = 0 (with or without oy — either is fine).

### 1.3 Octave Perlin noise (fractal sum)

An octave generator with N octaves holds N independent Perlin instances.
Given a sample coordinate c (already multiplied by the caller's per-axis
scale), it returns

    sum over i = 0 .. N-1 of   noise_i(c * 2^-i) * 2^i

i.e. **octave 0 has the highest frequency and the smallest amplitude; each
subsequent octave halves the frequency and doubles the amplitude.** The
theoretical amplitude of the sum is about 2^N - 1, so a 16-octave field has
values on the order of tens to a few thousand and callers divide it down
(by 512, 8000, etc.). The low-frequency octaves dominate.

When filling a grid, sample (i, j, k) of a grid with origin (x0, y0, z0)
and per-axis scales (sx, sy, sz) is evaluated at

    ((x0 + i) * sx, (y0 + j) * sy, (z0 + k) * sz)

and then passed through the octave sum above (each octave additionally
adds its own random offset after the frequency multiplication, inside the
single-octave noise).

Grid layout in the original is X-major, then Z, then Y innermost
(index = (i * sizeZ + k) * sizeY + j). For 2D grids the layout is
X-major, Z innermost.

### 1.4 Simplex noise (climate only)

The climate fields use 2D simplex noise.

- Skew factor `F = (sqrt(3) - 1) / 2`, unskew factor `G = (3 - sqrt(3)) / 6`.
- Permutation table: built identically to 1.2 (random X/Y/Z offsets in
  [0, 256) are drawn first; only the X and Y offsets are used, as the two
  2D axes).
- For input (x, y) (already scaled, plus offsets): skew
  `s = (x + y) F`; cell `i = floor(x + s)`, `j = floor(y + s)`; unskew
  `t = (i + j) G`; first-corner offsets `x0 = x - (i - t)`,
  `y0 = y - (j - t)`. Choose the middle corner (1,0) if x0 > y0 else (0,1).
  Corner offsets: `x1 = x0 - i1 + G`, `y1 = y0 - j1 + G`,
  `x2 = x0 - 1 + 2G`, `y2 = y0 - 1 + 2G`.
- Gradient index for a corner (ci, cj) = `p[(i & 255) + ci + p[(j & 255) + cj]] mod 12`,
  selecting from the 12 vectors (±1, ±1), (±1, 0) [twice], (0, ±1) [twice]
  — i.e. the 3D cube-edge gradients projected onto their first two
  components: (1,1), (-1,1), (1,-1), (-1,-1), (1,0), (-1,0), (1,0), (-1,0),
  (0,1), (0,-1), (0,1), (0,-1).
- Each corner contributes `(max(0, 0.5 - dx^2 - dy^2))^4 * dot(g, (dx, dy))`.
- Result is `70 * (sum of three contributions)`, approx. [-1, 1].

**Octave simplex.** Given base scales (sx, sz), a frequency multiplier `fm`
and an amplitude multiplier `am` (default 0.5): both base scales are first
divided by 1.5. Octave i (from 0) samples at scale `(sx/1.5) * fm^i`,
`(sz/1.5) * fm^i` and is weighted by `0.55 / am^i` (= 0.55 * 2^i for the
default). Like the Perlin octaves, later octaves are lower frequency and
higher amplitude when fm < 1.

---

## 2. Climate (temperature and humidity)

Climate is evaluated per block column at world (x, z), in block units.

| Field | Octaves | base scale (per block) | fm | am |
|---|---|---|---|---|
| temperature noise `Tn` | 4 | 0.025 × 0.025 | 0.25 | 0.5 |
| humidity noise `Hn` | 4 | 0.05 × 0.05 | 1/3 | 0.5 |
| blend noise `Bn` | 2 | 0.25 × 0.25 | 1/1.7 (0.5882352941176471) | 0.5 |

(Remember the octave simplex divides the base scale by 1.5.)

Then:

    b = Bn * 1.1 + 0.5
    T = (Tn * 0.15 + 0.7) * 0.99 + b * 0.01
    T = 1 - (1 - T)^2
    T = clamp(T, 0, 1)
    H = (Hn * 0.15 + 0.5) * 0.998 + b * 0.002
    H = clamp(H, 0, 1)

T and H feed the density function (section 3) and the biome choice used by
the surface pass (section 6). T also decides whether sea surface freezes.

**Biome lookup (only what the surface pass needs).** Quantize
`t = floor(T * 63) / 63`, `h = floor(H * 63) / 63`, then let `w = t * h`.
The column is **desert** when `t >= 0.1`, `w < 0.2` and `t >= 0.95`.
Desert columns use sand as both their top and filler block; every other
biome uses grass on top and dirt as filler. The full eleven-biome lookup
is in section 8; this rule is exactly its desert case.

---

## 3. The density field

### 3.1 Lattice

Density is computed on a coarse lattice, then interpolated to blocks.

- Horizontal spacing: **4 blocks** (lattice index `lx = worldX / 4`,
  `lz = worldZ / 4`).
- Vertical spacing: **8 blocks**; there are **17** lattice levels
  `ly = 0..16` covering y = 0..128.
- Per 16×16 chunk the original computes a 5 × 17 × 5 lattice (lattice
  points are shared with neighbouring chunks at the edges).

All noise inputs below are expressed in **lattice units**, not blocks.

### 3.2 Noise inputs and scale factors

Let `S = 684.412` (the same constant for horizontal and vertical).

| Name | Kind | Octaves | Scale (x, y, z) per lattice unit | Divisor / use |
|---|---|---|---|---|
| low-main `L` | 3D Perlin | 16 | (S, S, S) | `L / 512` |
| high-main `U` | 3D Perlin | 16 | (S, S, S) | `U / 512` |
| selector `M` | 3D Perlin | 8 | (S/80, S/160, S/80) = (8.55515, 4.277575, 8.55515) | `(M/10 + 1)/2` |
| scale `C` | 2D Perlin | 10 | (1.121, —, 1.121) | `(C + 256) / 512` |
| depth `D` | 2D Perlin | 16 | (200, —, 200) | `D / 8000` |

So, for instance, the lowest-frequency octave of L samples at
684.412 / 2^15 ≈ 0.0209 per lattice unit, i.e. one Perlin cell every
~48 lattice units (~190 blocks horizontally, ~380 blocks vertically). The
highest-frequency octave is essentially white noise but carries only
1/65535 of the weight.

### 3.3 Per-column terms

For each lattice column (lx, lz), obtain climate T and H at the
corresponding block column (see Quirk Q5 on exactly which block) and the
2D noises C and D at (lx, lz). Then:

1. **Climate dampening.**
   `a = 1 - (1 - T*H)^4`  (near 0 when cold or dry, near 1 when hot and wet).
2. **Scale (roughness).**
   `scale = (C + 256) / 512 * a`, capped at 1.
3. **Depth (base height offset).**
   - `d = D / 8000`
   - if d < 0: `d = -d * 0.3`   (fold negatives up, attenuated)
   - `d = d * 3 - 2`
   - if d < 0 (deep / ocean case):
     `d = d / 2`, floor at -1, then `d = d / 1.4 / 2`
     (so d ∈ [-0.357, 0)), **and force `scale = 0`**.
   - else (land case): cap d at 1, then `d = d / 8` (so d ∈ [0, 0.125]).
4. `scale = max(scale, 0) + 0.5` (so scale ∈ [0.5, 1.5]).
5. `d = d * 17 / 16` (17 = number of vertical lattice levels).
6. **Center height** in lattice levels: `center = 17/2 + d * 4 = 8.5 + 4d`.
   With d = 0 this is level 8.5 → block y = 68. Land columns raise the
   center by up to ~0.53 levels (~4 blocks); ocean columns lower it by up
   to ~1.52 levels (~12 blocks).

### 3.4 Per-sample density

For each vertical level ly in 0..16:

1. **Vertical falloff.** `fall = (ly - center) * 12 / scale`.
   If `fall < 0` (below the center), multiply it by 4.
   Above the center density drops slowly (12/scale per level); below it
   rises four times as fast, so the ground quickly becomes solid and the
   interesting overhangs live above y≈68. A small `scale` (cold/dry
   climate or ocean) makes the falloff steeper → flatter terrain; a large
   `scale` (warm/wet, high C) makes it gentler → taller, more broken
   terrain.
2. **Noise.** `lo = L / 512`, `hi = U / 512`, `sel = (M / 10 + 1) / 2`.
   `n = lo` if sel < 0; `n = hi` if sel > 1; otherwise `n = lo + (hi - lo) * sel`.
   (Optimization: when sel is outside [0, 1], only one of L/U is needed.)
3. `density = n - fall`.
4. **Top slide.** For the top three levels (ly = 14, 15, 16):
   `t = (ly - 13) / 3`, `density = density * (1 - t) + (-10) * t`.
   Level 16 (y = 128) is therefore always exactly -10, forcing air at the
   ceiling.

There is no bottom slide; the bottom is held solid by the ×4 falloff.

Because `n` comes from full 3D noise with amplitude that can exceed the
falloff several levels above the center, density can go positive above an
air gap: this is the source of Beta's overhangs, arches and floating
islands.

### 3.5 Interpolation

Each lattice cell (4 × 8 × 4 blocks) is filled by **trilinear
interpolation** of its eight corner densities. The original steps
incrementally: Y in 8 steps of 1/8 (outer), then X in 4 steps of 1/4, then
Z in 4 steps of 1/4 (inner), adding precomputed per-step deltas. Order does
not matter mathematically; a direct trilinear evaluation is equivalent up
to floating-point rounding.

### 3.6 Solid / air / water rule

For each block (x, y, z) with interpolated density ρ (plus any bias terms;
see 6.3):

- if `ρ > 0` (strictly) → **stone**;
- else if `y < 64` → **water** (sea level is 64; the water surface is the
  block at y = 63). In the original, if `y == 63` and the column
  temperature T < 0.5, it is **ice** instead of water.
- else → **air**.

So "sea fill" is simply: every non-solid block below y = 64 is water,
including enclosed caverns in the density field (there is no flood-fill).

---

## 4. Surface pass

Run after the stone/water/air fill, per column. Three 2D noises are
sampled at block resolution (per block, not per lattice unit):

| Name | Generator | Octaves | Scale per block | Use |
|---|---|---|---|---|
| sand noise `Sn` | sand/gravel gen | 4 | 1/32 | beach sand |
| gravel noise `Gn` | **same** generator as Sn, sampled on a different plane | 4 | 1/32 | beach gravel |
| stone-depth noise `Dn` | stone-depth gen | 4 | 1/16 | soil thickness |

(In the original, Sn samples the 3D noise on the plane (x, z, 0) — world Z
fed into the noise's Y axis — while Gn samples the 2D path at (x, z). Any
two decorrelated 4-octave fields are fine.)

Per column, with three uniform randoms r1, r2, r3 in [0, 1):

- `sand = Sn + r1 * 0.2 > 0`   (about half of all candidate columns)
- `gravel = Gn + r2 * 0.2 > 3` (rarer)
- `soil = trunc(Dn / 3 + 3 + r3 * 0.25)` — truncation toward zero. Typical
  values are 0..6. When `soil <= 0` the column shows bare stone.

The column's default `top` and `filler` are the biome's (grass/dirt, or
sand/sand for deserts).

Scan y from 127 down to 0 with a counter `run = -1`:

1. **Bedrock:** if `y <= rand_int(0..4)` (fresh random each block), place
   bedrock and continue. (Bedrock is not in this project's palette —
   use Stone.)
2. If the block is **air**: set `run = -1`.
3. If the block is **water**: nothing changes (run is untouched).
4. If the block is **stone**:
   - If `run == -1` (first solid block below air/water — an exposed
     surface):
     - if `soil <= 0`: `top = air`, `filler = stone`;
     - else if `60 <= y <= 65` (sea level −4 to +1; "beach band"):
       reset `top`/`filler` to the biome defaults; then if `gravel`:
       `top = air`, `filler = gravel`; then if `sand`:
       `top = sand`, `filler = sand` (sand overrides gravel);
     - if `y < 64` and `top == air`: `top = water`;
     - `run = soil`;
     - place `top` if `y >= 63`, else place `filler`.
       (So surfaces at or below y = 62 — sea floors — are filler: dirt,
       sand or gravel, never grass. A gravel/stone-patch surface exactly at
       y = 63 becomes water; at y = 64/65 it becomes air, lowering the
       ground by one block.)
   - else if `run > 0`: `run -= 1`, place `filler`. If `run` just reached
     0 and `filler` is sand: `run = rand_int(0..3)` and `filler = sandstone`
     (sandstone is not in this palette — use Sand or Stone).
   - else (run == 0): leave stone.

Note that `top`/`filler` changes persist down the column: a second exposed
surface lower in the same column (under an overhang) reuses whatever
`top`/`filler` were last set.

Outcomes worth checking in the viewer:
- grass on land above y = 63, dirt underneath for `soil` blocks;
- sand/gravel beaches at the shoreline band y = 60..65;
- dirt/sand/gravel sea floors;
- occasional bare-stone patches (soil ≤ 0), including stone shores;
- desert regions with sand on top regardless of height.

---

## 5. Quirks found in the original

| # | Quirk | Effect | Visible? | Recommendation |
|---|---|---|---|---|
| Q1 | Perlin cell index uses a truncating int cast with a "subtract 1 if negative" fix-up; for coordinates beyond ±2^31 the cast saturates. Main noise octave 0 runs at 684.412 per lattice unit, so it overflows past ~12.5 million blocks. | The "Far Lands". | Only at extreme distance. | Use 64-bit floor; don't reproduce. |
| Q2 | 2D Perlin path ignores the Y offset and evaluates on y = 0; the x-low/z-low corner uses a different gradient function (x-term dropped when hash bit 3 is set, and different second-term choice) than the other three corners. | Slight anisotropy/bias in 2D fields (depth, scale, gravel). | Not noticeably. | Use the clean 3D-at-y=0 version. |
| Q3 | 3D batch path caches the four x-lerped gradient pairs per Y cell and only recomputes them when the integer Y cell changes, so samples sharing a Y cell reuse gradient dot-products computed with the first sample's Y fraction (only the fade weight is updated). Affects octaves whose vertical step is < 1 Perlin cell (main noise octaves ≳ 10, all selector octaves, i.e. the dominant ones). | Vertically "stretched/stepped" character of the noise. | Subtle; arguably part of the texture. | Optional; implement clean Perlin by default. |
| Q4 | The temperature/humidity simplex floor uses "truncate, minus 1 if positive-or-zero is false", which is off by one at exact non-positive integers. | None in practice. | No. | Use proper floor. |
| Q5 | Climate is read at block offset `i * 3 + 1` within the chunk (16 / 5 = 3 by integer division) for lattice column i = 0..4, so the 5th lattice column (which physically lies at the next chunk's block 0) uses the climate of block 13 of the current chunk. Neighbouring chunks therefore compute **different** densities for the same shared lattice edge. | Tiny steps/seams in terrain exactly on chunk borders. | Rarely, small cliffs on chunk lines. | Don't reproduce; sample climate at the true lattice position (lx*4, lz*4). |
| Q6 | The surface noises are written in one array order and read in the transposed order, so within each chunk the sand/gravel/soil value at local (x, z) actually comes from local (z, x). | Beach and stone patches have seams on chunk borders. | Mildly. | Don't reproduce. |
| Q7 | Scale noise's third scale argument (and the depth noise's) is ignored; climate float literals (0.025f, 0.05f) are single precision. | None. | No. | Ignore. |
| Q8 | `soil` uses truncation toward zero, so values in (-1, 1) become 0 and produce bare stone the same as negative values. | Slightly more stone patches. | Yes, mildly. | Reproduce (use trunc). |
| Q9 | Per-column surface randomness comes from one RNG stream seeded per chunk, so the noise jitter (r1..r3, bedrock) depends on chunk processing order within the chunk. | None structurally. | No. | Use a position hash instead so any region is deterministic. |
| Q10 | The top slide divides by a single-precision 3.0. | None. | No. | Ignore. |

---

## 6. Implementation requirements for this project

### 6.1 Crate layout

- `crates/terrain` — pure Rust, `no_std` + `alloc` friendly where practical
  (use `libm` or hand-written `floor` if needed), no heavy dependencies.
- `crates/web` — `wasm-bindgen` wrapper.

### 6.2 Public API (terrain)

```text
enum Block : u8 { Air = 0, Stone = 1, Dirt = 2, Grass = 3, Sand = 4, Gravel = 5, Water = 6 }

generate_region(seed: u64, x0: i32, z0: i32, w: u32, d: u32) -> Region

struct Region { blocks: Vec<u8>, width: u32, depth: u32, height: u32 /* = 128 */ }
```

Choose and document a block index layout (e.g. `((z * width) + x) * 128 + y`
or Y-major). Generation must be **position-deterministic**: the block at a
given world coordinate must not depend on `x0`, `z0`, `w`, `d`. That means:
lattice points are always at world multiples of 4 (X/Z) and 8 (Y),
regardless of region origin; per-column randomness is a hash of
(seed, x, z) (and y for bedrock); climate sampled at fixed world positions.

Substitutions for blocks outside the palette: bedrock → Stone, ice →
Water, sandstone → Sand (or Stone).

### 6.3 Extension point: density bias terms

Provide a list of bias functions `f(x, y, z) -> f64` (world block
coordinates) whose sum is added to the interpolated density immediately
before the `ρ > 0` test. Intended future uses: island bumps, spawn water,
route channels. Default: empty list. Make it clearly marked in code, e.g.
a `DensityBias` trait / `&[&dyn Fn(i32, i32, i32) -> f64]` and a
`generate_region_with_bias` variant.

### 6.4 Web wrapper

Expose `generate_region` (returning the block bytes) and a heightmap helper
returning, per column, the top non-air block type and its height
(e.g. two arrays or an interleaved `Uint8Array`). For water columns, also
useful: the height of the first non-water block beneath (for depth tint).

### 6.5 Tests

- Noise output stays within expected bounds (single Perlin within
  ~[-1.1, 1.1], single simplex within ~[-1, 1], octave sums within
  ±(2^N - 1)).
- Same seed → identical output; different seeds → different output.
- Overlapping regions agree on shared world coordinates.
- Sea level fill: every non-solid block below y = 64 is water, and there
  is no water at y ≥ 64.

### 6.6 Performance notes

A 512×512 region is a 129×17×129 lattice (~283k points, ~40 Perlin
octave evaluations each). Evaluate the lattice for the whole region at
once (not chunk by chunk) to avoid duplicate edge work, skip whichever of
L/U the selector makes irrelevant, and precompute per-axis
floor/fade values where convenient.

---

# Part II — Biomes and vegetation

Part II covers what happens after the terrain of Part I exists: choosing a
biome per column, the per-chunk "population" (decoration) pass, every tree
and small-plant generator, snow, and the related block properties. As in
Part I, seed parity is not a goal; everything is described as rules and
constants. Section numbers continue from Part I.

Notation used below:

- `rand(n)` is a uniform random integer in `[0, n)`; `u()` is a uniform
  random real in `[0, 1)`. Nested forms such as `rand(rand(3) + 1)` mean the
  inner draw is made first and used as the bound of the outer draw.
- "Spread s/v" around a point (x, y, z) means a trial position
  `(x + rand(s) - rand(s), y + rand(v) - rand(v), z + rand(s) - rand(s))`,
  i.e. a triangular distribution on `[-(s-1), s-1]` horizontally and
  `[-(v-1), v-1]` vertically, peaked at the centre.
- "trunc" is truncation toward zero; "floor" rounds toward −∞.
- The *heightmap* value of a column is the lowest y such that the block at
  y and every block above it has light opacity 0 (see the block table,
  section 16). It is one above the highest light-blocking block; leaves
  (opacity 1), water and ice (opacity 3) count as light-blocking, plants,
  snow layers and air do not.
- A *movement-blocking* block is any block except air, water, lava, the
  small plants (tall grass, fern, dead bush, flowers, mushrooms, sugar
  cane) and the snow layer. Logs, leaves, cactus, pumpkin and ice are
  movement-blocking. The same split defines *solid* for the cactus
  side-clearance rule.
- An *opaque cube* is stone, dirt, grass, sand, gravel, sandstone, ores,
  clay, logs, pumpkin and (see quirk B6) leaves; not water, ice, plants,
  cactus or snow layers.

---

## 7. Climate (one shared source)

Biomes and the density field (section 3.3) read **the same climate
field**. Section 2 already gives the full formula and it is confirmed here
without change:

| Field | Generator RNG seed | Octaves | Base scale (per block) | Frequency multiplier per octave |
|---|---|---|---|---|
| temperature `Tn` | seed × 9871 | 4 | 0.025 | 0.25 |
| humidity `Hn` | seed × 39811 | 4 | 0.05 | 1/3 |
| blend `Bn` | seed × 543321 | 2 | 0.25 | 1/1.7 = 0.5882352941176471 |

All three are octave simplex (section 1.4) with amplitude multiplier 0.5,
i.e. octave i is sampled at `(base / 1.5) × fm^i` and weighted
`0.55 × 2^i`. With 4 octaves the temperature sum has theoretical amplitude
0.55 × 15 = 8.25 (humidity likewise; blend 0.55 × 3 = 1.65); the
clamps below only bite in the tails.

Combination, per block column (x, z):

    b = Bn × 1.1 + 0.5
    T = (Tn × 0.15 + 0.7) × 0.99 + b × 0.01
    T = 1 − (1 − T)²            then clamp T to [0, 1]
    H = (Hn × 0.15 + 0.5) × 0.998 + b × 0.002, clamp H to [0, 1]

The squaring step skews T strongly toward 1: an un-squared value of 0.7
becomes 0.91, 0.5 becomes 0.75. There is no further smoothing or
interpolation; the field is evaluated per block column. Output ranges are
exactly [0, 1] for both.

Uses of the shared climate:

1. Density (section 3.3) uses unquantised T and H (`a = 1 − (1 − T·H)^4`).
2. Biome lookup (section 8) uses T and H quantised to 64 levels.
3. Sea-surface ice (section 3.6) uses unquantised T < 0.5 at y = 63.
4. The snow pass (section 13) uses unquantised T, adjusted for height.

Reconciliation with Part I: no discrepancy was found between section 2 and
the original's climate code. Two incidental points: the original's biome
grid routine asks the noise for a w × w grid even when it wants w × d
(harmless, it is always called with 16 × 16 or 1 × 1); and the single-
precision literals 0.025 and 0.05 (Quirk Q7) apply here too. An
implementation must expose **one** function `climate(x, z) -> (T, H)` and
use it for all four purposes above; never re-derive T for the biome or the
snow pass from a different noise instance or seed.

---

## 8. Biome lookup

### 8.1 Quantisation and the 64 × 64 table

The original never evaluates the decision rules at lookup time. At start-up
it fills a table of 64 × 64 biomes; entry (i, j) holds the biome for
temperature `i / 63` and humidity `j / 63` (computed in single precision),
stored at index `i + 64·j`. Looking up a column takes
`i = trunc(T × 63)`, `j = trunc(H × 63)` (both in 0..63, since T, H are
in [0, 1]) and returns entry (i, j). So T and H are effectively floored to
multiples of 1/63 before the rules below are applied.

### 8.2 Decision rules

With `t = i/63`, `h = j/63` and `w = t × h` (temperature times humidity),
evaluate in this order; the first matching row wins:

| # | Condition | Biome |
|---|---|---|
| 1 | t < 0.1 | Tundra |
| 2 | w < 0.2 and t < 0.5 | Tundra |
| 3 | w < 0.2 and t < 0.95 | Savanna |
| 4 | w < 0.2 | Desert |
| 5 | w > 0.5 and t < 0.7 | Swampland |
| 6 | t < 0.5 | Taiga |
| 7 | t < 0.97 and w < 0.35 | Shrubland |
| 8 | t < 0.97 | Forest |
| 9 | w < 0.45 | Plains |
| 10 | w < 0.9 | Seasonal Forest |
| 11 | otherwise | Rainforest |

Because t and h are multiples of 1/63 none of the thresholds is ever hit
exactly, so the table is reproduced exactly by this integer form (verified
for all 4096 entries), with `P = i × j`:

| Test | Integer equivalent |
|---|---|
| t < 0.1 | i ≤ 6 |
| t < 0.5 | i ≤ 31 |
| t < 0.7 | i ≤ 44 |
| t < 0.95 | i ≤ 59 |
| t < 0.97 | i ≤ 61 |
| w < 0.2 | P ≤ 793 |
| w < 0.35 | P ≤ 1389 |
| w < 0.45 | P ≤ 1786 |
| w > 0.5 | P ≥ 1985 |
| w < 0.9 | P ≤ 3572 |

Table cells per biome (out of 4096; world frequency differs because T is
skewed high): Tundra 1575, Forest 839, Savanna 518, Taiga 473, Shrubland
397, Swampland 138, Seasonal Forest 57, Desert 54, Plains 32, Rainforest 13,
Ice Desert 0.

### 8.3 Quirks of the lookup

- **Ice Desert is unreachable.** It is defined (sand surface, snowy) but no
  rule returns it.
- **Plains, Seasonal Forest and Rainforest need i ≥ 62** (T ≥ 62/63 ≈
  0.984), i.e. the hottest band; Plains there is the *dry* end.
- **Desert needs i ≥ 60** (T ≥ 0.952) with w < 0.2, i.e. H below ~0.21.
- **Taiga can never be Swampland-tested**: when t < 0.5, w < 0.5 always, so
  rule 5 cannot fire and all cool, moderately humid columns are Taiga.
- **Tundra covers both the cold strip (i ≤ 6) and the cold-dry corner**
  (t < 0.5 and w < 0.2), which is why it fills so many table cells.
- Two further biomes (Hell, Sky) exist in the original for other
  dimensions and never appear in the overworld lookup.

### 8.4 Where the lookup is evaluated

- Surface pass (section 4): per block column, at the column itself.
- Population (section 10): **once per chunk**, at block
  `(16·cx + 16, 16·cz + 16)` — the centre of the decorated square
  (section 15) — and that single biome governs every decision of the pass.

---

## 9. Per-biome effects

### 9.1 Surface, flags, trees and plant counts

"Top/filler" are the defaults used by the surface pass (section 4).
"Snowy" and "no rain" only affect weather during play, not generation.

| Biome | Top / filler | Snowy | No rain | Tree-count term | Tree choice |
|---|---|---|---|---|---|
| Rainforest | grass / dirt | – | – | N + 5 | 1/3 big oak, 2/3 oak |
| Swampland | grass / dirt | – | – | 0 | 1/10 big oak, 9/10 oak |
| Seasonal Forest | grass / dirt | – | – | N + 2 | 1/10 big oak, 9/10 oak |
| Forest | grass / dirt | – | – | N + 5 | 1/5 birch, 4/15 big oak, 8/15 oak |
| Savanna | grass / dirt | – | – | 0 | 1/10 big oak, 9/10 oak |
| Shrubland | grass / dirt | – | – | 0 | 1/10 big oak, 9/10 oak |
| Taiga | grass / dirt | yes | – | N + 5 | 1/3 taiga tree A, 2/3 taiga tree B |
| Desert | sand / sand | – | yes | −20 (none) | (1/10 big oak, 9/10 oak) |
| Plains | grass / dirt | – | – | −20 (none) | (1/10 big oak, 9/10 oak) |
| Ice Desert | sand / sand | yes | yes | 0 | 1/10 big oak, 9/10 oak |
| Tundra | grass / dirt | yes | – | −20 (none) | (1/10 big oak, 9/10 oak) |

N is the noise-driven base count of section 10.3; every biome additionally
gets one extra tree attempt with probability 1/10, so "0" biomes average
0.1 tree attempts per chunk and "−20" biomes get none at all. The Forest
picker first rolls 1/5 for birch, then 1/3 of the remainder for big oak.

Small-plant patch attempts per chunk (each "patch" is one call of the
generator in section 12; probability entries are a single patch with that
chance):

| Biome | Dandelion | Rose | Tall grass (fern share) | Dead bush | Cactus | Sugar cane | Pumpkin | Brown / red mushroom |
|---|---|---|---|---|---|---|---|---|
| Rainforest | 0 | 1/2 | 10 (each patch fern w.p. 2/3) | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Swampland | 0 | 1/2 | 0 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Seasonal Forest | 4 | 1/2 | 2 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Forest | 2 | 1/2 | 2 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Savanna | 0 | 1/2 | 0 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Shrubland | 0 | 1/2 | 0 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Taiga | 2 | 1/2 | 1 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Desert | 0 | 1/2 | 0 | 2 | 10 | 10 | 1/32 | 1/4 / 1/8 |
| Plains | 3 | 1/2 | 10 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Ice Desert | 0 | 1/2 | 0 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |
| Tundra | 0 | 1/2 | 0 | 0 | 0 | 10 | 1/32 | 1/4 / 1/8 |

Only Rainforest produces ferns; everywhere else tall-grass patches are the
grass variant. Roses, sugar cane, pumpkins and mushrooms are biome
independent (but each still needs suitable ground, e.g. roses and pumpkins
never succeed on desert sand).

### 9.2 Snow

No biome places snow directly. Snow layers come only from the
temperature-based snow pass (section 13), so they appear wherever T is low
enough — mostly Tundra and Taiga, but also high ground in other biomes.

---

## 10. Population (decoration) pass

### 10.1 When and where

Population of chunk (cx, cz) runs once, after the terrain (Part I) of that
chunk **and of its three neighbours (cx+1, cz), (cx, cz+1), (cx+1, cz+1)**
exists. Let `X0 = 16·cx`, `Z0 = 16·cz`. Most features are anchored inside
the *decorated square* `[X0+8, X0+24) × [Z0+8, Z0+24)`, which straddles
those four chunks (section 15).

During population, sand and gravel that lose support fall instantly
instead of becoming falling entities.

### 10.2 Per-chunk RNG

The original seeds a fresh generator with the world seed, draws two 64-bit
integers A and B and forces each to be odd (halve truncating toward zero,
double, add one). The chunk's population RNG is then seeded with
`(cx·A + cz·B) XOR seed` (wrapping 64-bit arithmetic). All draws below come
from that single stream, in the listed order. An implementation should
instead derive the stream from a hash of (seed, cx, cz) — order of
consumption within a chunk is all that matters for self-consistency.

### 10.3 Steps, in order

Positions are `(x, y, z)`; "+8" means `X0 + 8 + rand(16)` (resp. Z), "+0"
means `X0 + rand(16)` (the ore/clay generators add 8 internally, so they
too end up centred on the decorated square). Unless stated, the draws for
one attempt are x, then y, then z.

| # | Step | Attempts | Position | Notes |
|---|---|---|---|---|
| 1 | Water lake | 1 if rand(4) = 0 | +8, y = rand(128), +8 | still water |
| 2 | Lava lake | 1 if rand(8) = 0 | +8, y = rand(rand(120) + 8), +8 | placed only if y < 64 or rand(10) = 0 |
| 3 | Dungeon | 8 | +8, rand(128), +8 | |
| 4 | Clay | 10 | +0, rand(128), +0 | vein size 32, only in water |
| 5 | Dirt | 20 | +0, rand(128), +0 | vein size 32 |
| 6 | Gravel | 10 | +0, rand(128), +0 | vein size 32 |
| 7 | Coal ore | 20 | +0, rand(128), +0 | vein size 16 |
| 8 | Iron ore | 20 | +0, rand(64), +0 | vein size 8 |
| 9 | Gold ore | 2 | +0, rand(32), +0 | vein size 8 |
| 10 | Redstone ore | 8 | +0, rand(16), +0 | vein size 7 |
| 11 | Diamond ore | 1 | +0, rand(16), +0 | vein size 7 |
| 12 | Lapis ore | 1 | +0, rand(16) + rand(16), +0 | vein size 6 |
| 13 | Trees | count below | +8, heightmap, +8 | draws: x, z, then the biome's tree choice |
| 14 | Dandelion patches | per biome (9.1) | +8, rand(128), +8 | flower patch |
| 15 | Tall grass patches | per biome (9.1) | +8, rand(128), +8 | in Rainforest a fern roll rand(3) ≠ 0 is drawn *before* the position |
| 16 | Dead bush patches | Desert: 2 | +8, rand(128), +8 | |
| 17 | Rose patch | 1 if rand(2) = 0 | +8, rand(128), +8 | flower patch |
| 18 | Brown mushroom patch | 1 if rand(4) = 0 | +8, rand(128), +8 | flower patch rules for mushrooms |
| 19 | Red mushroom patch | 1 if rand(8) = 0 | +8, rand(128), +8 | |
| 20 | Sugar cane | 10 | +8, rand(128), +8 | |
| 21 | Pumpkin patch | 1 if rand(32) = 0 | +8, rand(128), +8 | |
| 22 | Cactus | Desert: 10 | +8, rand(128), +8 | |
| 23 | Water spring | 50 | +8, rand(rand(120) + 8), +8 | flowing water |
| 24 | Lava spring | 20 | +8, rand(rand(rand(112) + 8) + 8), +8 | flowing lava |
| 25 | Snow pass | every column of the decorated square | | section 13 |

Steps 1–12 and 23–24 are out of scope for this project beyond the order and
counts above (they belong to caves/ores, which the viewer does not render).
Briefly: lakes carve an irregular blob of up to 16 × 8 × 16 made of 4–7
ellipsoids, starting 4 blocks below the first non-air block under the given
point; springs only fire on a stone block (or air) with stone above and
below, exactly three stone horizontal neighbours and exactly one air
neighbour.

**Tree count (step 13).** The tree-density noise is an 8-octave Perlin
generator (section 1.3), created from the main terrain RNG right after the
depth noise. Sample it once per chunk as a 2D field at
`(X0 × 0.5, Z0 × 0.5)` — the chunk's own corner, *not* the decorated
square — giving `D` (theoretical range ±255). Then:

    N = trunc((D / 8 + 4·u() + 4) / 3)
    count = (1 if rand(10) = 0 else 0) + biome term (table 9.1)

N is drawn (the `u()` is consumed) in every biome. With D = 0, N is 1 or 2;
with D/8 = 8, N is 4 or 5; for D/8 below about −4, N is 0 or negative, so
a forest chunk can get as few as ~3 attempts. A non-positive count means no
trees.

For each attempt: pick x (+8), then z (+8), then the tree type (table 9.1,
which consumes RNG), set `y = heightmap(x, z)`, and run the tree generator
(section 11) with origin (x, y, z) — the origin is the lowest trunk block,
the block below it is the ground. A failed tree is not retried.

**Height selection summary.**

| Feature | Starting y | Height search |
|---|---|---|
| Trees | heightmap | none (already on top) |
| Tall grass, dead bush | rand(128) | descend while the block is air or leaves (and y > 0); the patch is centred on the first other block |
| Flowers, mushrooms, pumpkins, cactus | rand(128) | none: trials are y ± 3 around the random height |
| Sugar cane | rand(128) | none: all trials at exactly that y |

Because the flower/mushroom/pumpkin/cactus/reed centres have a uniformly
random height and no search, most of those patches land in solid rock or
open air and place nothing; this is the main reason those plants are
sparse, and an implementation that wants Beta-like density must keep it
(or scale the counts down accordingly if it searches for the surface).
Mushroom patches whose random height falls in an underground air pocket
are how mushrooms end up in caverns.

---

## 11. Trees

Common rules for the four small trees (oak, birch, taiga A, taiga B):

- **Bounds:** origin y ≥ 1 and `y + h + 1 ≤ 128`, where h is the tree's
  height; otherwise fail.
- **Clearance:** every block of a stack of horizontal squares from the
  origin's y up to `y + h + 1` (inclusive) must be **air or leaves**;
  anything else (including tall grass, flowers, snow, water) fails the
  tree. Positions outside 0..127 vertically fail. The square's half-width
  (radius ρ) per level is given for each tree.
- **Ground:** the block below the origin must be grass or dirt (and
  `y < 128 − h − 1`). It is then replaced by dirt.
- **Leaves** are written only where the current block is not an opaque
  cube; with the original's flags (quirk B6) existing leaves are left
  alone, while air, water, snow layers and small plants are overwritten.
- **Logs** of the trunk are written only where the current block is air or
  leaves (so they replace leaves just placed by this tree).
- "Square of radius r" means all (dx, dz) with |dx| ≤ r and |dz| ≤ r around
  the trunk column; its "corners" are the four cells with |dx| = |dz| = r
  (none when r = 0).

### 11.1 Oak

- Height `h = 4 + rand(3)` (4..6).
- Clearance radius: 0 at the origin level, 1 for levels up to `y + h − 2`,
  2 for levels `y + h − 1` … `y + h + 1`.
- Canopy: four layers, from `y + h − 3` up to `y + h`. With
  `k = level − (y + h)` (−3, −2, −1, 0) the radius is
  `r = 1 − trunc(k / 2)`, i.e. **2, 2, 1, 1** bottom to top.
  Every cell of the square is leaves except corners: on the top layer
  (k = 0) corners are always left out; on the three lower layers each
  corner is kept with probability 1/2. (The original draws the coin for
  every corner, including the top layer's, before checking k.)
- Trunk: logs from y to `y + h − 1`. The top canopy layer sits directly
  above the last log.
- Log and leaves variant: oak.

Resulting shape: two 5×5 layers with random corners, a 3×3 layer with
random corners, and a 3×3 plus-shape on top.

### 11.2 Birch

Identical to oak except `h = 5 + rand(3)` (5..7) and both logs and leaves
are the birch variant.

### 11.3 Taiga tree A (sparse "pine")

- Height `h = 7 + rand(5)` (7..11).
- Bare-trunk length `a = h − 3 − rand(2)` (h−3 or h−4); leafy part
  `L = h − a` (3 or 4); maximum radius `R = 1 + rand(L + 1)` (1..L+1).
- Clearance radius: 0 for levels below `y + a`, R from `y + a` to
  `y + h + 1`.
- Canopy: layers from `y + h` down to `y + a` (L + 1 layers), radius
  starting at 0 on the top layer. After each layer: if the layer just done
  was `y + a + 1` and the radius is ≥ 1, the radius decreases by one;
  otherwise, if it is below R, it increases by one. So radii read, top
  down, 0, 1, 2, … up to R, then hold at R, and the lowest layer is one
  smaller than the layer above it. Corners are always omitted (no
  randomness), so radius 1 is a plus and radius 2 a 5×5 minus corners.
- Trunk: logs from y to `y + h − 2` (h − 1 logs); the top two canopy
  layers have no log inside.
- Variant: spruce (logs and leaves).

### 11.4 Taiga tree B (layered "spruce")

- Height `h = 6 + rand(4)` (6..9).
- Bare-trunk length `a = 1 + rand(2)` (1..2); maximum radius
  `R = 2 + rand(2)` (2..3).
- Clearance radius: 0 below `y + a`, R from `y + a` to `y + h + 1`.
- Canopy: layers from `y + h` down to `y + a` (h − a + 1 layers). The
  radius uses a small state machine with a current radius r, a cap c and a
  restart value s. Start with `r = rand(2)` (drawn after the ground check),
  `c = 1`, `s = 0`. After each layer: if `r ≥ c`, set `r = s`, then
  `s = 1` and `c = min(c + 1, R)`; otherwise `r = r + 1`. Top-down this
  produces rings that grow and snap back, e.g. starting at 0 with R = 3:
  0, 1, 0, 1, 2, 1, 2, 3, 1, 2, 3, 1, 2, 3, …; starting at 1:
  1, 0, 1, 2, 1, 2, 3, 1, 2, 3, …. Corners are always omitted.
- Trunk: `h − rand(3)` logs from y upward (top at `y + h − 1`,
  `y + h − 2` or `y + h − 3`), drawn after the canopy, so the upper one to
  three canopy layers may have no log inside.
- Variant: spruce.

### 11.5 Big oak

The big oak uses its own random stream, seeded from one 64-bit draw of the
population RNG (so a big-oak attempt consumes exactly one draw from the
chunk stream, whatever it does). The population pass configures it with
"scale 1", which fixes these constants:

| Constant | Value | Meaning |
|---|---|---|
| height range | 12 | total height `H = 5 + rand(12)` (5..16) |
| cluster height | 5 | each foliage cluster is 5 layers tall |
| trunk ratio | 0.618 | trunk height `F = trunc(0.618·H)`, at most H − 1 |
| branch slope | 0.381 | how far below its cluster a branch leaves the trunk |
| branch length factor | 1.0 | |
| foliage density | 1.0 | clusters per level `C = max(1, trunc(1.382 + (H/13)²))`: 1 for H ≤ 10, 2 for H ≥ 11 |
| branch cutoff | 0.2 | branches starting below `0.2·H` above the origin are not drawn |
| level cutoff | 0.3 | no clusters on levels below `0.3·H` |
| trunk width | 1 | single-column trunk |

**Line walk.** Two operations walk a straight line between integer points
P and Q. Let the *major axis* be the axis with the largest |Q − P|
component (ties go to the earlier of x, y, z). Step t from 0 to that
component's difference (both ends included, one block per step); the
major coordinate is `P + t` and each other coordinate is `P + t × ratio`
where ratio is that axis's difference over the major difference. The
*clearance walk* takes the floor of the minor coordinates; it succeeds if
every visited block is air or leaves, and otherwise reports the step t at
which it hit an obstruction. The *log walk* rounds the minor coordinates
(floor of value + 0.5) and writes a log at every visited block,
unconditionally replacing whatever is there. A zero-length line is clear
and writes nothing.

**Algorithm.** Origin (x0, y0, z0).

1. Ground: the block below the origin must be grass or dirt, else fail.
   (Unlike the small trees, it is **not** turned into dirt.)
2. Trunk clearance: clearance-walk from (x0, y0, z0) to
   (x0, y0 + H − 1, z0). If clear, keep H. If blocked at step k: fail when
   k < 6, otherwise set `H = k` (the tree shrinks to fit). Then compute F
   and C from the (possibly reduced) H.
3. Clusters. A cluster is a foliage position (cx, cy, cz) plus the height
   b at which its branch leaves the trunk.
   - The first cluster is always the crown: `(x0, y0 + H − 5, z0)` with
     `b = y0 + F`. No checks.
   - Then walk levels `ℓ = H − 5, H − 6, …, 0`. Level ℓ is evaluated at
     block height `y = y0 + ℓ − 1` (the original is off by one here: the
     shape uses ℓ while the position uses ℓ − 1). Its size is
     `s(ℓ) = 0.5 × sqrt(m² − (m − ℓ)²)` with `m = H/2`, defined as 0.5·m
     when ℓ = m exactly, and with the level skipped entirely when
     `ℓ < 0.3·H`.
   - For each of C tries on a non-skipped level: draw `u1`, `u2`;
     branch length `len = s(ℓ) × (u1 + 0.328)`; angle
     `θ = u2 × 2 × 3.14159`; cluster column
     `cx = floor(x0 + len·sin θ + 0.5)`, `cz = floor(z0 + len·cos θ + 0.5)`,
     `cy = y`.
   - Accept the try only if (a) the vertical segment from (cx, cy, cz) to
     (cx, cy + 5, cz) is clear, and (b) with D the horizontal distance
     between (x0, z0) and (cx, cz) and `b = min(y0 + F, trunc(cy − 0.381·D))`,
     the line from (x0, b, z0) to (cx, cy, cz) is clear. Rejected tries
     are simply dropped.
   Branch lengths peak at mid-height: the maximum `s` is H/4 (4 for
   H = 16), so clusters sit at most about 5.3 columns from the trunk.
4. Foliage: for every accepted cluster (crown included), write leaves in
   five horizontal layers cy … cy + 4 with radii **2, 3, 3, 3, 2**. A layer
   of radius r contains the offsets (dx, dz) with |dx|, |dz| ≤
   trunc(r + 0.618) and `sqrt((|dx| + 0.5)² + (|dz| + 0.5)²) ≤ r`, which
   gives a 5-cell plus for r = 2 and a 5×5 square minus its four corners
   (21 cells) for r = 3. Leaves go only into air or leaves.
5. Trunk: log walk from (x0, y0, z0) to (x0, y0 + F, z0) — F + 1 logs,
   overwriting leaves.
6. Branches: for every cluster with `b − y0 ≥ 0.2·H`, log walk from
   (x0, b, z0) to (cx, cy, cz). The crown's "branch" is the vertical run
   from y0 + F to y0 + H − 5, which extends the trunk upward when
   H − 5 > F (H ≥ 12). Branch logs replace the leaf at the bottom centre
   of their cluster.

Overall: a trunk of 61.8 % of the height, a crown cluster at the top
(top leaves at y0 + H − 1), and one or two side clusters per level on
roughly the upper two-thirds, with leafless lower branches skipped.
Logs and leaves are the oak variant.

---

## 12. Small plants

All patch generators put plants only on **air** positions and only where the
plant's survival rule holds. Survival rules:

| Plant | Ground (block directly below) | Other condition |
|---|---|---|
| Dandelion, rose, tall grass, fern | grass, dirt or farmland | light ≥ 8 **or** the position is at/above its column's heightmap |
| Dead bush | sand | light ≥ 8 or at/above heightmap |
| Brown / red mushroom | any opaque cube | 0 ≤ y ≤ 127 and light < 13 |
| Sugar cane | sugar cane, **or** grass/dirt that has water (still or flowing) in at least one of its four horizontal neighbours | — (sand is *not* accepted in this version) |
| Cactus | sand or cactus | none of the four horizontal neighbours is solid |
| Pumpkin | grass (generator), and must be an opaque full block | — |

"Light" means the combined sky/block light at that position at
population time. The chunk's sky light is computed column by column when
the terrain is created: it starts at 15 above the column and drops by each
block's light opacity going down (section 16), with no sideways spread
before population. An implementation should use exactly that vertical
rule: a plant spot is lit to `15 − (sum of opacities of blocks above it)`,
floored at 0. Under one or two leaf layers flowers still grow; mushrooms
need three or more leaf layers, water above, or rock above.

Generators:

- **Flower patch** (dandelion, rose, either mushroom): 64 trials at spread
  8/4 around the centre.
- **Tall grass / fern patch**: first descend from the centre while the block
  is air or leaves (stop at y = 0), then 128 trials at spread 8/4 around
  that block. The variant (grass or fern) is fixed for the whole patch.
- **Dead bush patch**: same descent, 4 trials at spread 8/4.
- **Sugar cane**: 20 trials at spread 4/1 (i.e. dx, dz in −3..3, same y).
  A trial needs air at the spot and water beside the block below it. Its
  height is `2 + rand(rand(3) + 1)` — 2, 3 or 4 with probabilities 11/18,
  5/18, 2/18. Blocks are placed bottom-up while each passes the survival
  rule; only the bottom position is checked for air, the upper ones
  overwrite whatever is there.
- **Cactus**: 10 trials at spread 8/4. A trial needs air at the spot;
  height `1 + rand(rand(3) + 1)` — 1, 2 or 3 with probabilities 11/18,
  5/18, 2/18. Each block bottom-up is placed if the cactus survival rule
  holds at that position (so a neighbour beside an upper block can stop
  growth part-way); upper positions are not checked for air.
- **Pumpkin patch**: 64 trials at spread 8/4; needs air and grass below;
  each pumpkin gets a random facing 0..3.

Patches can overlap their own earlier placements; a trial on an occupied
spot just fails.

---

## 13. Snow and ice

**Ice (terrain fill, Part I).** The only water frozen during generation is
the sea surface: during the density fill, a non-solid block at y = 63 is
ice instead of water when the column's T < 0.5 (section 3.6). Lakes and
springs placed by population never freeze, and no other water level
freezes. (Freezing of other water happens later through weather, which is
out of scope.)

**Snow pass (population step 25).** For every column (x, z) of the
decorated square:

1. Let `top` be one above the highest block in the column, scanning from
   y = 127 down to y = 1, that is movement-blocking or liquid (−1 if none).
2. Compute `T' = T(x, z) − (top − 64) / 64 × 0.3` using the shared climate
   (unquantised T).
3. Place a snow layer (thinnest, one-eighth height) at y = top if all hold:
   `T' < 0.5`; `0 < top < 128`; the block at `top` is air; the block at
   `top − 1` is movement-blocking; and that block is not ice.

Consequences: snow lies on grass, sand, stone, logs, leaves (tree tops get
a white cap), cactus and pumpkins, but never on water or sea ice; a column
whose top is a small plant gets no snow (the plant occupies the spot). The
threshold rises with height — at top = 64 it is T < 0.5, at top = 96 it is
T < 0.65, at top = 128 T < 0.8 — so mountains in temperate climates get
snow caps, while low ground needs a cold climate. The snow pass runs after
trees, so it sees the final canopy.

---

## 14. Spawn lists

These do not affect terrain; listed for completeness. Every overworld biome
starts from the same lists, and Forest and Taiga each add Wolf (weight 2)
to the creature list. Weights are relative within a list.

| List (population cap) | Species and weights |
|---|---|
| Creatures (15) | Sheep 12, Pig 10, Chicken 10, Cow 8; + Wolf 2 in Forest and Taiga |
| Monsters (70) | Spider 10, Zombie 10, Skeleton 10, Creeper 10, Slime 10 |
| Water creatures (5) | Squid 10 |

So in Forest/Taiga the creature odds are Sheep 12/42, Pig 10/42,
Chicken 10/42, Cow 8/42, Wolf 2/42; elsewhere Sheep 12/40, Pig 10/40,
Chicken 10/40, Cow 8/40.

---

## 15. Chunk-boundary handling (the "+8" offset)

The original generates terrain one 16 × 16 chunk at a time, on demand, and
must decorate without knowing whether neighbouring chunks exist. It solves
this by:

1. delaying population of chunk (cx, cz) until chunks (cx, cz), (cx+1, cz),
   (cx, cz+1) and (cx+1, cz+1) all have terrain; and
2. anchoring features in the square shifted by +8 on both axes, so the
   anchor square is centred on the shared corner of those four chunks.

Features spread at most ~7–8 blocks from their anchor (patch spread ≤ 7,
small-tree canopy ≤ 2, big-oak clusters ≤ ~5.3 + 2), so they stay inside
the 32 × 32 area of those four chunks and never write into a chunk that
does not exist yet. The single biome per population (section 8.4) is
sampled at the centre of the anchor square for the same reason.

Side effects of the scheme: decoration of a given spot depends on which
neighbouring populations ran first (a tree can fail on grass left by the
neighbour's grass patch; a later tree's leaves never replace earlier
leaves), and the per-chunk biome means decoration changes abruptly on the
shifted grid, not at the biome border.

**This project should not reproduce the scheme.** It generates whole
regions at once, so it should:

- run terrain (Part I) for the region plus a margin of at least 16 blocks
  on every side;
- partition the world into fixed 16 × 16 *decoration cells* aligned to
  world multiples of 16 (the anchor squares themselves, i.e. the cell is
  the anchor area — no extra shift is needed);
- process every cell that overlaps the region-plus-margin in a fixed world
  order (e.g. by cell z, then cell x), with a cell RNG derived from
  (seed, cell x, cell z), sampling the cell's biome at its centre (or,
  better, per feature at the feature's own column — a recommended
  deviation);
- crop to the requested region.

With a 16-block margin the only way output can depend on region origin is
through chains of interacting features crossing more than one cell, which
is rare; tests comparing overlapping regions should compare blocks at
least 16 blocks inside both regions' edges, or use a 32-block margin.
Any fixed world order is acceptable, including one made of several passes
over the cells (for example by cell-coordinate parity), and so is a margin
larger than 16 blocks, chosen so that output is exactly independent of
region origin and size. (Decided in #3.)

---

## 16. Block table for the new blocks

Fire values are the two numbers the original's fire block registers per
block: *encouragement* (how readily fire spreads onto an adjacent air
block next to it) and *flammability* (how readily the block itself catches
and burns away). Unlisted blocks are 0 / 0, i.e. fire-proof.

| Block | Original id : meta | Fire encouragement / flammability | Collision | Light opacity | Opaque cube |
|---|---|---|---|---|---|
| Oak log | 17 : 0 | 5 / 5 | full | 255 (opaque) | yes |
| Spruce log | 17 : 1 | 5 / 5 | full | 255 | yes |
| Birch log | 17 : 2 | 5 / 5 | full | 255 | yes |
| Oak leaves | 18 : 0 | 30 / 60 | full | 1 | yes (B6) |
| Spruce leaves | 18 : 1 | 30 / 60 | full | 1 | yes (B6) |
| Birch leaves | 18 : 2 | 30 / 60 | full | 1 | yes (B6) |
| Tall grass | 31 : 1 | 60 / 100 | none | 0 | no |
| Fern | 31 : 2 | 60 / 100 | none | 0 | no |
| Dead bush | 32 : 0 | 0 / 0 | none | 0 | no |
| Dandelion | 37 | 0 / 0 | none | 0 | no |
| Rose | 38 | 0 / 0 | none | 0 | no |
| Brown mushroom | 39 | 0 / 0 | none | 0 (emits light 1) | no |
| Red mushroom | 40 | 0 / 0 | none | 0 | no |
| Snow layer | 78 : 0 | 0 / 0 | none at generated thickness (1/8 block) | 0 | no |
| Ice | 79 | 0 / 0 | full | 3 | no |
| Cactus | 81 | 0 / 0 | inset 1/16 on the four sides and the top (hurts on contact) | 0 | no |
| Sugar cane | 83 | 0 / 0 | none | 0 | no |
| Pumpkin | 86 : 0–3 (facing) | 0 / 0 | full | 255 | yes |

Notes: metadata 0 of id 31 is a "dead shrub" variant not produced by the
overworld generator. Leaves metadata also carries a "needs decay check"
bit (value 8) set at runtime; generation writes 0, 1 or 2. Water and lava
(existing palette) have light opacity 3 and 255 respectively for the
heightmap rule.

Suggested extension of the `Block` enum of section 6.2 (existing values
unchanged): `Ice = 7, OakLog = 8, BirchLog = 9, SpruceLog = 10,
OakLeaves = 11, BirchLeaves = 12, SpruceLeaves = 13, TallGrass = 14,
Fern = 15, DeadBush = 16, Dandelion = 17, Rose = 18, SugarCane = 19,
Cactus = 20, BrownMushroom = 21, RedMushroom = 22, Pumpkin = 23,
SnowLayer = 24` (ice replaces the "ice → Water" substitution of 6.2).
Bedrock, sandstone, clay, ores, lakes, dungeons and springs remain
substituted or omitted.

---

## 17. Quirks found in Part II

| # | Quirk | Effect | Recommendation |
|---|---|---|---|
| B1 | Biome lookup quantises T, H down to 1/63 steps via a precomputed table. | Biome borders are slightly blockier than the climate field. | Reproduce (cheap; use the integer table of 8.2). |
| B2 | Ice Desert is defined but unreachable. | None. | Keep unreachable. |
| B3 | One biome per population, sampled at the anchor-square centre. | Tree density/plant mix jumps on a 16-block grid offset by 8 from chunk lines; forest trees spill a few blocks into neighbouring plains. | Recommended deviation: decide per feature at its own column. |
| B4 | Tree-density noise sampled at the chunk corner, while trees go in the +8 square. | Density field shifted by 8 blocks. | Irrelevant once cells are used; sample at the cell centre. |
| B5 | Flower/mushroom/pumpkin/cactus/reed centres use a random height with no search. | Most patches fail; sparse plants; mushrooms in caverns. | Reproduce for look, or search the surface and divide counts by roughly 4. |
| B6 | Leaves are flagged "opaque cube" in the original's static table (the flag is captured during construction, before the leaves' graphics setting is assigned). | Trees never overwrite each other's leaves (birch leaves keep their colour inside an oak canopy) and mushrooms may grow on leaves. | Reproduce (treat leaves as opaque for these two checks). |
| B7 | Big-oak level index is off by one between shape (ℓ) and position (ℓ − 1). | Clusters one block lower than the profile suggests. | Reproduce (it is part of the shape). |
| B8 | Big oak does not convert its ground to dirt; small trees do. | Grass under big oaks. | Reproduce. |
| B9 | The oak/birch canopy coin is drawn for top-layer corners too, then ignored. | RNG parity only. | Ignore. |
| B10 | Sugar cane and cactus overwrite non-air blocks above their base. | Rare reed/cactus through leaves. | Optional; checking for air is fine. |
| B11 | Sugar cane accepts only grass or dirt, not sand. | No cane on sandy beaches unless dirt is exposed. | Reproduce (era-accurate). |
| B12 | Plants judge light from the purely vertical skylight computed at terrain time. | Flowers grow under shallow overhangs' edges only if sky is directly above. | Reproduce with the vertical rule of section 12. |

---

## 18. Uncertain items

- **Lighting during population.** The survival rules read the world's light
  value. Chunk creation computes a purely vertical skylight before
  population, but blocks placed during population (trees in particular) may
  queue relighting with sideways spread, and whether that runs before later
  steps of the same pass was not established. Section 12's vertical rule is
  the best reading; it may slightly differ under fresh canopies.
- **Leaves as opaque cube (B6)** follows from construction order in the
  client jar studied; a different build or graphics setting could differ.
  Its effect is minor either way.
- **Typical range of the tree-density noise.** Only the theoretical bound
  (±255 before dividing by 8) is certain; the practical spread of D, and so
  of forest tree counts, was not measured.
- **Exact definition of "normal cube" for the pumpkin's ground** was read as
  "opaque, non-translucent full block"; grass qualifies, which is all that
  matters because the generator already requires grass.
- **Lakes, dungeons, clay, ores and springs** are only summarised (order and
  attempt counts); their shapes are not specified here.
- **Spawn caps** (70 / 15 / 5) are the per-type caps from the creature-type
  definition; how they scale with loaded chunks at runtime is out of scope.

---

### Crop growth (Part III, farming)

Random-tick growth of a crop:
- The crop can grow only if the light at the block above it is at least 9. That light includes time-of-day darkening. This game gets a light value of its own (sky light with time-of-day darkening, blocked by solid blocks above), built as separate work, and crop growth waits for it. (Decided in #392.)
- Let f start at 1. Add 3 if the farmland under the crop is wet, or 1 if dry. "Wet" means moisture above 0.
- For each of the 8 farmland squares around it, add 0.75 if wet or 0.25 if dry. They count even with nothing planted on them.
- Halve f if any crop sits diagonally, or if crops lie along both the X and Z axes. A single straight row is not penalised.
- The chance per random tick is 1 in trunc(100 / f).

| Case | f | Odds per random tick |
|---|---|---|
| Single row, fully wet | 10 | 1 in 10 |
| Single row, dry | 4 | 1 in 25 |
| Full field, wet | 5 | 1 in 20 |
| Full field, dry | 2 | 1 in 50 |

Wheat has 8 stages, 0 to 7. Each success advances it by exactly 1, and stage 7 is the cap.

Uncertain items: the random-tick rate, whether freshly tilled farmland starts dry, and light under slabs or stairs.

---

### Sprinting (Part III, player movement)

Beta 1.7.3 has no sprinting. These rules are this project's own choices, not facts about the original. (Decided in #820.)

- **Sneaking.** Sprinting is disabled while sneaking: a sprint can't start while sneaking, and starting to sneak ends a sprint.
- **Water.** Sprinting is disabled in water: a sprint can't start in water, and entering water ends a sprint.
- **Walls.** Hitting a wall ends the sprint.
- **Camera.** While sprinting the view gets a slight fisheye effect: the field of view widens a little. The amount and how quickly it eases in and out are this game's own tunables in `data/input.toml`.

Not yet decided: whether any horizontal contact with a wall counts or only a head-on hit, and whether a sprint ended by sneaking or water resumes without a fresh double-tap.

#### Sprinting in the reference version

Facts about sprinting in the reference version (Java Edition 26.3, decided in #821), from local research on #819. Speeds are in blocks per tick unless marked otherwise. Where the project choices above differ, they win: in this game sneaking and water end a sprint, though the reference version lets a sprint continue while sneaking.

- **Speed.** Sprinting multiplies movement acceleration by 1.3, on the ground and in the air. Each value below is the base value times 0.98 for a held movement key.

  | | Walking | Sprinting |
  |---|---|---|
  | Ground acceleration per tick (base 0.1, sprint 0.13) | 0.098 | 0.1274 |
  | Air acceleration per tick (base 0.02, sprint 0.026) | 0.0196 | 0.02548 |

  Horizontal drag per tick is ×0.546 on ordinary ground and ×0.91 in the air. Top speed is about 0.2159 walking and 0.2806 sprinting on the ground (4.317 and 5.612 blocks/s), and about 0.2178 and 0.2831 in the air.
- **Double-tap window.** 7 ticks by default; a setting from 0 to 10 ticks, where 0 turns double-tap sprinting off. The window counts from the first forward press. With a window of *n* the second press must come at most *n* − 1 ticks after the first (6 at the default), and forward must be released in between, so the earliest second press is 2 ticks after the first. (The *n* − 1 and 2-tick figures were worked out from the per-tick countdown, not tested in-game.)
- **Holding forward.** A sprint ends on the first tick with no forward input, and that tick already moves at walking values. Holding forward and back together, or strafing with no forward key, counts as no forward input. Forward plus a strafe key keeps the sprint.
- **Walls.** Colliding with a solid block at more than 8° ends a sprint at once; shallower contact does not.
- **Sneaking.** Sneaking does not end a sprint; the ×0.3 sneak scale stacks with the ×1.3 sprint scale. Sneaking does stop a sprint from starting, by double-tap or the sprint key, unless underwater. Holding sneak, holding back, or using a slowing item cancels a double-tap waiting for its second press.
- **Other end conditions.** Attacking a mob and blocking with a shield end a sprint. Eating or drinking pauses it, and the sprint key can't start one meanwhile. (Hunger and Blindness also block sprinting; this game has neither.)
- **In the air.** A player stays sprinting while airborne as long as forward is held.
- **Sprint-jump.** Jump height is unchanged (upward speed 0.42, about 1.2522 blocks). On the tick the jump starts, if the player is sprinting on that tick, 0.2 of horizontal speed is added along the horizontal facing, ignoring pitch and which movement keys are held. Sprint state is updated before the jump within a tick, so releasing forward on the jump tick gives no push. On the jump tick the player still counts as on the ground: they get the ground sprint acceleration (0.1274) as well as the push, move the full 0.2 of the push that tick, and ground drag (×0.546) applies at the end of the tick, with air drag (×0.91) from the next tick on. A sprint-jump covers up to about 4.225 blocks and averages about 7.127 blocks/s on flat ground.
