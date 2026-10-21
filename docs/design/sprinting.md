---
title: Sprinting
status: accepted
scope: game
source: "#808"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Checked against the code:

- Player tick: `crates/game/src/player.rs` (`Player::tick`). Step 1 scales
  forward and strafe input by `input_scale`, times `sneak_scale` while
  `input.sneak`. There is no sprint state.
- Per-tick input: `crates/game/src/input.rs` (`TickInput`, with `move_forward`,
  `sneak` and others) and the accumulator that feeds it
  (`crates/game/src/sampler.rs`). `TickInput` has no sprint field.
- Tunables: `data/player.toml` `[movement]` (`input_scale`, `sneak_scale`, ...),
  loaded by `crates/game/src/physics.rs`; every key is required. Bindings:
  `data/input.toml` (`[actions.move_forward]` is `KeyW`, mode `hold`; `[look]`
  has `fov_degrees = 70.0`). Action names are mapped in
  `crates/game/src/bindings.rs`.
- Camera: `crates/game/src/camera.rs` takes `fov_degrees` as a constructor
  argument; nothing varies it per tick.
- Replays record per-tick input as text (`crates/game/src/replay.rs`, key
  `fwd` and others); a new input field needs a replay key.
- Client tests: `client/test/scripts/*.input`, DSL in `client/test/dsl.mjs`
  (`down`, `up`, `press`, `frame <ms> [xN]`, `expect`). The DSL has no sprint
  expectation yet.
- `SPEC.md` has no player physics or sprinting.

Nothing in this doc is built yet.

## Reconciliation notes

1. **Rule 3 of basic-gameplay.** `docs/design/basic-gameplay.md` rule 3 says
   "no sprinting". Section 1 below drops that phrase. This doc cannot edit
   basic-gameplay.md (the reconciler touches one file only); the edit to that
   rule should be part of Phase A's implementation PR's follow-up or done by
   the maintainer. Until then, this doc overrides that one phrase. Note 4 in
   that doc's own reconciliation notes already calls it a design target, not a
   Beta fact.
2. **No source for the "same as Minecraft" values.** Speed multiplier,
   double-tap window, what ends a sprint, and interactions with jumping,
   sneaking, water, walls and camera come from the original game, not
   `SPEC.md`, and Beta 1.7.3 has no sprinting. Per CLAUDE.md these need
   `research` spec-questions. Phase A therefore uses this game's own values in
   `data/player.toml` `[movement]` and `data/input.toml`, as #90 did for the
   existing movement constants, and makes no parity claim.
3. **Determinism.** Double-tap detection must depend on tick counts, not wall
   clock. Count the ticks since the last forward press edge in the sim (or the
   sampler, in ticks), so replays and input scripts reproduce it. The window is
   then a tick count in data, not milliseconds.
4. **Input record.** Either `TickInput` gains a `sprint` flag computed by the
   input layer, or the sim derives the state from a forward press edge field.
   The first is smaller and keeps `Player::tick` a function of its input; the
   replay format needs a matching key.
5. **Sprint state placement.** The state lives on `Player` (like `on_ground`),
   so it survives between ticks; ending conditions are evaluated in `tick`.
6. **Camera.** Any FOV change while sprinting needs `Camera` to accept a
   per-frame FOV; that is a separate, optional step (open question 2) and is
   `[manual check]` if judged by look. A measurable version: the FOV value in
   a controller state, checked by an input script `expect`.
7. **Raft scene.** `crates/game/src/raft.rs` and the raft test scene use the
   same `Player::tick`; whether sprint applies there is open question 3.
8. **Invariants.** `crates/terrain` is untouched, so `no_std` and wasm size are
   unaffected for the terrain wasm; `crates/game-web` grows slightly if the
   flag is exposed. Report sizes in the PR.

---

# Mince Raft: Sprinting

Status: idea
Scope: game

I want simple sprinting: double-tap the move-forward key to sprint. Otherwise sprinting should behave the same as in Minecraft, except that there is no hunger: sprinting costs nothing and is never blocked by a food level. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. Loosen "no sprinting"

basic-gameplay rule 3 says "Movement should feel like Beta: no sprinting, no attack cooldown, sneak prevents walking off edges." Drop "no sprinting" from that rule. The rest stays.

## 2. Double-tap forward to sprint

- Tapping `move_forward` twice in quick succession, and holding it after the second press, starts a sprint.
- Sprinting is a player state in the sim. It's driven by input like everything else, so it stays deterministic (design rule 6) and can be tested with input scripts.
- Keep it to double-tap. No separate sprint key for now.

## 3. Same as Minecraft, without hunger

Everything else about sprinting should match the original game: how fast you go, how long the double-tap window is, what ends a sprint, how it interacts with jumping, sneaking, water and walls, and any camera change while sprinting. Leave out anything to do with hunger or food.

- `SPEC.md` has no player physics and no sprinting, and Beta 1.7.3 itself has no sprinting. None of this can come from memory: per CLAUDE.md, each of these needs a `research` spec-question, and the answers reach `SPEC.md` through the maintainer. The research describes a current release of
  the original game as the reference version. (Decided in #821.)
- Until then, any tunables go in `data/player.toml` (`[movement]`) and `data/input.toml` as this game's own values, as the existing movement constants are (#90).

## 4. Tests

- Input scripts in `client/test/scripts/` that start a sprint with a double-tap and check speed, that a slow second tap doesn't start one, and that each condition that should end a sprint does.

## Phases (reconciler)

**Phase A.** Sections 1, 2 and 4 with placeholder values: a `sprint` state on
`Player`, driven by a tick-counted double-tap of `move_forward` (note 3),
a speed multiplier and window length in `data/player.toml` / `data/input.toml`,
a replay key, and the DSL support to `expect` sprint state. The sprint ends
when forward is released, when sneaking starts, and on a blocked-forward
collision (the author's "walls"); other end conditions wait for research. Done
when input scripts show a double-tap starting a sprint with higher speed, a slow
second tap not starting one, and each listed end condition ending it. Also
files the `research` spec-questions of section 3 (what the original does, per
the reference version in section 3).

**Phase B.** Section 3 items that wait on open questions: camera FOV change
(question 2), raft and water behaviour (question 3), and a configurable window
(question 4), plus replacing placeholder values with researched ones once
`SPEC.md` has them. Done when each decided behaviour has an input script.

## Open questions

2. **Camera.** Match the original's FOV change while sprinting? A: yes, with a
   tunable in `data/input.toml`. B: leave the camera alone.
3. **Raft and water.** Does sprinting apply on the raft scene, and what happens
   entering water? A: land only; sprint ends on entering water or boarding.
   B: works everywhere the player walks, and ends in water. C: as the original
   (needs research).
4. **Double-tap window.** A: fixed value in `data/input.toml` (not a user
   setting). B: a user setting in the menu.
5. **Which sprint end conditions are placeholders in Phase A?** A: forward
   release, sneak, wall hit only. B: also stopping in water, until question 3
   is answered.
