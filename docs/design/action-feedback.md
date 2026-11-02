---
title: Say what an action did, or why it did nothing
status: accepted
scope: both
source: "#1999"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
---

## Current state

- **Sim events exist.** `crates/game/src/events.rs` has `GameEvent` with `ItemPickedUp`, `BlockTilled`, `CropPlanted`, `CropHarvested`, `Foraged`, `ForageMissed`, `Swing` and the rest of basic-gameplay section 11. `Walker::drain_events` (`crates/game/src/walk.rs:1018`) returns them; `crates/game-web/src/lib.rs:416` turns them into strings; `client/controller.js` passes them out as `events` each frame.
- **Events are not in the world hash.** `Walker::world_hash` (`crates/game/src/replay.rs` header) covers tick, player, belt, hands, items, animals and raft, not the event list. Events are derived from the same inputs, so a replay regenerates them.
- **Only `forage_missed` is a refusal.** No other no-op (out of reach, hands full while carrying, wrong tool, nothing to till) emits anything. There is no refusal reason in `GameEvent`.
- **Observers see counts only.** In `client/tools/observe.mjs` `events` and `events.foraged` are `debug` paths; `client/test/dsl.mjs` serves `events` as a count and `events.foraged`.
- **No message area in the HUD.** `client/hud.js` (`hudModel`, `mountHud`) draws crosshair, timer, hands, hearts and belt; `client/hudlayout.js` places them. Pixel text exists: `drawString` in `client/font.js`, used for the hands label and by `client/hintui.js`. `observe.mjs` lists `hud.selected` and `hud.hands` as player paths; there is no `hud.message`.
- **Selection outline** is drawn in `client/scene.js` (around line 203) from the controller's `aim`. Nothing keeps it after the target changes or disappears.
- **Data-file convention.** Game tuning lives in `data/*.toml` (`hands.toml`, `interactions.toml`, `input.toml`); no file holds display strings yet.

## Reconciliation notes

- **Refusals touch the action code, not only the HUD.** Today the dispatch in `walk.rs` (around lines 799-820) detects "acted" by comparing event counts. Refusal events need each action function (`till.rs`, `plant.rs`, `harvest.rs`, `forage.rs`, `hands.rs`, `interactions.rs`) to say why it did nothing. Suggest a single `ActionRefused { action, reason }` event with a `Reason` enum (`TooFar`, `HandsFull`, `ToolsStowed`, `WrongTool`, `NothingHere`) rather than one variant per case. `ForageMissed` stays as it is.
- **Open question 2 (hash and replays).** The code answers most of it: events are not hashed, so adding refusal events cannot change `world_hash` or break existing recordings. Only the event list the sim returns changes. Suggest: refusal events are ordinary `GameEvent`s (sim-emitted, deterministic, visible to input scripts) and stay out of the hash like every other event. Client-only would put gameplay rules in JS and could not be tested in Rust.
- **`hands_full` vs `tools_stowed`.** Basic-gameplay section 6 makes primary do nothing while carrying (`tools_stowed`) and secondary with a non-usable item do nothing (`hands_full` when picking up). The idea's list matches these, but the sim must decide which reason a given press gets; spec that table in Phase A. The table: a secondary press on a loose tool when the belt is full gets `hands_full`; a secondary press while carrying an item that has no use on the target (seeds on stone, a dish on grass) gets `nothing_here`; and `wrong_tool` is dropped, so the `Reason` enum here and the list in section 2 have four values (`TooFar`, `HandsFull`, `ToolsStowed`, `NothingHere`) and a press whose held item matches no rule is `nothing_here`. (Decided in #2028.)
- **Query for "too far".** Reach is in `data/targeting.toml` and `crates/game/src/targeting.rs`. A press on a target beyond reach has no target, so `too_far` needs the sim to look for the nearest target past reach. Smallest approach: re-run the pick with a longer range only when the normal pick finds nothing and the press is a deliberate action; report `nothing_here` if that finds nothing too.
- **Quoted help text.** "Hands full: drop with Q" hard-codes a key. Q comes from `data/input.toml` (`KeyQ`); the string template should take the binding name rather than repeat it.
- **Strings in a data file.** Fits the repo's convention. Suggest `data/messages.toml` keyed by event name (and reason), read by the client. The `crates/terrain` and wasm-size invariants are not affected; `crates/game` may use `std`. Because the client is plain ES modules with no bundler, the file must reach the client the way other `data/*.toml` values do (via `crates/game-web`, or a build step in `build.sh`); the Phase B ticket should check which.
- **Selection outline hold.** The outline is client-side presentation, so it belongs in `client/controller.js` (testable without WebGL, per CLAUDE.md) with a frame count in data. The "outline vanishes" case in #1574 happens because the target is gone; the held outline must be drawn at the old box, not the new aim.
- **Observation path.** `hud.message` is a player-visible path, so it goes in the `player` class in `observe.mjs`, not `debug`; it shows the current text or empty. The critic still must not see `events` (play-critic-agent note 5), so the message is the only route.
- **[manual check]** "fades after about two seconds" and "a few frames" are testable as tick/frame counts in data; the look of the line above the hearts is a visual judgment for the maintainer.
- **Overlap.** `docs/design/ui-overhaul.md` set the pixel font, panel and scale rules this line must follow (`hudScale`, `drawString`); `docs/design/play-critic-agent.md` defines the player/debug split; `docs/design/held-and-world-items.md` defines the pickup rules the messages report. No conflict found.

