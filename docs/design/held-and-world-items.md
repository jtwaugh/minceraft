---
title: Held items and items in the world
status: accepted
scope: game
source: "#1137"
superseded_by: ""
phases:
  A: done
  B: done
  C: todo
---

## Current state

Checked against the code:

- Icons exist in `art/` (batches `art/batches_*.json`): `sword`, `hoe`, `seeds`, the animal drops and the flowers. Items are declared in `data/items.toml` (`[[tool]]`, `[[item]]`, `[item_entity]`).
- The sim knows what is held: `Walk::held()` and `Walk::hands()` in `crates/game-web/src/lib.rs`, read by `readBelt` in `client/overlay.js`. `client/hud.js` draws the hands as an icon in the HUD (`#hud-hands`), not in the 3D view.
- There is no swing or use animation state in the sim. `crates/game/src/walk.rs` (around line 883) only pushes a `GameEvent::Swing` when primary is pressed with empty hands; secondary emits no use event for animation. Nothing counts animation ticks.
- Item entities: `crates/game/src/item_entities.rs` (`ItemStore`, gravity, settling, pickup), reached through `Walk::items()`. `crates/game-web` exposes only `spawn_item` and `loose_items` (a count). There is no accessor that returns item positions or names, unlike `Walk::animals()`, which returns a flat `Vec<f64>` plus `animal_species()`.
- `box_size` is `[item_entity] box_size = 0.25` in `data/items.toml`, one cube edge for all items.
- `client/scene.js` calls `spawnTestItems(walk)` (from `client/overlay.js`) in the test scene only, and draws animals with `drawAnimals` through the model drawer (`client/modelgl.js`, `client/models.js`). Nothing draws item entities or a held item.
- The model drawer is unshaded and unfogged, and its atlas packs model faces plus any texture named in `textureNames` (`packAtlas`), so item icons can be packed into it. Its fragment shader discards texels with alpha under 0.5.
- View bobbing is a documented default in `docs/design/basic-gameplay.md` section 3, but no bob is implemented in `client/` (`basic-gameplay.md` Phase 1 note says bobbing and hand rendering were left for later).

