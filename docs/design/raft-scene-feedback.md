---
title: Raft scene feedback
status: done
scope: game
source: "#446"
superseded_by: ""
phases:
  A: done
  B: done
  C: done
  D: done
---

## Current state

Checked against the code on `master`:

- **Swell push (item 1).** `data/raft.toml` `[swell_push]` has `per_degree = 0.004` and `item_multiplier = 2.0`. `SwellPush` in `crates/game/src/raft.rs` turns deck tilt into a push; `crates/game/src/item_entities.rs` and `crates/game/src/animal_entities.rs` apply it to items and animals resting on the deck. `crates/game/tests/raft_stability.rs` has `swell_without_sneak_drifts` and `swell_with_sneak_does_not_drift`, and `item_entities.rs` has `deck_push_is_scaled_by_item_multiplier`.
- **Animals in water (item 2).** `crates/game/src/animal_entities.rs` applies gravity, a swept move, drag and the deck push, and wandering. It has no water handling. `CollisionWorld::body_in_water` (`crates/game/src/collision.rs`) and the player's water movement (`crates/game/src/physics.rs`, basic-gameplay 4.3) already exist. 4.3 says "No drowning in v0", so drowning is new for animals and would be a new rule.
- **Animal art (item 3).** `ANIMAL_COLORS` and `drawAnimals` in `client/scene.js` draw coloured boxes, red while hurt, tipped over while dying. `art/PROMPT.md` lists no cow, pig or chicken texture. It does mention villagers and squid as 3D box models, which sets the style precedent.
- **Raft driving (item 4).** `RaftFrame.yaw_delta` and `Raft::advance` (`crates/game/src/raft.rs`) apply a frame's displacement, yaw and tilt. Only the scripted swell produces frames (`walk.rs`, `Swell::frame`). Nothing reads player input to produce one. Quarter-turn collision is in place (basic-gameplay 4.7, #140). Multi-region streaming with the raft and rider crossing region borders is built (`crates/game/src/stream.rs`, `crates/game/tests/raft_regions.rs`).
- **Raft model (item 5).** `scene.raft_blocks()` (`crates/game-web/src/lib.rs`) is meshed as ordinary blocks in `client/scene.js`. The art batches already have `raft_planks`, `raft_edge` and `raft_bottom` block faces (`art/PROMPT.md` batches 2 and 7; `data/blocks.toml`). There is no raft model, log or lashing art beyond those.
- **Water outline (item 6).** `selection_box` in `crates/game/src/collision.rs` gives every non-air block, liquids included, a selection box. `crates/game/src/targeting.rs` has the test `casts_hit_solid_plant_and_water`. The client draws the box from the target (`client/overlay.js`, `client/scene.js` `drawBox`).

