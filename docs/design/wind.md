---
title: Wind from drifting fronts and gusts
status: accepted
scope: game
source: "#1725"
superseded_by: ""
phases:
  A: done
  B: todo
---

## Current state

Checked against `master` (f1c2f75).

- **Constant sway** is in `client/sway.js`: `SWAY_AMPLITUDE` 0.08, `SWAY_SPEED` 1.6, `SWAY_WAVE` 0.45, `TRUNK_SWAY_AMPLITUDE`, `swayPhase(nowMs, reducedMotion)` (0 under reduced motion), `swayOffset(weight, x, z, phase, amplitude)` and `trunkLean`. The direction is baked into the `sin`/`cos` terms.
- **Shader mirror** is `swayGlsl` and the `program(cut)` vertex shader in `client/scene.js` (~lines 88-124), fed by one `swayPhase` uniform (~line 592). `trunkLean` is already mirrored for the opaque program, so tree trunk sway (`tree-trunk-sway.md`) has partly landed.
- **Snow shake-off** is `spawnSnow` in `client/particles.js`; it calls `swayOffset(1, cx, cz, phase, 1)` and `scene.js` (~line 530) passes `swayPhase(now, false)` as `snow.phase` to `controller.frame`.
- **Sim clock:** `crates/game/src/clock.rs` has the fixed 20 Hz tick (`TICKS_PER_SECOND`) and `Step { ticks, alpha }`. `Walker::sim_tick()` (`crates/game/src/walk.rs`) returns the tick count. `replay.rs` replays sims.
- **Noise:** `crates/terrain/src/noise.rs` has `Simplex` (`Simplex::new(&mut Rng)`, `noise(x, y)`), and the game crate already depends on `terrain`. The sim PRNG is `crates/game/src/rng.rs`. `fish_spawner.rs` (`spawn_rng(seed, tick)`) is an existing pattern for deriving a deterministic stream from seed and tick.
- **Wasm exports** for the game are in `crates/game-web/src/lib.rs` (`wasm_bindgen`); the client reaches them through `client/controller.js`.
- **Data:** `data/*.toml` has no `data/wind.toml`. Game data files are read by the game crate with `toml` (see `data/player.toml`, `data/input.toml`).
- **Phase A done (plan #1940):** the wind function with fronts, gusts and direction wobble (`crates/game/src/wind_front.rs`, `wind_gust.rs`, `wind_field.rs`), `data/wind.toml` and the `game-web` `Wind` export are built, with tests for determinism, replay, range, calm and storm samples and unchanged climate. Closing PRs: #1978, #1981, #2052 (earlier tickets of plan #1853 built the fronts, `Wind` type and export).
- **Not built (as of the first reconcile; Phase A has since landed, see above):** no wind function, no fronts or gusts, no wind readout or pin command, no day cycle or weather. Climate output does not depend on any of this.
- Related docs: `tree-trunk-sway.md` (#1609), `biome-flora.md` (Phase C.5 introduced the sway), `debug-console.md` (command and alias tables in `data/commands.toml`).

## Reconciliation notes

- **SPEC 1.4 says simplex is "climate only".** That describes where Beta uses it, not a restriction on this project. Reusing the `Simplex` primitive for a project-owned field is fine; SPEC.md is not touched and no Beta fact is added. Climate stays unchanged as long as the wind builds its own `Simplex` from its own `Rng` stream.
- **Wind-specific seed.** `Simplex::new` draws offsets and a permutation from the `Rng` it is given (`terrain::noise::Rng`). Suggest seeding that `Rng` with `seed` XOR a named wind constant (a distinct constant per layer: pressure, gust), so no terrain or climate stream is shared. Add a test that climate samples for a fixed seed are identical with the wind module linked and queried.
- **Where the code lives.** The idea says the game crate with a wasm export. The game crate may use `std` and depends on `terrain`, so no invariant is at risk (`crates/terrain` is untouched). `Simplex` is `pub`, so no change to terrain is needed. The export goes in `crates/game-web/src/lib.rs`. Report the wasm size of `game-web` before and after.
- **Determinism and floats.** The wind uses `f64` noise and `sin`/`cos`/`atan2` style maths in the sim crate. The requirement "same output on every platform" is only exact if no platform-dependent libm call is on the path; native tests and wasm can differ in the last bits for transcendental functions. Suggest using only `+ - * /`, `floor` and `sqrt` (which are exactly specified), rotating the gradient by swapping components rather than with trig, and testing wasm against native with a small tolerance (1e-9) for anything that has to use more. Rendering can tolerate tolerance; the sim only needs it if gameplay later reads wind (a later idea), at which point it should compare bits. See open question 1.
- **Units.** The idea offers blocks per second or a unitless strength and direction. Suggest returning `(dx, dz)` in blocks per second from the game crate (the later gameplay ideas need that), and giving the client a derived `strength = clamp(|wind| / WIND_MAX, 0, 1)` and unit direction.
- **Time input.** Tick is an integer from the sim (`sim_tick`), but sway renders every frame. The function takes time as `f64` ticks (tick plus `alpha`), so the client passes `tick + alpha` and the sim passes a whole tick. The tick-only determinism test then covers integer inputs. (Decided in #1732.)
- **Gust coordinates that move with the wind.** Sampling gust noise at `p - w·t` needs the wind itself, which in turn comes from the front. Suggest using the front wind at the query position (one level, no recursion). Note this makes gust position drift without bound with time; at long play times `w·t` becomes large and f64 loses fine detail only after hours, which is acceptable, but wrap or cap if a profile shows otherwise. Gust noise is pure advection: gust = `Simplex((p - w·t) / gust_size)`, with no separate change over time, so the advection test below stays exact. There is no `gust_time_scale`; it is not in `data/wind.toml` or its loader. (Decided in #1857.)
- **Option A uniforms (one wind per frame).** The vertex shader has one `swayPhase` uniform and a fixed direction. Phase B must replace the baked direction by a direction uniform, an amplitude scale uniform and a spatial gust term; the `sway.js` JS mirror and `swayGlsl` must change together. The existing `plantfade.js` mirror-test pattern applies. Per-frame cost is trivial (one query at the player).
- **Phase of the sway wave.** Today the phase is `time × SWAY_SPEED` and the offset is a sine in `x + 0.6 z`. With wind, the wave should travel along the wind direction, and the phase should be accumulated from wind speed integrated over time (not `speed × t`) or the sway visibly jumps when the speed changes. Integrate on the client per frame; this is a visual accumulator and need not be deterministic. Add a note in the test that the accumulator is continuous when the wind changes.
- **Snow.** `spawnSnow` currently uses the squared sway at a cell. Suggest it take a gust-strength value from the same wind query (Phase B). `client/test/particles.test.mjs` already covers it.
- **Trunk sway (#1609).** `trunkLean` and `TRUNK_SWAY_AMPLITUDE` exist. Phase B must drive them from the same wind (same uniforms), so no second wave remains.
- **Reduced motion.** Already handled by `swayPhase`; Phase B must keep a zero result for it, and the wind readout may still show the real wind.
- **Debug console.** A readout and a pin command are commands in `data/commands.toml` / `crates/game/src/commands.rs` (see `debug-console.md`). A pin is state the sim reads, so it must live in the sim (settable only through the command queue) so replays record it; see open question 2.
- **Cost.** Four front samples (finite differences) plus the centre, and a gust sample or two per query: a handful of simplex calls. `[manual check]`, proposed measurable form: median frame time on a fixed flyover seed within 3% of before.
- **Fronts "take minutes of play to pass".** `[manual check]`. Measurable form: a unit test that the pressure field sampled at a fixed point changes by less than 10% of its range over 10 s, and has moved by a full feature wavelength after `front_size / drift_speed` seconds, from the `data/wind.toml` values.
- **"Gusts roll across a field in the wind direction".** `[manual check]`. Measurable form: the gust field's value at `p + w·dt` at time `t + dt` equals its value at `p` at `t` (advection test).
- **"Calm is possible".** Measurable: over a grid of positions and times for a fixed seed, some samples fall at the floor and some reach the ceiling from `data/wind.toml`.
- **World output:** unchanged. No block, height, biome or decoration depends on wind.

## Idea (author's text)

# Mince Raft: Wind from drifting fronts and gusts

Status: idea
Scope: game (client for the visible effects)

The world's wind is currently a constant sine: plants and leaves sway with one fixed phase, speed and direction everywhere, and the snow that shakes off foliage uses the same wave. This idea adds a real wind value that changes over space and time. It is built from two cheap layers: large **weather fronts** that drift across the world and set the wind's direction and strength, and small fast **gusts** on top. Wind is a pure function of seed, world position and sim tick, with no stored state, so it costs a few noise samples per query and every client, replay and region agrees on it. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `client/sway.js`: `swayPhase(nowMs, reducedMotion)` and `swayOffset(weight, x, z, phase, amplitude)`. Fixed `SWAY_AMPLITUDE`, `SWAY_SPEED` and `SWAY_WAVE`; the direction is baked into the `sin`/`cos` terms. The cutout vertex shader in `client/scene.js` mirrors it through a `swayPhase` uniform.
- `client/particles.js` `spawnSnow`: its "gust" is the squared `swayOffset` at a cell, so flakes come off at the sway peaks.
- `crates/game/src/clock.rs`: fixed 20 Hz sim tick. `replay.rs` replays sims, so anything the sim reads has to be deterministic.
- The terrain crate has octave simplex noise (SPEC 1.4) used for climate. The wind can reuse the noise primitive but must not reuse the climate seeds or change climate output.
- Tunables live in `data/*.toml`. There is no `data/wind.toml` yet.
- Related: #1609 (tree trunks sway in the wind) uses the same sway wave. Whatever replaces the wave should drive it too.
- There is no day cycle or weather yet.

## 2. Want

- Want: **one wind function**, `wind(seed, x, z, tick) -> (dx, dz)` in blocks per second (or a unitless 0..1 strength plus a direction), in the game crate and exported to wasm. Same inputs give the same output on every platform, to visual accuracy only: native and wasm may differ in the last bits, and the cross-platform test uses a tolerance of 1e-9. (Decided in #1731.)
- Want: **fronts.** A very low-frequency 2D noise field `P` (think pressure), sampled at a position that drifts with time: `P(x − u·t, z − v·t)`, where the drift `(u, v)` is itself constant or very slowly turning. Wind runs along the contours of `P` (its gradient rotated 90°), with speed proportional to the gradient's size. Steep regions are stormy and flat ones are calm. Front size (hundreds of blocks to a few thousand), drift speed and the speed scale go in `data/wind.toml`. A front should take minutes of play to pass over, not seconds.
  - Options for the gradient:
    - A. Finite differences: sample `P` at ±ε. That is four samples, which is simple.
    - B. An analytic gradient from the noise. Cheaper, but needs noise code that returns derivatives.
    - **A preferred** unless profiling says otherwise.
- Want: **gusts.** Faster, smaller-scale noise in space and time, multiplied onto the front's speed (e.g. `speed · (1 + g·gust)`, clamped ≥ 0) with a small extra direction wobble. Gusts should visibly roll across a field or forest in the wind's direction, not pulse everywhere at once, so gust noise is sampled in coordinates that move with the local wind.
- Want: **calm is possible.** Some areas and times are nearly still, so the world isn't always windy. A floor and ceiling on speed are set in data.
- Want: **the sway follows the wind.** Plant, leaf and (if #1609 lands) trunk sway lean in the wind's direction with an amplitude driven by wind speed. Snow shake-off follows gust strength. Options for getting wind to the shader:
  - A. Sample once per frame at the player and pass the direction, strength and gust phase as uniforms. Simplest: the whole loaded area shares one wind, and gusts still roll across it through the phase term.
  - B. Sample per region or per coarse cell and upload a small texture the shader reads by world position. Wind can then vary across the view.
  - **A preferred** for the first phase. Fronts are far larger than the view distance, so A loses almost nothing.
- Want: **reduced motion** still stops all sway, as today.
- Want: pure maths in testable modules, with a unit test for each layer. A client mirror goes in the `sway.js` / `plantfade.js` pattern, tested against the wasm values.
- Want: a debug console readout of the wind at the player (direction, speed, gust), and preferably a command to pin the wind for testing.

## 3. Phases (suggested)

- Phase A: the `wind` function (fronts + gusts), `data/wind.toml`, unit tests and the wasm export. No visible change.
- Phase B: sway and snow driven by the wind (option A uniforms); debug readout.
- Later, separate ideas: gameplay effects (wind pushing the raft, a sail, drift on dropped items), and extra layers that bias the same function: land/sea breeze, terrain shelter and channelling, biome roughness, and a day cycle.

## 4. Things to check

- **Determinism.** Wind must depend only on seed, world position and tick, never on the player's path, region origin or frame rate. Add a test that two queries at the same `(x, z, tick)` from different "player" contexts agree, and that a replay reproduces it.
- **Tick vs. frame time.** The sim is at 20 Hz, but sway renders every frame. The client should interpolate between ticks (the clock's `alpha`) so the motion doesn't step.
- **Seeding.** Use a wind-specific seed derivation so climate, terrain and decoration output are unchanged. **World output: unchanged.** No block, height, biome or decoration depends on wind.
- **Cost.** A handful of noise samples per query, a few queries per frame. Check it doesn't show up in `FrameTimes`.
- **Wasm size.** Report before and after. Reusing the terrain noise should keep it small.
- **Not Beta behaviour.** Beta has no wind. This is a project choice entirely; nothing goes in SPEC.md.

## Phases (reconciler)

The author's suggested phases in section 3 are used as written; the header lists them.

**Phase A.** The pure wind function. Covers: `data/wind.toml` (front size, drift, speed scale, gust scale and speed, floor and ceiling, wobble) read by the game crate; a `wind` module in `crates/game` with the wind-specific seed derivation, fronts (finite-difference gradient rotated 90°) and gusts advected by the local wind; a `game-web` export. Done when unit tests show: identical output for repeated queries and for a replayed run; the front field moves as the data says; gusts advect with the wind; speeds stay within the floor and ceiling, with both calm and stormy samples present for a fixed seed; climate output for a fixed seed is unchanged; and `./build.sh` succeeds with the `game-web` wasm size reported before and after.

**Phase B.** Visible effects. Covers: a client mirror and per-frame query at the player (`tick + alpha`); the direction, strength and gust-phase uniforms replacing the fixed sway direction for plants, leaves and trunks in `client/sway.js` and `client/scene.js`, with a continuous phase accumulator; snow shake-off driven by gust strength in `client/particles.js`; reduced motion still zero; the console readout (and pin command, if open question 2 is decided for it). Done when the sway and particle tests pass, the client mirror matches the wasm values, and an input script checks the readout and, if pinned, that sway follows the pinned direction. Waits on Phase A and on open question 2 for the pin.

## Open questions

2. **Where does the "pin wind" state live?**
   - A. In the sim, set by a console command through the command queue, so replays record it.
   - B. In the client only, a debug override of the uniforms; the sim wind is untouched.
   - C. Skip the pin; keep only the readout.