Phase A (item entities in the world) is done: `Walk::items()` and `item_names()` in `crates/game-web/src/lib.rs` (#1164), `readItems` in `client/overlay.js` (#1165), item icons packed into the model atlas (#1168), `itemPose` in `client/itemmodel.js` (#1172), and `drawItems` in every scene plus `client/test/scripts/items.input` (#1182). Phase B (held item) is done: held string to icon name (#1192), held mesh and resting pose (#1195), the held item drawn in its own pass (#1196), and the `held.drawn` expectation with `client/test/scripts/helddraw.input` (#1197). Phase C is not started.

## Reconciliation notes

1. **Item entity data has no path to the client.** Section 2 needs per-item position, name and (for spin) an age or id. `game-web` only exposes a count. Suggest an accessor shaped like `animals()`: a flat `Vec<f64>` (`id, x, y, z, box_size`) plus a parallel `Vec<String>` of item names.
2. **No animation state in the sim.** Section 1 says swings are "timed from sim ticks so input scripts can check them". The sim has only the `Swing` event and no tick counter. Suggest a small per-player counter in `crates/game` (ticks since primary or secondary succeeded, durations in a data file, for example `data/hands.toml`), exposed through `game-web` and the input-script DSL (`client/test/dsl.mjs`) as an expectation. The doc says a successful action triggers the swing or use animation (`basic-gameplay.md` section 6, "Every action that succeeds triggers the hand swing..."); a `Swing` event is currently emitted on any primary press with empty hands, success or not. Decide whether the animation follows that event or the action's success.
3. **Beta swing facts are not in SPEC.md.** The doc rightly defers to SPEC.md, but `SPEC.md` has no section on the hand swing, held-item pose or bobbing. The shape of the swing therefore cannot be taken from Beta here. Raised as an open question (1): either open a `spec-question` labelled `research` or make it this project's own choice, tuned in data. Do not fill it in from memory.
4. **View bobbing is a separate dependency.** "It bobs with the camera" needs a camera bob that doesn't exist (`basic-gameplay.md` section 3 lists it as a default). Either hand bob is phased after camera bob, or the held item gets its own bob from distance walked. Noted in the phases.
5. **Determinism and tests.** Animation uses sim ticks, so it is testable in input scripts. Pixel placement, "looks right" and "stands out from the terrain" are visual: tagged `[manual check]` below, with measurable versions proposed.
6. **Invariants.** All work is in `crates/game`, `crates/game-web` and `client/`, so `crates/terrain` stays untouched and `no_std`. Tunables (swing length, spin speed, hand offset) go in `data/*.toml`, not as magic numbers. The client uses plain ES modules. The item atlas adds a few 8x8 textures, so the wasm and page size effect is small; report wasm size in PRs that touch `game-web`.
7. **Overlap with other docs.** `docs/design/ui-art-and-inventory.md` owns HUD and inventory art; the HUD hands icon stays (the doc says hands win over the belt, which matches `held`). `docs/design/picking-plants.md` and `docs/design/debug-console.md` add items and `spawn`, which section 2 should draw with no change. `docs/design/player-death.md` Phase C wants tools as item entities. Item entities currently hold only `ItemInstance`, so drawing a dropped tool needs that doc's work first; until then only `item:<name>` entities are drawn.
8. **Clip-into-blocks.** "Doesn't clip into blocks in front of the player" is a rendering-order matter, not a sim one. A held item drawn in a separate pass with its own depth range (open question 4) avoids it without any sim code.

## Mince Raft: Held items and items in the world

Status: idea
Scope: game

Tools and items have icons (`sword`, `hoe`, `seeds`, the animal drops and flowers are all in `art/`), but they only appear in the HUD belt and the inventory view. In the 3D scene, holding the sword shows nothing, and anything thrown, dropped or dropped by an animal can't be seen. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. First-person held item

`basic-gameplay.md` section 3 says "A first-person hand renders the current tool or carried item, with Beta's swing animation on primary and a smaller use animation on secondary." Phase 1 left hand rendering for later and no phase has picked it up since.

- Draw what `held` is (`tool:<name>`, `item:<name>` or empty) in the lower right of the view, from the item's existing icon. Hands win over the belt, as they already do for `held`. The held item is a flat sprite: the 8x8 icon on a thin two-sided slab, like world items (`itemBoxes` in `client/itemmodel.js`), one box per item, not an extruded voxel slab. (Decided in #1186.)
- A swing on primary and a smaller use motion on secondary, timed from sim ticks so input scripts can check them (CLAUDE.md "Testing the client"). How Beta's swing behaves comes from SPEC.md, not from this doc.
- It doesn't clip into blocks in front of the player. Its bob with the camera waits until a camera bob doc or ticket exists; Phase C plans no bob. (Decided in #1204.)
- An empty hand: either nothing, or a simple hand/arm drawn in `art/`.

> **Reconciler:** Needs the sim animation counter (note 2) and has no Beta swing facts in SPEC.md (note 3). `[manual check]` "lower right" and "looks like a swing": a measurable version is an input-script expectation on the animation phase per tick (for example `swing = 0` before, rising over N ticks, back to 0) plus a test that the held item's model matrix at tick k matches the data curve.

## 2. Item entities in the world

`crates/game/src/item_entities.rs` has loose items with gravity, settling and pickup, and `client/scene.js` spawns test items, but nothing draws them.

- Draw each item entity at its position, sized by `item_entity.box_size` (`data/items.toml`), using the item's icon.
- Each item entity is drawn as a flat billboarded sprite of its 8x8 icon that always faces the camera. Its idle motion is a bob only (nothing to spin), so items stand out from the terrain. (Decided in #1145.)
- They show up in streamed regions and in the raft scene alike.

> **Reconciler:** Needs the item accessor in `game-web` (note 1). `scene.js` spawns test items only in `scene` mode; the streamed and raft scenes must call the same draw path. `[manual check]` "stand out from the terrain": the testable part is that every entity in `Walk::items()` yields exactly one draw call at its position, and that spin or bob is a pure function of time and id, so it needs no sim state.

## Open questions

3. Should an empty hand show a hand or arm, or nothing at all?

Added by the reconciler:

5. Where do the swing and use durations and curves come from, given SPEC.md has no hand swing section?
   - A: this project's own values in `data/hands.toml`, tuned by feel (no SPEC change).
   - B: open a `spec-question` labelled `research` and wait for facts about the original.
6. Should the animation follow the `Swing` event (any primary press with empty hands) or only a successful action?
   - A: follow the action's success, as `basic-gameplay.md` section 6 says.
   - B: follow the button press, so swinging at air animates.
8. Should a dropped tool be drawn?
   - A: wait for `player-death.md` Phase C, where tools become item entities.
   - B: draw only `item:<name>` entities now and skip tools.

## Phases (reconciler)

**Phase A.** Item entities in the world (section 2). Add the `game-web` accessor for item entities (note 1), pack the item icons into the model atlas, and draw each entity at its position at `box_size`, with a spin or bob that depends only on time and id. Cover streamed regions and the raft scene as well as the test scene. Depends on open question 2 for the mesh; the flat billboard is the cheapest default if the maintainer doesn't answer. Item entities, and later the held item, are drawn in the existing model pass (`client/modelgl.js`, the atlas from `client/models.js`), with the item icons packed into the atlas via `textureNames`; there is no separate item pass. (Decided in #1146.) Done when a headless test checks one draw per entity from `Walk::items()` and an input script that throws an item shows `items.count` rising.

**Phase B.** Held item (section 1, held tool or item, no empty hand). Draw the held item in the lower right from its icon, using the answer to question 1, with no swing yet. Done when an input script shows the held thing follows `held` (tool selected, carried item, empty) and a test shows it is drawn nowhere else when `held` is empty.

**Phase C.** Swing and use (section 1). Add the sim animation counter and `data/hands.toml` (notes 2 and 3), the primary swing and the smaller secondary use motion, plus the empty-hand art if question 3 says to draw one. No bob: it waits for a camera bob doc or ticket. Waits on open questions 5, 6 and 8. (Decided in #1204.) Done when input scripts check the animation phase tick by tick through a swing and a use, and the held item's matrix follows the data curve.