Phase A (items 3 and 5) is done: animal box models and raft model art (#545, #559), the model atlas and UVs (#547, `client/models.js`), the GL model drawer (#564, `client/modelgl.js`), animals drawn as models with hurt-red and death-tip kept (#592) and the raft drawn as a model (#606, `client/raftmodel.js`). Items 1, 2, 4 and 6 are not done.

Phase B (items 1 and 6) is done: the rider's swell push removed (#686), no selection outline on water with the ray still stopping there (#703), loose items stop sliding on the deck (#698) and animals on the deck get no swell push (#707). `[swell_push]` and `SwellPush` no longer exist in the code. Items 2 and 4 are not done.

Phase C (item 2) is done: animals swim at the surface (#726), drown after `drown_ticks` under water (#733), the nearest-support search is `crates/game/src/animal_steer.rs` (#727), swimming animals steer towards support (#767), and `crates/game/tests/animal_swim.rs` covers the end-to-end determinism (#795). Item 4 is not done.

Phase D (item 4) is done: helm cell marker (#840), `[drive]` tunables and drive frame (#857), the raft stops at solid blocks in its footprint (#880), enter and leave the helm and steer in `Walker::tick` (#898), driving state and heading in game-web (#907), the raft drawn at its heading (#935), raft pose and `driving` in the world hash with a replay test (#937), and the driving input script `client/test/scripts/drive.input` (#966, #980, #982). All six items are done.

## Reconciliation notes

1. **Item 1 reverses decisions #158 and the Phase 3 "done when" line.** basic-gameplay 4.7 says standing still on a rocking raft drifts unless you sneak, and Phase 3 says "feel the deck push during swells". Both are stated by the maintainer as decided, and this idea overrides them. A doc change belongs in basic-gameplay 4.7, the Phase 3 text and the items bullet; the reconciler does not edit that doc. Suggest a follow-up that updates it after this one is accepted. Smallest change: set the rider's push to zero in code and in `data/raft.toml`, keep the swell sway and tilt. Items stop sliding too: the whole `[swell_push]` section is removed, `item_multiplier` included. Whether animals keep a push is open question 2. (Decided in #634.)
2. **Tests that rely on the push** (answers question 1): `swell_without_sneak_drifts` in `crates/game/tests/raft_stability.rs` must flip to "does not drift"; `swell_with_sneak_does_not_drift` becomes redundant; `ride_is_deterministic` is unaffected. `SwellPush`'s data validation and the `[swell_push]` keys also change. Sneak's role on the deck is edge protection only (4.5).
3. **Item 2 vs 4.3.** "No drowning in v0" in 4.3 is about the player. Animals drowning does not conflict with it, but the doc should say so. Animal swimming should reuse `body_in_water` and the 4.3 numbers by default so one water model exists (see open question 2). Animals have no navigation, only a wander state machine (`advance_wander`), so "head for land or the raft" needs a new steering step: smallest version is to pick the nearest solid, non-water support cell within a radius and wander towards it.
4. **Drowning and determinism.** Drowning ticks must come from the tick counter and the seeded `Rng` in `crates/game/src/rng.rs`, so replays stay identical (`crates/game/src/replay.rs`). Death and drops reuse the existing death path in `animal_entities.rs` unless the open question says otherwise.
5. **Item 3 needs art and a render path.** Art is edited under `art/` (text pixel tiles, one 16-colour palette, `snake_case` names unique across batches, `validate.py` must print `ok`). A new batch file and `art/PROMPT.md` entries are needed; `art/batches_7_12.json` is generated by `tools/draw_7_12.py` and must not be hand-edited. The client path is new: a textured model in `client/scene.js` instead of `unitBox` with a colour. Animal box sizes come from `data/animals.toml` `box`; a model must fit them or the doc should change them. Hurt-red and death-tip must still work.
6. **Items 3 and 5 share one need**: a client path for textured non-cube models (the boxes in `drawAnimals` are untextured). Build it once. Smallest version: a box-model drawer with per-face UVs into the tile sheet, as the PROMPT.md villager and squid entries already describe. This is a viewer/client change; `viewer/` stays framework-free (CLAUDE.md).
7. **Item 4 and the 90-degree rule.** Collision rotates only in quarter turns (#140). A smooth steering turn would need the visual yaw to be continuous while collision snaps, which can disagree with where the player stands. Simplest consistent options are in open question 3 below. Also the raft sim is not seed-only deterministic once a player steers it, but it is still a function of the input stream, so the replay invariant holds if driving reads `TickInput` only. Driving must be a new input binding in `data/input.toml` and `crates/game/src/input.rs`, tested through a `client/test/scripts/*.input` script (CLAUDE.md: client behaviour is tested by input scripts, not screenshots). Turning uses continuous yaw, and collision uses the nearest quarter turn, accepting that the two can disagree with where the player appears to stand. (Decided in #801.)
8. **Item 4 and world-layout.** world-layout Phase D's "Raft travel" and Phase E's route travel are scripted, between levels, and tagged in world-layout as game scope. Player-driven travel is a different thing and does not conflict, but should not be mistaken for it: it does not need the weekend screen. On open question 3 of this idea: multi-region streaming exists in the sim but the renderer half of world-layout Phase D is not built, so driving past the loaded area works in the sim and not in the browser. Driving can ship inside the first region; it only becomes interesting once the renderer streams.
9. **Item 4 stopping conditions** need the raft to collide with the world. `RaftWorld` (`crates/game/src/raft.rs`) overlays the raft on the base world for the player, but `Raft::advance` does not currently test the raft's own cells against terrain. Stopping at islands or shallows needs a raft-vs-terrain sweep (new code). Blocks above the water are the likely rule; shallows are blocks within a draught of the sea floor, which needs a number in `data/raft.toml`.
10. **Item 6 vs #174.** basic-gameplay section 5 says the ray stops on water and can target water. This idea changes that to at least "no outline". The Phase 4 text and the `casts_hit_solid_plant_and_water` test depend on the old rule. If the ray passes through water, `Target::Block` on water disappears, and any rule matching water as a target (for example the dispatch table in section 7) changes; check `data/interactions.toml` for water rules before choosing. Smallest change for "no outline, keep the ray stop": suppress the box in `walk.rs` where the outline box is produced, not in `selection_box`.
11. **No SPEC.md conflict.** None of the items states Beta behaviour; water physics, drowning and rafts here are this project's own. SPEC.md is not touched.
12. **Invariants.** `crates/terrain` is untouched by all six items. `crates/game` may use std. Textures are new, this project's own, in `art/`. Wasm size: a new box-model path is client JS, so the terrain wasm does not change; `game-web` may grow with the new bindings and PRs should report sizes.
13. **Size and order.** Each item is a separate piece of work. Suggested order by dependency: 1 and 6 (small rule changes, with doc updates), 2, then the shared textured-model path with 3 and 5, then 4 (largest, needs a control scheme and raft-vs-terrain collision).
14. **[manual check]** items: "standing on the raft should feel stable", "reads as a raft rather than a patch of blocks", "proper animal art". Measurable versions: (1) after 1000 ticks of swell the rider's offset from its start relative to the raft is under 1e-6 without sneaking; (3, 5) `validate.py` prints `ok` for the new tiles and every named texture is used by the client, with a human look at the final result.

## Idea

# Mince Raft: Raft scene feedback

Status: idea
Scope: game

Feedback from playing the raft test scene (`client/?scene=raft`). Six problems, each with what I want instead. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. The player slides around on the raft

I don't like sliding on the deck. Standing on the raft should feel stable.

- Today: basic-gameplay 4.7 (decided in #158) adds a down-slope push each tick from deck tilt (`data/raft.toml` `[swell_push] per_degree = 0.004`), so standing still means drifting unless you sneak.
- Want: no drift for the player while standing on the deck. The raft can still sway and tilt visually; the player rides it without being pushed around.
- Open: whether loose items on the deck should still slide (`item_multiplier`), or whether that goes too.

> **Reconciler:** `[manual check]` "feels stable". Measurable: see note 14. Overrides #158, see note 1.

---

## 2. Animals in water sink and don't drown

An animal that ends up in the water falls to the bottom of the ocean and stays there, alive.

- Today: `crates/game/src/animal_entities.rs` has no water handling; animals just fall under gravity.
- Want: animals in water try to swim. They float or paddle at the surface and head for land (or the raft) if they can find it.
- If an animal stays under water too long, it drowns and dies, with its normal death animation and drops. (Decided in #713.)
- Swimming animals use the player's water numbers from basic-gameplay 4.3 (`data/player.toml` `[water]`), plus one `swim_speed` per species in `data/animals.toml`; no separate buoyancy or gravity numbers. (Decided in #712.)

> **Reconciler:** Confirmed: no water handling in `animal_entities.rs`. See notes 3 and 4. "Under water too long" needs a number in `data/animals.toml`.

---

## 3. Animals have no sprite

Animals are plain coloured boxes (`ANIMAL_COLORS` in `client/scene.js`), red while hurt.

- Want: proper animal art for cow, pig and chicken, this project's own, added to `art/` (PROMPT.md lists no animal textures yet), and a client render path that uses it.

> **Reconciler:** Confirmed. See notes 5 and 6. `[manual check]` "proper art".

---

## 4. The raft can't be driven

`RaftFrame` has a `yaw_delta`, and the raft supports quarter turns, but nothing lets the player steer or move it. Raft travel and steering were left out of Phase 3 (#139).

- Want: a way to drive the raft: move it across the sea and turn it.
- Open: how the player controls it (a tiller or helm block to interact with, or keys while standing at a spot), speed, how turning works given the deck only rotates in 90-degree steps for collision (#140), and what stops it (islands, shallows).
- The player steers with keys while standing on a helm cell. Interacting with the helm cell enters and leaves driving (vehicle) mode. (Decided in #800.)
- Stop: any solid block in the raft's footprint stops the raft. (Decided in #802.)

> **Reconciler:** Confirmed: only the swell produces `RaftFrame`s. See notes 7 to 9. A helm block would also need a block (`data/blocks.toml`, which `crates/terrain` reads) or a game-side marker on the raft mask; a game-side marker avoids changing terrain.

---

## 5. The raft needs its own model and sprite

The raft is meshed from ordinary blocks (`scene.raft_blocks()`).

- Want: a custom raft model with its own textures (logs lashed together, deck planks, edges), so it reads as a raft rather than a patch of blocks.

> **Reconciler:** `raft_planks`, `raft_edge` and `raft_bottom` exist already; the new part is a model and any new log textures. See notes 5 and 6. `[manual check]`.

---

## 6. No selection outline on water

The selection outline shows up on water blocks. It shouldn't.

- Today: basic-gameplay section 5 (decided in #174) says every non-air block, liquids included, has a selection box, so the ray stops on the water surface and can target water.
- Want: water never shows the outline. The ray still stops at water and `Target::Block` can be water (for rules that need it, such as filling a bucket later), but no outline is drawn for it. (Decided in #636.)

> **Reconciler:** Confirmed. See note 10.

---

## Phases (reconciler)

**Phase A.** Items 3 and 5: the shared textured box-model drawer in `client/` (open question 10, default A), new cow, pig and chicken tiles plus raft log and lashing tiles in `art/`, and a raft model replacing the block mesh from `scene.raft_blocks()`. Done when `validate.py` (run from `art/`) prints `ok`, every new texture is used by the client, a client test covers the model drawer's UVs, and hurt-red and death-tip still work for animals. Final look is `[manual check]`.

**Phase B.** Items 1 and 6: remove the rider's swell push, then drop the item push too (note 1); animals on the deck get no swell push, like the rider; and stop drawing the outline on water while the ray still stops there (note 10). Done when `swell_without_sneak_drifts` is replaced by a test that the rider's offset from its start relative to the raft stays under 1e-6 after 1000 ticks of swell, and a test in `walk.rs` shows water yields no outline. (Decided in #635.)

**Phase C.** Item 2: animals swim, steer towards land or the raft, and drown, per open questions 3 and 4. A swimming animal steers towards the nearest solid, non-water support cell within a radius; the raft counts as support like any other solid. Done when tests show an animal placed in water stays at the surface, reaches a nearby support cell, and drowns after the configured ticks from `data/animals.toml`, deterministically from the tick counter and seeded `Rng`. (Decided in #714.)

**Phase D.** Item 4: driving the raft, per open questions 6 to 8; largest, so last. Done when a `client/test/scripts/*.input` script drives the raft with the chosen control, a test covers turning and the raft-vs-terrain stop, and replays of the same `TickInput` stream stay identical.

## Open questions

1. Does removing the swell push (1) change anything else that relied on it, such as tests in `crates/game/tests/raft_stability.rs`?
2. Swimming animals: same water physics as the player (basic-gameplay 4.3) or their own simpler numbers in `data/animals.toml`?
3. Raft driving: does the raft's movement need the multi-region streaming from world-layout Phase D to go anywhere interesting?
4. Is any of this better folded into existing docs (basic-gameplay for 1, 2, 6; world-layout for 4) than a new one?

> **Reconciler:** answers to 1 and 3 are in notes 2 and 8. Question 4: a separate doc is fine because this one overrides decided text in basic-gameplay (notes 1 and 10), and that text should be edited by the maintainer or a later doc change rather than by this PR.

## Open questions (reconciler)

7. **Turning (item 4).** Options: A. Quarter turns only (a key press turns 90 degrees over N ticks, visual yaw eased, collision snaps at the end). B. Continuous yaw, with collision using the nearest quarter turn. B can disagree with where the player appears to stand; suggest A.
8. **What stops the raft (item 4).** Options: A. Any solid block in the raft's footprint. B. A: plus water shallower than a draught. C. Soft stop: speed drops to zero without moving into the cell.
10. **Model rendering (items 3 and 5).** Options: A. Box models with per-face UVs from the existing tile sheet (smallest). B. A new mesh format. Suggest A.
