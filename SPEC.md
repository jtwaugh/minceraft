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
(The original also builds one more 8-octave generator for tree density,
which is out of scope.) The three climate generators have their own RNGs,
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
biome uses grass on top and dirt as filler. (Other biomes only matter for
decoration and colour, which are out of scope.)

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
see 7.3):

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
