---
title: Show the ready tool and stowed tools
status: accepted
scope: game
source: "#2000"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
---

## Current state

- `Walk::hands()` (carried item name, or empty) and `Walk::held()` (`empty`, `tool:<name>` or `item:<name>`: the carried item, else the selected tool) are exported in `crates/game-web/src/lib.rs` (`hands`, `held`, `selected_slot`). The rule is in `crates/game/src/items.rs` (held derivation, line ~343) and `crates/game/src/walk.rs` (`held()`, primary a no-op while carrying).
- `readBelt` in `client/overlay.js` returns `{ slots, selected, hands, held, swing, use }`. `hands` is the item name or null; `held` is a string and is not passed into the HUD model.
- `client/hud.js` builds the model `{ slots, selected, hands, ... }` and draws the hands frame (`rects.handsSlot`) as a plain `slot` frame plus the carried item's icon; the belt slots use `slotView` (`slot` or `slot_selected`). `client/hudlayout.js` computes `handsSlot.icon` only from `model.hands`.
- `client/tools/observe.mjs` serves the player `hud.selected` and `hud.hands`; `held` is debug-classified. There is no `hud.stowed` or `hud.ready`.
- Belt keys move `belt.selected` while carrying (`client/test/scripts/belt.input`); tools stay stowed. The rule is decided in #215 (`docs/design/basic-gameplay.md`, section 6; Phase 5 done).
- Nothing yet satisfies Section 2 of the idea: no dimmed belt, no tool icon in the empty hands frame, no `hud.stowed` / `hud.ready`.

## Reconciliation notes

- **Hands frame doubles as two things.** The doc has the frame show either a carried item or the ready tool. `hands` (item name) also drives the name label above the strip (`rects.hands`). Keep the label for carried items only, so the tool preview needs a separate model field (for example `ready`) rather than reusing `hands`.
- **Dimmed/greyed/faded art.** Dimmed slots, a grey selection frame and a faded or outlined tool icon need either new textures in `art/` (own palette, `snake_case` names, per `art/PROMPT.md`) or a canvas filter in `hud.js`. The smallest way is to draw the existing textures at reduced alpha in `hud.js`, which needs no art. If textures are wanted instead, add them to the art batches and `validate.py`.
- **Pulse.** The hands-frame pulse on a belt key while carrying needs a transient timer in client state. Keep it in `client/controller.js` (testable, no WebGL), not in `scene.js`.
- **"Hands full: drop with Q" depends on the action-feedback idea.** No design doc or accepted idea for action feedback exists in `docs/design/` (checked by search). Until it does, that line is not buildable; Phase C below waits on it.
- **Observation shape.** `hud.stowed` and `hud.ready` are player-classified paths, so add them to `CLASSES.player` and `playerObservation` in `client/tools/observe.mjs`. `ready` should be the tool name when hands are empty and a tool is selected, else null (when carrying, the frame shows the item, which `hud.hands` already reports).
- **game-web export.** `held()` already gives `tool:<name>`, so `ready` can be derived in JS from `held` and `hands` with no Rust change; `stowed` is `hands !== null`. A new export is only needed if an empty belt slot must be told apart from "no tool", which `held() == "empty"` already does.
- **Invariants.** No terrain or world output change; `crates/terrain` untouched; no viewer frameworks. Wasm size unchanged if no export is added.
- **[manual check]** "Dimmed" and "greyed" are visual judgments. Measurable version: input scripts assert `hud.stowed` and `hud.ready` values, and a layout/model test asserts the dim flag per slot; the look itself stays a maintainer check (screenshots only when asked, per CLAUDE.md).
- **Open question 1 of the idea** is already answered by the idea itself (leave #215 alone); recorded below.

## Idea

# Mince Raft: Show the ready tool and stowed tools

Status: idea
Scope: client (HUD), small game-web export if needed

The hands and belt follow the Overcooked rule (basic-gameplay section 6, #215): carrying an item stows the tools. The rule works but is invisible. With seeds in hand, pressing a belt key moves the selection and the player believes they swapped to the hoe (#1520). With empty hands, selecting the hoe leaves the hands frame empty, so nothing says the hoe is ready (#1655). This idea makes both states visible. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `Walk::held()` and `Walk::hands()` in `crates/game-web/src/lib.rs`, read by `readBelt` in `client/overlay.js`. `held` is the derived thing in use (a carried item, else the selected tool); `hands` is only the carried item.
- `client/hud.js` and `client/hudlayout.js`: the hands frame (`rects.handsSlot`) shows the carried item's icon or nothing; the belt shows the selected slot.
- Belt keys change `belt.selected` while carrying, though tools stay stowed.
- In `client/tools/observe.mjs`, `held` is a debug path; the player sees `hud.hands` and `hud.selected`.

## 2. Want

- While carrying an item, the belt draws as stowed: slots dimmed, the selection frame greyed. A belt key still moves the selection (so the choice is ready once the hands empty) but also pulses the hands frame, and with the action-feedback idea shows "Hands full: drop with Q".
- With empty hands, the hands frame shows the selected tool's icon, faded or outlined to tell it apart from a carried item, so the frame always shows what a click will use.
- The player observation gains `hud.stowed` (true while tools are stowed) and `hud.ready` (the icon shown in the hands frame).

## 3. Open questions

1. Should a belt key while carrying instead put the item down or drop it and switch tools? That would change decision #215, so this idea leaves the rule alone and only shows it.

## Source findings

#1520 item 1, #1655 item 2.

## Phases (reconciler)

**Phase A.** Ready tool in the hands frame (Section 2, second bullet; `hud.ready`). Add a `ready` field to the HUD model from `held`/`hands` (tool name when hands are empty and a tool is selected), draw its icon faded in `rects.handsSlot` without a name label, and add `hud.ready` to the player observation. Done when an input script with empty hands and the hoe selected expects `hud.ready` to be the hoe and a layout test shows the icon rect, and picking up an item makes `hud.ready` null with `hud.hands` the item.

**Phase B.** Stowed belt (Section 2, first bullet apart from the message; `hud.stowed`). While carrying, draw belt slots dimmed and the selection frame greyed; belt keys still move the selection; add `hud.stowed` to the player observation; pulse the hands frame on a belt key while carrying, with the timer in `client/controller.js`. Done when input scripts expect `hud.stowed` true while carrying and false after a drop, that a belt key while carrying changes `hud.selected` and triggers the pulse state, and the model test checks the dim flag. The look is a `[manual check]`.

**Phase C.** "Hands full: drop with Q" message. Waits on the action-feedback idea (not yet in `docs/design/`); see Open questions. Done when a belt key while carrying shows the message through that mechanism and an input script observes it.

## Open questions

1. **Action-feedback mechanism.** The idea refers to an "action-feedback idea" that has no design doc. Options: (A) wait for it, plan Phase C afterwards; (B) drop the message from this doc and ship Phases A and B only; (C) build a minimal one-line message in the HUD here.
2. **Dim/fade rendering.** Options: (A) reduced alpha in `hud.js` with existing textures; (B) new dimmed and outlined textures in `art/`.
3. **Belt key while carrying** (the idea's own question): the idea keeps #215 and only shows the state. Left as the author decided; no change needed.