## Mince Raft: Say what an action did, or why it did nothing

Status: idea
Scope: game (events) and client (HUD)

The play critic twice acted on something and could not tell what happened. Right-clicking a loose item made the outline vanish with nothing in the hands or belt (#1574). Right-clicking ground with the hoe changed nothing the HUD shows (#1587). In both cases the player can't tell success from a miss, a wrong tool or being out of reach. This idea adds a short on-screen line for action results and for refusals. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- basic-gameplay section 11 and `crates/game/src/events.rs`: the sim already emits events for every action (Phase 8, plan #469), and they reach the client and input scripts. In `client/tools/observe.mjs` they are `debug` paths (`events`, `events.foraged`, `field.ground`, `planter.ground`), so a player and the critic never see them.
- basic-gameplay section 6: secondary on a loose item picks it up into empty hands, a loose tool goes to the belt whatever the hands hold (#1645); while carrying, tools are stowed and primary does nothing (#215).
- No-op actions (out of reach, hands full, wrong tool, nothing to till) emit nothing.
- `client/hud.js` has no message or toast area.

## 2. Want

- A one-line HUD message above the hearts, in the pixel font, that fades after about two seconds. It shows the latest action result: "Picked up seeds", "Tilled soil", "Planted wheat", "Seeds to the belt".
- The sim emits a refusal event when a deliberate action does nothing, with a reason: `too_far`, `hands_full`, `tools_stowed`, `wrong_tool`, `nothing_here`. The HUD shows these too ("Hands full: drop with Q").
- Keep the selection outline on the block or item for a few frames after an action, so the player sees what they acted on.
- Event-to-text strings live in a data file, not in code.
- The player observation gains `hud.message`, so the critic can see the line.

## 3. Open questions

1. Which actions are worth a message? Every swing would be noise; pickups, tilling, planting, harvesting and refusals seem enough.
2. Do refusal events go in the world hash and replays like other events, or are they client-only?

## Source findings

#1574 item 1, #1587 item 2.

## Phases (reconciler)

**Phase A.** Refusal events in the sim (section 2, second bullet). Add `ActionRefused { action, reason }` to `GameEvent`, emit it from the till, plant, harvest, forage, pickup and carry paths when a deliberate secondary press, or a primary press while carrying, does nothing, and expose it through `crates/game-web`. Done when Rust tests cover each reason, a test shows `world_hash` is unchanged by refusal events and an overlapping-region/replay run still agrees, and an input script reads the event count.

**Phase B.** The message line (section 2, first, fourth and fifth bullets). Add `data/messages.toml` mapping events and reasons to text, a message model in `client/hud.js` (latest message, age, fade, in ticks from data) with a layout rect above the hearts in `client/hudlayout.js`, and `hud.message` as a player path in `client/tools/observe.mjs`. Done when Node tests check the text for pickup, tilled, planted, harvested and each refusal, the fade after the configured time, and the observation path; the look is a `[manual check]`.

**Phase C.** Outline hold (section 2, third bullet). Keep the last acted-on box in `client/controller.js` for a number of frames from data and draw it in `client/scene.js`. Done when an input script right-clicks a loose item and reads that the outline box is still set for the configured frames and then clears.

## Open questions

1. **Refusal events and the hash (author's question 2).** A: ordinary sim events, outside the hash like all events (suggested, see notes). B: client-only, derived in JS from the pick result. Which?
2. **Which actions get a message (author's question 1).** A: pickups, tilling, planting, harvesting, foraging and all refusals (suggested). B: A plus attack hits and kills. Misses of `swing` stay silent either way.
3. **Where the strings reach the client.** A: via `crates/game-web`, as with other game data. B: a build step copying `data/messages.toml` for the client. Either works; the Phase B ticket should pick the one `build.sh` already supports.
4. **Several messages in one frame.** A: show the latest only (the idea's wording). B: show the latest and fold repeats ("Picked up seeds x2"). Suggest A.
