---
title: Show when the game is waiting for a click
status: accepted
scope: game
source: "#1998"
superseded_by: ""
phases:
  A: done
  B: todo
---

## Current state

Checked against the code:

- **Input ignored while unlocked.** `client/input.js:52` requests pointer lock on a canvas click when not locked, not in the inventory and not dead. Keydown, keyup, mouse buttons and wheel return early unless `locked` (`input.js:95-117`). Matches section 1.
- **Relock.** `closeInventory({ relock })` (`input.js:64`) re-requests lock; the inventory key relocks, Escape does not (`input.js:77`). A submitted console relocks (`input.js:131`). Matches section 1.
- **Hint.** `client/hintui.js` has `HINT_TEXT` starting "Click to look around", `mountHint(...)` returning `{ update(locked), toggle(), open }`, a layout built from `drawPanel` and `drawString`, and a `?` corner button (`cornerbutton.js`). `update` folds the hint the first time the pointer locks and never reopens it except from the button. `client/scene.js:479` mounts it and `scene.js:621` calls `hint.update(input.locked)`. Matches section 1.
- **No waiting cue exists.** Nothing in `client/` draws a prompt when the pointer is unlocked after the first lock. `deathui.js` (`mountDeath`, `update(dead)`) is the nearest pattern for a full-screen overlay driven by controller state.
- **DSL gap is real.** In `client/test/dsl.mjs:147` the fake canvas's `requestPointerLock()` only counts (`lockRequests`); the fake document has `exitPointerLock` that clears `pointerLockElement` and fires `pointerlockchange`, but nothing sets it. `lock`, `unlock` and `blur` statements exist (`dsl.mjs:353-356`). The `blur` statement only fires the window event and does not unlock.
- **Observation.** `client/tools/observe.mjs` classifies `mode` and `locked` as `player` and `lock_requests` as `debug`. There is no `prompt` or `hint` path, and `dsl.mjs` serves neither (`mode` is `death | console | inventory | fly | walk`, `dsl.mjs:217`).
- **Nothing is built yet:** the cue, the DSL relock, and the `prompt` and `hint` observation paths.
- **Phase A done (plan #2014).** The DSL fake canvas locks on request and `blur` unlocks (#2048, `client/test/dsl.mjs`); pure `waitingForClick` and the hint state exist (#2042, `client/waitingcue.js`); `prompt` and `hint` are served by the DSL and classified in `client/tools/observe.mjs` (#2083). Still not built: the cue itself (Phase B).

## Reconciliation notes

1. **Existing scripts may change.** Making the fake canvas lock on request (section 2) changes the `locked` value after any `click`, Tab-out or console submit in scripts under `client/test/scripts/` that currently rely on it staying false (`inventory.input`, `console.input`, `death.input`, `modes.input`, `transitions.input`). Phase A must run all of them and fix the expectations that encoded the harness gap, not the game. A real browser also grants the lock asynchronously; firing `pointerlockchange` synchronously inside `requestPointerLock` is the simplest fake, and is enough for the scripts.
2. **`blur` and locking.** A real browser drops pointer lock when the window loses focus, which fires `pointerlockchange`, so `input.js` already handles it through the `locked` path. The DSL `blur` does not unlock, so the "window blur" case in section 2 would not show the cue in a script unless `blur` also unlocks (as the real browser does). Suggest `blur` clears `pointerLockElement` and fires `pointerlockchange` before the window event.
3. **Cue state needs a source.** `scene.js` today passes only `input.locked` to the hint. The cue also needs "mode is walk or fly, console and inventory closed, not dead", all of which the controller and `input` already expose (`input.console`, `input.inventory`, `input.dead`, `state.flying`). Keep the condition as a pure function (for example `waitingForClick(...)` in the new UI module) so Node tests and `dsl.mjs` use the same one for the observation `prompt`; do not recompute it in `scene.js` only.
4. **Observation `hint`.** `hint` (open or folded) needs `mountHint`'s `open` flag, which the DSL does not mount (no DOM). Suggest moving the open/fold rule (fold on first lock, reopen on toggle) into a small pure state object in `hintui.js` that both `mountHint` and the DSL drive, so the DSL does not reimplement it.
5. **Section 2 is the cue; `hint` can stay.** The author keeps the first-run hint as is, so the two panels can show together at game start (hint top-right, cue centred). That is fine; no change to `hintui.js` layout is needed beyond note 4.
6. **Style.** Reuse `drawPanel`, `drawString`, `interfaceScale` and `font_unknown` cell size like `hintui.js`; text must be only characters the pixel font maps (A to Z, 0 to 9 and `. , : ! ? ' - / ( )`, see `ui-overhaul.md`), so "Click to play" is fine. No new art is needed unless the dim layer wants a texture (a CSS `rgba(0,0,0,.4)` box matches the death screen).
7. **`[manual check]`:** "small", "centred" and "dim the view so it reads as paused" are visual. Measurable versions: the panel fits in the viewport at the four `hudsizes` viewport sizes (a `hintLayout`-style pure test), and the cue's centre is within one texel of the viewport centre.
8. **Overlap.** `player-death.md` owns the death overlay and `play-critic-agent.md` owns the observation classes (`observe.mjs`); this doc adds two `player` paths there and does not conflict. `ui-overhaul.md` Phase D built the hint panel this reuses.
9. **Invariants.** Client-only; no `crates/terrain` change, no world output change, no wasm size change expected, no framework in `viewer/`. Per CLAUDE.md, the cue's behaviour is tested with input scripts, not screenshots.

---

# Mince Raft: Show when the game is waiting for a click

Status: idea
Scope: client (and the critic's player observation)

Five play critic runs (#1520, #1574, #1587, #1655, #1672) hit the same wall: in walk mode with the pointer unlocked, every key does nothing and nothing on screen says why. It happens at game start and again after leaving the inventory. A new player sees a dead game. This idea makes the waiting state visible every time it happens, not just once, and lets the critic see that cue. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `client/input.js`: a canvas click requests pointer lock when not locked, not in the inventory and not dead. While unlocked, keydown, keyup, mouse buttons and wheel are all ignored in walk mode.
- `closeInventory({ relock })` in `client/input.js`: closing with the inventory key requests lock again; closing with Escape does not (the pointer is already free, so Escape reaches the page).
- `client/hintui.js`: a controls hint panel whose text starts "Click to look around". It shows at game start, folds away the first time the mouse locks, and reopens only from the `?` corner button. After that first lock, nothing tells the player a click is needed.
- `client/test/dsl.mjs`: the fake canvas counts `lockRequests` but never becomes locked, so a scripted Tab, Tab shows `locked = false` even though `input.js` asked for lock. Several findings ("Tab again leaves the pointer unlocked") are this harness gap, not a game bug. Escape from the inventory leaving the pointer free is real.
- `client/tools/observe.mjs`: the player observation has `mode` and `locked` but nothing for the hint panel or any prompt, so the critic can't tell whether a cue was on screen.

## 2. Want

- A small "Click to play" cue (pixel font, nine-slice panel like the hint) centred on screen whenever the mode is walk or fly, the pointer is unlocked, the console and inventory are closed and the player is alive. It covers game start, Escape out of the inventory, Escape from walk mode and a window blur. It disappears as soon as the pointer locks.
- Optionally dim the view behind it, so the frozen state reads as paused.
- The first-run controls hint can stay as it is; the cue is the part that comes back every time.
- The DSL's fake canvas becomes locked on `requestPointerLock` (firing `pointerlockchange`), so scripts and the critic see the real relock after Tab. A test pins Tab, Tab ending locked and Tab, Escape ending unlocked with the cue showing.
- The player observation gains `prompt` (the cue's text, or null) and `hint` (open or folded), classified `player` in `observe.mjs`.

## 3. Open questions

1. Should number keys select belt slots while unlocked (#1672 suggests it), or should all input stay paused behind the cue?
2. Should Escape out of the inventory also try to relock? Browsers usually refuse a lock request right after Escape, which may be why it was left out.

## Source findings

#1520 item 2, #1574 item 2, #1587 item 1, #1655 item 1, #1672 items 1 and 2.

## Phases (reconciler)

**Phase A.** The harness and the observation, no visible change: the DSL's fake canvas locks on `requestPointerLock` and fires `pointerlockchange`; `blur` unlocks as a browser does (note 2); existing scripts are corrected where they encoded the gap (note 1); a pure `waitingForClick` condition and the hint's open/fold state are factored out (notes 3 and 4); `prompt` and `hint` are served by `dsl.mjs` and classified `player` in `observe.mjs`, and `playerObservation` returns them. Done when a script pins Tab, Tab ending `locked = true` and Tab, Escape ending `locked = false` with `prompt` non-null, and `client/test/*.test.mjs` passes.

**Phase B.** The cue itself: a new UI module (pattern of `hintui.js` and `deathui.js`) draws the centred "Click to play" panel from `waitingForClick`, mounted in `scene.js` and updated each frame, with the optional dim layer. Done when a pure layout test shows the panel fits and is centred at the `hudsizes` viewport sizes, the cue's visibility follows `waitingForClick` in a script for game start, Escape from the inventory, Escape from walk and blur, and it hides on lock. The look is `[manual check]` (note 7). Open question 1 decides whether belt keys work behind the cue; leave input as it is until it is answered.

## Open questions

1. **Belt keys while unlocked.** Options: A) all input stays paused behind the cue (current behaviour, no change to `input.js`); B) number keys select belt slots while unlocked, which is a follow-up ticket in `input.js` and needs the cue to say so.
2. **Relock after Escape from the inventory.** Options: A) leave it as is, the cue tells the player to click; B) also request lock after Escape and rely on the cue when the browser refuses. Browsers usually refuse, so A is the smaller change.
3. **Dim the view?** Options: A) panel only; B) panel plus a dim layer (about 40% black) so the frozen state reads as paused; the dim could hide the thing a critic wants to look at, so B would need the observation to say it is on.
