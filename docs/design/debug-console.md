---
title: Debug console
status: accepted
scope: game
source: "#309"
superseded_by: ""
phases:
  1: todo
  2: todo
  3: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **Sim tick.** `Walker::tick(&mut self, input: &TickInput)` in
  `crates/game/src/walk.rs` is the one 20 Hz step. It is where the command
  queue would drain (Phase 1). It applies `input.tool_slot` first, then
  movement, items and animals.
- **Belt, hands, items.** `PlayerState` (`crates/game/src/player.rs`),
  `items.rs` and `data/items.toml` hold the belt, hands and item definitions
  that `give` and `clear` act on. Item entities are in `item_entities.rs`
  and animals in `animal_entities.rs`, which `spawn` would add to.
- **Sim RNG.** `crates/game/src/rng.rs` is the deterministic sim RNG the
  doc's spawn scatter needs.
- **Browser wiring.** `crates/game-web/src/lib.rs` exports `InputSampler`,
  `Scene`, `Walk` and friends. It has no command exports. `client/input.js`
  and `client/controller.js` run the input states; `client/scene.js` reads
  `?scene=` and `?mode=` from the query string.
- **Test-scene stubs.** `client/overlay.js` has `spawnTestItems(walk)` and
  `spawnTestAnimals(walk, kinds)`, called from `client/scene.js` when a
  scene is set. These are what `spawn` would replace.
- **Input script DSL.** `client/test/dsl.mjs` and
  `client/test/scripts/*.input` exist. The DSL has `spawn animal <kind>`,
  `held`, and the paths `mode`, `hands`, `belt.slot.<1-5>`, `items`. It has
  no `type` statement and no `mode = console`.
- **Not present:** any `commands.rs` or command registry, `data/commands.toml`,
  a `?debug=1` flag or any debug-mode gate, a pause or step facility, a
  `cheated` flag, an input recording format, a world hash, a teleport
  function, and a console UI. Nothing in the repo reads `?tool=`.

## Reconciliation notes

1. **Section numbers skip.** The idea jumps from section 2 (which holds only
   the input-state table and a trailing paragraph about later commands) to
   section 6, and its text refers to the command table, `give`/`spawn`/
   `clear` arguments, `tp`, pause and `step` that are never defined.
   Sections 3 to 5 are not reconstructed here. The exact syntax and
   semantics of each command must be settled when Phase 1 is planned (open
   question 6).
2. **`?tool=` does not exist.** Section 2 calls `spawn` the replacement for
   "the test scene's `?tool=` stub". The code has no such parameter; the
   stand-ins are `spawnTestItems` and `spawnTestAnimals` in
   `client/overlay.js`. Suggest replacing those once `spawn` exists.
