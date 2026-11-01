---
title: Day and night cycle
status: accepted
scope: game
source: "#1748"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Checked against `master` (731a1ff).

- **Phase clock:** `crates/game/src/phase.rs` is the gathering/service clock (`PhaseClock`, lengths from `data/phases.toml`, 120 s each, pure function of the tick). `crates/game/src/clock.rs` has the fixed 20 Hz tick (`TICKS_PER_SECOND`). The wasm side exposes `phase()` and `phase_seconds_left()` in `crates/game-web/src/lib.rs`; the HUD shows them (`client/hud.js`, `timer`).
- **Sun:** `SUN_DIR` is a constant in `client/view.js:30`, returned as `sunDir` by the view and passed to the sky pass and the main program in `client/scene.js` (uniforms at ~lines 563 and 583).
- **Sky colour:** `skyColor(base, dir, sunDir)` and its GLSL mirror are in `viewer/sky.js`; the sky pass, fog (`fogFar`) and water reflection in `client/scene.js` (~lines 159-195) all call it. `client/skyblend.js` eases the base colour from `sky_ramp` in `data/biomes.toml`.
- **Tint path:** the main program has a per-vertex `tint` attribute and a `tint` uniform (`client/scene.js`); shade and AO are baked per face in `crates/mesh`. No scene-wide light level or per-block sky light exists.
- **Console:** commands are defined in Rust in `crates/game/src/commands.rs` (`tp`, `give`, `spawn`, `pause`, `resume`, `step`, ...), not in a data file. `data/commands.toml` holds only cheat-code aliases (`[[alias]]`).
- **Reduced motion:** `client/scene.js:524` reads `prefers-reduced-motion`; it zeroes sway and water frames only. Nothing day-related is affected yet.
- **Not built:** no `time_of_day`, `sun_dir`, `daylight`, `data/day.toml`, `time` command or time readout.
- **Waiting on this:** `villages-and-villagers.md` (note 10, deferred home-at-night), `fish-and-fishing.md` (spawning ignores time of day), `world-layout.md` Phase C (fish query omits time of day), `wind.md` (no daily swing), SPEC "Crop growth" (decided in #392, waits on a separate sky-light value).

## Reconciliation notes

- **Commands in data (idea section 1):** the idea says `data/commands.toml` is where a `time` command goes. Commands are registered in `crates/game/src/commands.rs`; the data file only has aliases. Add `time` there, with its tests, like `tp` and `spawn`.
- **Where the sim state lives:** `time_of_day(tick)` is a pure function of the tick and data, so it needs no saved state, and replay and the world hash are unaffected. A `time set` or freeze command breaks that purity: it needs a sim-side offset or freeze flag, which must be part of what replay records. Smallest way: a `u64` tick offset in the sim, changed only by the command, so the function stays `(tick + offset) mod day_ticks`. `time set` and freeze live in the sim, not only in the client: the offset (and a freeze flag) is changed only by the command and is recorded with the replay commands, so sim consumers such as crops see it. (Decided in #1759.)
- **Precision (idea section 4):** compute in integer ticks (`(tick + start_offset) % day_ticks`) and only then divide to get 0..1. Interpolation between ticks uses the `Step.alpha` from `clock.rs`; the client must add `alpha / day_ticks` to the fraction and wrap, not read wall-clock time.
- **Crate boundaries:** `crates/terrain` is untouched (world output unchanged). `time_of_day`, `sun_dir` and `daylight` use trig, so they go in `crates/game` (std is allowed there), with a wasm export in `crates/game-web`. The client mirror of `skyColor` in `viewer/sky.js` is plain JS and GLSL, no framework. Report wasm size before and after, as CLAUDE.md requires.
- **Viewer sharing:** `viewer/sky.js` serves both the viewer and the client. The viewer has no clock, so any new night/dusk blend must default to the current look (daylight = 1) when no time is given, or the viewer's output changes.
- **Beta facts / SPEC:** SPEC.md gives no day length, sun path or night curve, and this doc takes none from Beta. The crop rule "light at least 9 including time-of-day darkening" ties a number to the darkening curve. That is Open question 1; it must become a `spec-question` issue, not be filled in here. Nothing in this doc goes in SPEC.md.
- **Overlap with `wind.md`:** wind's daily swing wants the same time function. Keep `day.toml` and `time_of_day` free of wind, and let wind read it later. Phase A here should land before wind's phase that uses a daily term.
- **Overlap with `villages-and-villagers.md` note 10 and `fish-and-fishing.md`:** both defer to a day cycle. This doc only provides the hook; wiring each is a separate idea or ticket, as the author says.
- **`[manual check]` items:** see tags below. No automatic test can judge night readability or how dawn and dusk look.
- **No `[external]` items.**

## Idea (author's text)

# Mince Raft: Day and night cycle

Status: idea
Scope: game (client for the sky, sun and light)

The world is always mid-afternoon. The sun direction is a fixed constant, the sky colour depends only on biome and temperature, and nothing in the sim knows what time it is. Several accepted docs and spec decisions are waiting on a day and night cycle: crop growth needs light that darkens with time of day (SPEC "Crop growth", decided in #392), villagers should go home at night (villages-and-villagers note 10), fish spawning has a time-of-day filter left out (world-layout Phase C, fish-and-fishing), and wind (#1725) wants a daily swing later. This idea adds one time-of-day clock in the sim and makes the sky, sun and scene brightness follow it. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `crates/game/src/phase.rs`: the "day phase clock". Gathering and service alternate from tick 0 (120 s each, `data/phases.toml`; basic-gameplay section 12, decided in #470). It's a pure function of the tick, so replays and the world hash stay deterministic. This is a work/service rhythm, not daylight.
- `crates/game/src/clock.rs`: fixed 20 Hz sim tick.
- `client/view.js`: `SUN_DIR` is a constant vector, passed as `sunDir` to the sky pass and fog in `client/scene.js`.
- `viewer/sky.js` `skyColor(base, dir, sunDir)`: sky gradient plus a sun tint, shared by the sky pass, above-water fog and water reflection (GLSL mirror in the same file).
- `client/skyblend.js`: sky base colour is the mean `sky_ramp` (in `data/biomes.toml`) over columns around the player, eased over `SKY_EASE_SECONDS`.
- Mesh vertex shade and AO are baked per face (`crates/mesh`). There is no scene-wide light level and no sky-light value per block yet. The "separate work" light value from #392 hasn't been built.
- Debug console (`docs/design/debug-console.md`, `data/commands.toml`) is where a "set time" command would go.

> **Reconciler:** all of this matches the code, except that console commands live in `crates/game/src/commands.rs`, not `data/commands.toml` (see Reconciliation notes).

## 2. Want

- Want: **one time-of-day function** in the game crate: `time_of_day(tick) -> f64` in 0..1 (0 = midnight, or another documented origin), a pure function of the tick like `phase.rs`. The day length and the starting time of day at tick 0 go in data (a new `data/day.toml`, or a section of `phases.toml`). It's exported to wasm.
- Want: **decide how it relates to the gathering/service clock.** Options:
  - A. Independent: the day has its own length, and phases keep cycling on their own.
  - B. One day = one gathering + one service (240 s now). Gathering is daytime and service is evening/night, so the HUD timer and the sky tell the same story.
  - C. One day = N phase cycles.
  - **B preferred** if service is meant to feel like "back at the raft in the evening". Otherwise A. This is the main thing for the maintainer to decide.
- Want: **derived values** from the time, each a pure function with a unit test:
  - `sun_dir(t)`: a sun that rises, crosses the sky and sets. The tilt and azimuth of its path go in data. Below the horizon at night.
  - `daylight(t)`: 0..1 scene brightness with a smooth dawn and dusk, not a hard switch. Night is dark but playable: there's a minimum brightness in data.
  - Optional `moon_dir(t)`, opposite the sun, for a faint night tint.
- Want: **the client follows it.**
  - `SUN_DIR` becomes `sun_dir(t)`.
  - Sky base colour is the biome `sky_ramp` mix blended toward night and dusk colours by `daylight`. The dusk and night colours are this project's own, in data.
  - Fog and water reflection follow, since they already share `skyColor`.
  - Terrain, plants and entities get multiplied by `daylight` (a uniform, through the existing `tint`-style path), so baked shade and AO stay as they are.
  - The client interpolates between ticks so the sun doesn't step.
- Want: **sim consumers get a hook, not new behaviour.** The function is there for crop light (#392), villager night behaviour, the fish time-of-day filter and wind. Wiring each of those up is its own ticket or idea, not this one.
- Want: debug console `time` command to show and set the time of day (or freeze it) for testing, and the time shown in the debug readout.
- Want: reduced motion doesn't stop the day cycle (it isn't motion in that sense). Confirm.

> **Reconciler:** reduced motion: confirmed from the code. `reducedMotion` in `client/scene.js` only gates sway and water animation; a slow sun is not motion in that sense, and the cycle should ignore the flag. Tag the "dark enough to feel like night" and "dawn and dusk look smooth" judgments `[manual check]`; a measurable proxy is a test that `daylight` is monotonic through dawn and dusk and never below the data minimum.

## 3. Phases (suggested)

- Phase A: `time_of_day`, `sun_dir`, `daylight`, data file, tests, wasm export, console command. No visible change.
- Phase B: sky, sun, fog and scene brightness follow it.
- Later, separate: the per-block sky-light value from #392 (sky light blocked by solid blocks, darkened by `daylight`), crop growth waiting on it, villagers going home, the fish filter, and night-time sources such as torches or lanterns.

> **Reconciler:** the author's Phases A and B are kept. Phase A adds the debug readout of the time (the `time` command and the readout need only the sim function). The "Later" items are other ideas or tickets, not phases here, so the header lists only A and B.

## 4. Things to check

- **Beta facts.** SPEC.md doesn't give Beta's day length, sun path or how much light darkens at night. Day length, sun path and visuals are project choices and don't need them. But the crop rule "light ≥ 9 including time-of-day darkening" (SPEC "Crop growth") ties a number to the darkening curve. Whether that darkening must match Beta is a `spec-question` (`research` if yes, `decision` if this project picks its own curve). Don't fill it from memory.
- **Determinism.** Time of day depends only on the tick and data, never on frame rate or wall-clock time. A replay reproduces it.
- **Saved games / long sessions.** Tick is a `u64`. Make sure the time maths keeps precision over long sessions (use the tick modulo the day length, not a float of the raw tick).
- **The raft scene and menus.** Check whether the root menu backdrop and any fixed scenes (raft-scene-feedback) should freeze at a nice time of day or follow the clock.
- **Night readability.** Dark enough to feel like night, never so dark the HUD or nearby blocks can't be read. Make the minimum brightness tunable, and the maintainer judges it by eye. `[manual check]`
- **Cost.** A few trig calls per frame, plus one uniform. Wasm size before and after.
- **World output: unchanged.** No block, height, biome or decoration depends on time of day.
- **Not Beta behaviour by default.** Except for the crop-light question above, this is a project choice, and nothing goes in SPEC.md from this doc.

> **Reconciler:** the HUD is drawn in 2D by `client/hud.js` and is not multiplied by the scene tint, so it stays readable at night by construction; apply `daylight` only to the world pass.

## Phases (reconciler)

**Phase A.** Sim function and data, no visible change. Add `data/day.toml` (day length in seconds, start time of day, sun path tilt and azimuth, minimum daylight, dawn and dusk widths) and a `DayClock` in `crates/game` (next to `phase.rs`, with a `DataFile` entry for errors) giving `time_of_day`, `sun_dir` and `daylight` as pure functions of an integer tick. Export them from `crates/game-web`. Add a console `time` command in `crates/game/src/commands.rs` (show, set, freeze) and the time in the debug readout. Done when unit tests cover: `time_of_day` wraps at the day length and is exact at a very large tick (for example near `u64::MAX / 2`); `sun_dir` is a unit vector, above the horizon by day and below it at night; `daylight` is smooth, never below the minimum, and 1 at noon; the command changes the value and a replay with the same commands reproduces it; and the data file rejects non-positive lengths.

**Phase B.** Client follows the clock. Replace `SUN_DIR` in `client/view.js` with the wasm `sun_dir`, interpolated by the step alpha; blend the sky base toward the data night and dusk colours by `daylight` (in `client/skyblend.js` or a sibling), keeping `viewer/sky.js` unchanged when no time is given; multiply terrain, plants and entities by `daylight` through one uniform in `client/scene.js`. Decide the root menu backdrop and raft scene (Open question 3) here. Done when client tests (the input-script DSL in `client/test/`, no screenshots) show the sun direction and daylight uniform at several set times, that the sky base is the biome mix at full daylight, and that `preview.mjs` and the viewer still run; the look itself is a `[manual check]`. State the wasm size before and after and that world output is unchanged.

## Open questions

1. **Does the crop-light darkening need to match Beta?** SPEC "Crop growth" says "light at the block above is at least 9, including time-of-day darkening". A. Project's own curve (a `decision` spec-question; the planner opens it when crop growth is planned). B. Match Beta (a `research` spec-question for the maintainer's machine). Not needed for Phases A and B here.
2. **How does the day relate to the gathering/service clock?** A. Independent lengths. B. One day = one gathering plus one service (240 s now), gathering is day and service is evening and night (author's preference if service means "back at the raft"). C. One day = N phase cycles. Note that under B, changing `data/phases.toml` also changes the day length, so `day.toml` should derive from it or assert equality.
3. **Do the root menu backdrop and fixed scenes follow the clock?** A. Follow the clock. B. Freeze at a chosen time of day, set in `data/day.toml`. Suggest B for the menu so it looks the same each visit.
