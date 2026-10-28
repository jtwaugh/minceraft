---
title: Sprinting
status: accepted
scope: game
source: "#808"
superseded_by: ""
phases:
  A: done
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

Phase A plan #815 closed (PRs #842, #868, #876, #877, #902, #908):

- Tunables: `sprint_scale` in `data/player.toml`, `[sprint]` `double_tap_ticks`
  in `data/input.toml` (`crates/game/src/bindings.rs`, `physics.rs`).
- Double-tap in ticks: `crates/game/src/input.rs` sets `TickInput.sprint`;
  replay bit 7 in `crates/game/src/replay.rs`.
- State: `Player.sprinting` in `crates/game/src/player.rs`; ends on forward
  release, sneak, water and a blocked-forward hit (unit tests cover each).
- Client: `walk.rs` exposes `sprinting()`; `client/test/scripts/sprint.input`
  covers speed, double-tap, slow tap, release, sneak and wall.
- `[sprint]` also has `fov_extra_degrees` and `fov_ease_seconds` in
  `bindings.rs` (Phase B, question 2).
- Water ending a sprint now has an input-script case (plan #1289, PR #1299,
  `client/test/scripts/sprint.input`), so Phase A's end conditions are all covered.
- Research spec-questions #819 and #820 and decisions #821 and #822 are closed.

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
- Sprinting is land only: a sprint ends on boarding the raft or at the helm, and cannot be started on the raft scene. (Decided in #1333.)

## 4. Tests

- Input scripts in `client/test/scripts/` that start a sprint with a double-tap and check speed, that a slow second tap doesn't start one, and that each condition that should end a sprint does.

## Phases (reconciler)

**Phase A.** Sections 1, 2 and 4 with placeholder values: a `sprint` state on
`Player`, driven by a tick-counted double-tap of `move_forward` (note 3),
a speed multiplier and window length in `data/player.toml` / `data/input.toml`,
a replay key, and the DSL support to `expect` sprint state. The sprint ends
when forward is released, when sneaking starts, on a blocked-forward
collision (the author's "walls"), and, as a placeholder until open question 3
is answered, while the player is in water; other end conditions wait for
research. Done
when input scripts show a double-tap starting a sprint with higher speed, a slow
second tap not starting one, and each listed end condition ending it. Also
files the `research` spec-questions of section 3 (what the original does, per
the reference version in section 3). (Decided in #822.)

**Phase B.** Section 3 items that wait on open questions: camera FOV change
(question 2), raft and water behaviour (question 3), plus replacing
placeholder values with researched ones once `SPEC.md` has them. The
double-tap window stays a fixed value in `data/input.toml`, not a user
setting. Done when each decided behaviour has an input script.
For the sprint-jump, ground drag applies on the jump tick for sprint-jumps
only; ordinary jumps keep the drag they have today. (Decided in #1332.)
A wall ends a sprint only on a head-on hit: the facing within a small angular
tolerance of the wall's normal. The tolerance is this game's own tunable in
`data/player.toml`, with no parity claim; contact outside it, such as sliding
along a wall, does not end the sprint. (Decided in #1331, #1334.)

## Reconciliation notes (phase-done, Phase A)

9. **Resolved (plan #1289, PR #1299).** Was: Phase A still incomplete. Phase A is done when input scripts show every
   listed end condition ending a sprint. `client/test/scripts/sprint.input`
   has cases for release, sneak and wall, but none for water; the water rule
   (`player.rs`, decided placeholder in #822) is covered only by Rust unit
   tests. Add a water case to the input scripts (extend the DSL if it cannot
   place the player in water), then mark Phase A `done`. Plan #815 noted its
   tickets assumed water does not end a sprint, which is why none covers it.

## Reconciliation notes (phase-done, Phase B)

10. **Phase B still incomplete (plan #1326).** Plan #1326's tickets (#1327,
   #1329, #1330) closed: the double-tap window of n-1 ticks, the sprint-jump
   push (`sprint_jump_boost` in `data/player.toml`, `player.rs`) and input
   scripts (`sprint_ref.input`, `sprint_extras.input`). The decisions made
   in its questions were not built, and no ticket covered them:
   - Head-on wall rule (#1331, #1334): `Player::tick` still ends a sprint on
     any loss of velocity along the facing (`lost_along_facing`); there is no
     angular tolerance tunable in `data/player.toml`, and no input script for
     sliding along a wall.
   - Jump-tick drag (#1332): sprint-jumps should apply ground drag on the
     jump tick, ordinary jumps keeping today's drag; `player.rs` has no such
     distinction.
   - Raft (#1333): a sprint should end on boarding the raft or at the helm
     and not start on the raft scene; `walk.rs` and `raft.rs` do not clear or
     block `Player.sprinting`.

## Open questions

2. **Camera.** Match the original's FOV change while sprinting? A: yes, with a
   tunable in `data/input.toml`. B: leave the camera alone.