3. **Dependency on #224.** Phase 2 says it depends on basic-gameplay Phase 5's
   browser wiring (#224). `basic-gameplay.md` marks Phase 5 done (belt in the
   browser test scene, #249), so that dependency is met.
4. **Rule 1 versus how the sim is driven.** `Walker::tick` takes only a
   `TickInput`. A queue on `Walker` drained at the top of `tick` fits, but
   `give`, `spawn`, `clear` and `tp` touch `PlayerState`, the item store, the
   animal store and the player position, which are separate fields of
   `Walker`. The handlers must live in `crates/game` and get `&mut Walker`
   (or the pieces); none may be done from `client/`.
5. **Pause and `step`.** "No ticks pass while paused" is satisfied by the
   client simply not calling `Walker::tick`, but then queued commands would
   not apply (including `resume`). Suggest the pause state lives in the
   Rust command layer, with `step n` as the only way to tick while paused,
   so headless tests cover it.
6. **Replay.** The recording format and world hash it extends
   (basic-gameplay 2.2 and its Section 14 replay test) do not exist yet;
   the basic-gameplay replay test is in its Phase 8 or later. Phase 3's
   replay test therefore has to build the minimal recorder and a world hash
   itself, or wait for that work. Suggest it defines both, small, in
   `crates/game`, and that basic-gameplay's replay test reuses them.
7. **`data/commands.toml` and CLAUDE.md.** Data in `data/*.toml` is read at
   compile time by `crates/terrain/build.rs`. `crates/game` is not `no_std`,
   so it can load its own data the way `data/items.toml` and
   `data/interactions.toml` are loaded; no conflict. The "aliases can't
   shadow built-in names" check follows the same load-time validation as
   interactions against items.
8. **Wasm size (open question 3).** CLAUDE.md says wasm size matters. The
   registry, parser and handlers add to `game-web`; the Phase 1 size report
   is required in the PR anyway. A cargo feature would also have to be set
   by `build.sh`, which is outside this doc.
9. **Determinism.** Rule 1, "spawn scatter uses the sim's seeded RNG", and
   the tick-boundary rule are consistent with the determinism invariant.
   Terrain is unaffected (`crates/terrain` is not touched).
10. **Overlap with other docs.** No conflict with `basic-gameplay.md`: the
    console is additive and does not change its phases. The `cheated` flag
    and "future scores" overlap with nothing yet. `error-and-state-conventions.md`
    may be relevant to the outcome buffer and error text; the planner should
    read it before Phase 1.
11. **Beta claims.** None. The doc states no Beta behaviour, and `SPEC.md`
    is not involved.
12. **`[manual check]` criteria.** "Faded after a few seconds" and the
    overall console look (section 8) cannot be tested by a script and are
    `[manual check]`. A measurable version: the fade delay is a constant in
    `client/` and the input scripts check that output lines are retained
    for the page session. Phase 2's "you can `give` and `spawn` in the test
    scene" is covered by the `items.count` and `hands` expectations.
13. **`[external]`:** nothing. Everything is built in this repo.
14. **Input-key detail.** The console key is written as a backtick (`` ` ``).
    In `KeyboardEvent.code` terms that is `Backquote`, which `data/input.toml`
    does not bind today. Use the code, not the character, so layouts differ
    only by the label.

## Mince Raft: Debug console

Status: idea
Scope: game

A debug console: a text box opened in-game where typed commands change the game's state. The first job is spawning items and tools so later phases (animals, crops, stations) can be tested without playing up to them. The console is built as a general command layer, so new debug commands and cheat codes are one table entry and one handler, not a new code path.

---

## 1. Design rules

1. **Commands are sim input, not side doors.** A command never mutates game state from JavaScript or mid-tick. The client queues it, and the sim applies it at the next tick boundary, before that tick's `TickInput`. This keeps rule 6 of basic-gameplay (same seed plus same input gives the same state) true with commands in play.
2. **Commands live in Rust, the console lives in `client/`.** Parsing, validation and execution are in `crates/game` so they're testable headless and shared by every front end. The client only owns the text box, history and display.
3. **One registry.** Every command, debug or cheat, is an entry in a registry with a name, aliases, argument schema, help text, the game states it's valid in, and whether it counts as a cheat. Adding a command means adding an entry and a handler.
4. **Commands respect the game's rules unless they say otherwise.** `give` follows the belt and hands rules (tools on the belt, one item in the hands). Commands that break a rule on purpose (teleport, freeze time) are marked as cheats.
5. **Debug only.** The console is off unless debug mode is on (see section 6). Nothing in normal play opens it.

---

## 2. The console as a client mode

The client today has three input states, run by `client/input.js` and `client/controller.js`:

State | Entered by | Keys go to
-- | -- | --
Unlocked | page load, Escape, blur | nothing (the sampler holds no keys)
Locked, walking | click the canvas | the sampler, via data/input.toml
Locked, flying | F while locked | the sampler, driving the fly camera

Later phases add their own commands with their own tickets: `spawn cow`, `grow` (advance the aimed crop a stage), `phase gathering|service`, `time <seconds>`, `heal`.

`spawn` is how you get more than one of anything, because the hands only hold one item. It's also the debug replacement for the test scene's `?tool=` stub and the scattered items in `client/`.

> **Reconciler:** the text between the table and "Later phases" (the console
> mode itself, its key, and how it differs from the three states) is missing
> from the idea. See note 1 and open question 6.

---

## 3. Phase 1 commands

All four are valid in a single game state, "playing". Names are case-sensitive snake_case, as in `data/items.toml` and `data/animals.toml`.

- `help`: lists the registered commands.
- `give tool <name>` and `give item <name>`: give one unit of the named tool or item. The kind is required.
- `spawn <name> [count]`: `<name>` is an item or an animal species, and `count` defaults to 1. Each entity appears 2 blocks in front of the player, scattered within a radius of 2 blocks, with the scatter drawn from the sim RNG. The count is at most 64; a larger count is an error and spawns nothing.
- `clear belt|hands|items|animals`: `belt` restores the v0 belt, `hands` empties the hands, `items` removes all loose item entities, and `animals` removes all animals.

(Decided in #314.)

---

## 6. Debug mode and cheats

- **Debug mode** is on when the page loads with `?debug=1`, or always in the test scene. With it off, the console keys do nothing.

- **Cheats.** Commands marked as cheats set a `cheated` flag on the session the first time one runs. The flag is shown in the debug overlay and recorded with replays and any future scores. Non-cheat commands (`help`, `pause`, `step`) don't set it.

- **Cheat codes and aliases** live in `data/commands.toml`: a code expands to one or more command lines. For example:

  ```toml
  [[alias]]
  name = "kitchen"
  expands = ["give hoe", "spawn seeds 8"]
  ```

  Typing `kitchen` runs both lines in the same tick, in order. Aliases can't shadow built-in command names (checked at load, like item names against interactions).

> **Reconciler:** "always in the test scene" needs a definition: the test
> scenes are `client/index.html` with `?scene=` (see `client/scene.js`).
> Suggest debug mode is on whenever `scene` is set or `debug=1` is present.

---

## 7. Replay and determinism

Recorded input sequences are the basis for replay tests (basic-gameplay 2.2). The recording format gains command lines, stored per tick:

```
tick 1203: input {...}; commands ["give hoe", "spawn seeds 3"]
```

Replaying the recording runs the commands at the same ticks, so a cheated session replays exactly. Spawn scatter uses the sim's seeded RNG, never `Math.random` or wall time.

Test: a recorded session that uses `give`, `spawn`, `tp` and `pause/step` produces the same world hash on two runs.

> **Reconciler:** no recording format or world hash exists yet (note 6).

---

## 8. Console UI

- A single-line text box at the bottom of the screen with the last ~10 output lines above it, faded after a few seconds once the console closes. Plain DOM over the canvas, no framework (CLAUDE.md). `[manual check]`
- Up and Down cycle through history (kept for the page session; `localStorage` optional, wrapped in try/catch).
- Tab completes command names, then item and tool names from the registry.
- Output colours: echo, result, error. `[manual check]`

---

## 9. Tests

- Parser: every command's usage parses, bad arguments give a useful error, aliases expand, aliases can't shadow commands.
- Execution: `give tool` uses the first empty slot and selects it; `give item` with full hands drops the item at the player's feet as a loose item entity instead of failing, and `give tool` with no empty belt slot does the same; `spawn seeds 5` adds 5 entities; `clear belt` restores the v0 belt. (Decided in #315.)
- Ordering: `give hoe` and a primary press in the same tick resolve with the hoe.
- Pause: no ticks pass while paused; `step 3` advances exactly 3.
- State validity: a command run in a state it doesn't allow errors and changes nothing.
- Client input scripts (`client/test/scripts/*.input`): `` ` `` opens the console and releases held W; typing "f" in the console doesn't toggle fly; Enter returns to the mode it came from; Escape leaves the pointer unlocked; the console key does nothing without debug mode. Extend `client/test/dsl.mjs` with `type <text>` and expectations on `mode = console`, `hands`, `belt.N` and `items.count`.
- Replay with commands (section 7).

> **Reconciler:** the DSL already has `hands`, `belt.slot.<1-5>` and `items`;
> the doc's `belt.N` and `items.count` map onto those names, or the DSL gets
> aliases. Pick one when Phase 2 is planned. "`give item` fails with full
> hands" and "`give tool` uses the first empty slot" need the belt to have an
> empty slot; the v0 belt (hand, sword, hoe) does.

---

## 10. Implementation phases

**1. Command core.** `crates/game/src/commands.rs`: registry, parser, per-tick queue drained at the start of `Walker::tick`, outcome buffer, the `cheated` flag, and `help`, `give`, `spawn`, `clear`. Headless tests only. Done when the execution and ordering tests pass.

**2. Console in the browser.** wasm exports in `crates/game-web` (`submit_command`, `drain_command_output`), the console mode in `client/controller.js` and `client/input.js`, the UI, history and completion, `?debug=1`. Depends on basic-gameplay Phase 5's browser wiring (#224). Done when the input-script tests pass and you can `give` and `spawn` in the test scene.

**3. Pause, teleport, aliases and replay.** `pause`/`resume`/`step`, `tp`, `data/commands.toml` with aliases and cheat codes, commands in recordings, and the replay test.

Each later gameplay phase adds its own commands in its own tickets.

---

## 11. Open questions

1. **Sim while the console is open.** A: keeps running with neutral input (proposed above). B: auto-pauses while open, like a single-player menu.
2. **Pointer lock while typing.** A: unlock on open and re-lock on Enter (proposed). B: stay locked and route keys to the box, so there's no re-lock and no browser cooldown risk, but every key handler must check for console mode.
3. **Shipping.** A: the console is always compiled in and gated by `?debug=1`. B: behind a cargo feature and left out of release wasm to save size. Report the size difference in Phase 1 and decide then.
5. **Cheat flag scope.** A: per session (proposed). B: per save/world once saves exist, like a permanent "cheats used" mark.

## Open questions (reconciler)

6. **Missing section 2 (console mode) and 3 to 5 (command set and syntax),
   except the Phase 1 commands, which section 3 now defines.**
   - A: paste them into this file so the planner can read them.
   - B: let the Phase 1 plan define each command's syntax, with the
     maintainer answering in the plan's `decision` issues.
7. **Where pause lives.**
   - A: in the Rust command layer, so `pause`, `resume` and `step` are
     headless-testable (suggested).
   - B: in `client/controller.js`, which just stops calling the tick.
8. **Replay infrastructure.**
   - A: Phase 3 builds the minimal recorder and world hash in `crates/game`
     and basic-gameplay's replay test reuses them (suggested).
   - B: Phase 3 waits for basic-gameplay's replay test to land.
