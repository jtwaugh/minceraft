---
title: Player death and death screen
status: accepted
scope: game
source: "#807"
superseded_by: ""
phases:
  A: done
  B: done
  C: todo
---

## Current state

- Phase A is done: the dead flag, `PlayerDied` / `PlayerRespawned` events, the `respawn` command and the replay state landed in #841, #875, #881 and #897 (plan #817); `crates/game/tests/death.rs` covers death, respawn and replay.
- Phase B is done: `Walk.dead()` (#936), the death mode in `client/input.js` and `controller.js` with `client/test/scripts/death.input` (#939), `controller.respawn()` (#950) and the death screen panel `client/deathui.js` with a respawn button, mounted in `scene.js` (#951); plan #918. Layout is the Open question 1 default (title and one respawn button). The "like Minecraft's" look stays a `[manual check]`.
- Player health exists: `crates/game/src/player.rs` has `Player.health` (starts at `physics.max_health`, 20) and fall damage saturates it at 0 (`update_fall`, tested by `drop_from(100.0, &w).health == 0`). Nothing reads a 0 health: no dead flag, no event, input still drives the player.
- Health reaches the client: `Walker::health` / `max_health` (`crates/game/src/walk.rs`), `Walk.health()` / `max_health()` (`crates/game-web/src/lib.rs`), and `client/hud.js` draws the hearts from `client/controller.js`.
- Events: `crates/game/src/events.rs` implements the section 11 events of `basic-gameplay.md` (`EntityHurt`, `EntityKilled`, `Swing`, ...). There is no player death or respawn event, and `EntityHurt` / `EntityKilled` are for animals, not the player. The client receives them through `walk.drain_events()`.
- Animal death is the model for a death animation: `crates/game/src/animal_entities.rs` has `death_ticks_left` and a tip-over of about 20 ticks (`basic-gameplay.md` section 8).
- Replays: `crates/game/src/replay.rs` serialises the player's position, velocity, ground flag and health, so a death flag and respawn state must be added there too.
- The client already has a non-walking overlay mode: `state.inventory` in `client/controller.js` (sim ticks on with neutral input), plus `client/inventoryui.js`, `client/consoleui.js` and `client/overlay.js`. Modes and pointer lock are tested by `client/test/scripts/modes.input` and `inventory.input`.
- Spawn: only the raft test scene has a spawn (`crates/game/src/scene.rs`, `spawn`). The real-world scene is chosen in `client/scene.js`; `world-layout.md` covers the raft and start area.
- The phase clock is `crates/game/src/phase.rs` (`PhaseClock::at(tick)`, a pure function of the tick, data in `data/phases.toml`).
- SPEC.md has no section on a death screen, respawn or health beyond fall damage (section 4.6). `grep` finds no heading about death.
- UI art: `ui-art-and-inventory.md` (accepted, phases A to C done) drew the UI graphics and the inventory view; `root-menu.md` is done.

## Reconciliation notes

1. **"Like Minecraft's" needs a decision, not a fact from here.** The doc asks for Beta's death screen text, buttons, score and timing. SPEC.md has none of that, and the clean-room rules forbid filling it in from memory. Either open a `research` question for the spec lane, or decide on this project's own layout (suggest the latter: title, a respawn button, nothing else). See Open question 1.
2. **Dying is a sim state, not only a client mode.** Today `Player.health == 0` changes nothing. For replays to stay deterministic (CLAUDE.md invariant), the dead state, the respawn tick and the position after respawn must live in `crates/game` and in the replay state, and be set by the sim from the tick that health reaches 0. The client only reads it. Suggest a `dead` flag on `Player` and events `PlayerDied { cause }` and `PlayerRespawned { position }` added to `events.rs` and to the section 11 table (a `basic-gameplay.md` edit made by the docs updater or maintainer, not this doc's implementation PR).
3. **Respawn is an input, so it has to be a command.** Input stops driving the player on death, but a respawn click needs to reach the sim in the replay log. Check `crates/game/src/commands.rs` and `input.rs` for where a `Respawn` command fits.
4. **Only fall damage can kill today.** `Player.health` is only lowered in `update_fall`. Drowning, hunger and mob attacks do not exist (`basic-gameplay.md` has no player damage from animals). The cause field on the death event should therefore start with `fall` only.
5. **Respawn point depends on world layout.** The raft scene has a spawn; the main game has none yet. "The raft" depends on `world-layout.md`, "last safe place" needs a new tracked position in the sim. Decision: the player respawns at the scene's start spawn, which Phases A and B already do; the raft and last-safe-place options are dropped. (Decided in #955.)
6. **Item loss touches the belt and hands model** (`basic-gameplay.md` section 6). Dropping items on death reuses `ItemDropped` with a new `source` (`"death"`) and the existing item entities (`crates/game/src/item_entities.rs`). Keeping them needs no code. See Open question 3.
7. **Death animation is optional and reuses the animal tip-over idea.** The camera is first person, so there is no player model to tip. A camera roll and drop over a few ticks would be new camera code (`crates/game/src/camera.rs`). The `death_ticks` value would go in `data/*.toml`, not a magic number.
8. **Phase clock.** `PhaseClock::at` is a pure function of the tick, so it keeps running unless a reset is added; resetting would need a stored offset in the sim and replay. Suggest keep running. See Open question 5.
9. **Art.** The death screen can be drawn with the existing UI styles. New art, if any, goes through `art/PROMPT.md` and the validator as in `ui-art-and-inventory.md`.
10. **Client tests.** Input scripts need new DSL support to force health to 0 (for example a `set health 0` line, or a scene where the player starts above a long drop and walks off). Extend `client/test/dsl.mjs` rather than using screenshots, as CLAUDE.md requires.
11. **`[manual check]`**: "like Minecraft's" is a visual judgment. Measurable version: the death screen shows exactly the elements decided in Open question 1 (listed by id in the DOM model), a respawn control, and nothing else.
12. **Invariants.** Nothing here needs a dependency or std in `crates/terrain`; all work is in `crates/game`, `crates/game-web` and `client/`. Report wasm size changes in PRs.

## Player death and death screen

The player can die, and dying shows a death screen like Minecraft's.

### What we want

- When the player's health reaches 0, the player dies. Today health can already reach 0 (fall damage, `crates/game/src/player.rs`), but nothing happens.
- On death, input stops driving the player and a death screen is shown over the world, like Minecraft's.
- From the death screen the player can respawn and keep playing, back at full health.
- Death and respawn are sim events (alongside the section 11 events in basic-gameplay) and stay deterministic for replays.
- Client behaviour (mode switch to the death screen, pointer unlock, respawn returning to walking) is covered by input scripts in `client/test/scripts/`, not screenshots.

> **Reconciler:** Everything above can be built without answering the open questions by using the smallest defaults noted in the phases below; the questions only change the details.

## Phases (reconciler)

**Phase A.** Sim death and respawn, no UI. Covers the first, fourth and (sim half of the) third bullets of "What we want". Add the dead state to the player, a `PlayerDied` event when health reaches 0, input ignored while dead, a `Respawn` command that restores full health at the scene spawn, and a `PlayerRespawned` event; carry the state in the replay. Items are kept (default for Open question 3) and the phase clock keeps running. Done when `crates/game` unit tests and a headless test (like `crates/game/tests/events.rs`) show a fall to 0 emits `PlayerDied` once, movement input does nothing while dead, respawn restores full health at the spawn, and the same seed and inputs replay identically.

**Phase B.** Client death screen. Covers the second and third bullets (screen, respawn from it) and the last bullet. Expose the dead flag through `game-web`, add a death screen mode to `client/controller.js` (pointer unlock, neutral input, like the inventory view) with a respawn control built from the existing UI styles and the layout decided in Open question 1 (default: title and one respawn button), and return to walking on respawn. Done when new `client/test/scripts/` cases cover: dying switches mode to the death screen and unlocks the pointer, walk input is ignored on it, respawn returns to walk mode with full health, and the DSL gained whatever it needs to force a death.

**Phase C.** Choices that wait on open questions. Covers the answers to questions 2 to 6 once decided: a non-default respawn point, dropping or losing items, a death animation, a phase clock reset, and any new art. Done when each decided behaviour has a unit test or input script. Skip the parts whose answer is the Phase A default.

## Open questions

1. **What does the death screen show and offer?** A: research Beta's screen (needs a `research` spec question, then SPEC.md). B: our own minimal layout, title plus a respawn button (suggested). C: B plus a score or cause of death line.
3. **What happens to the belt and hands items on death?** A: kept (suggested; no code). B: dropped where the player died, as item entities. C: lost.
4. **Is there a death animation, and how long?** A: none, screen appears on the tick of death (suggested). B: a short camera drop and roll, with the length in a data file.
5. **Does death interact with the phase clock?** A: keeps running (suggested). B: resets to the start of gathering.
6. **Does the death screen need new UI art?** A: reuse existing root-menu and inventory styles (suggested). B: new art through `art/PROMPT.md`.
