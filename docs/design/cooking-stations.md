---
title: Cooking stations and storage chests
status: accepted
scope: game
source: "#1525"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
  D: todo
  E: todo
  F: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **Raft structure.** `data/structures/raft_start.toml` is 5 by 5 by 2 with `raft = true`, a palette of `raft_planks`, `butcher_block`, `griddle`, `crate`, and one butcher block, one griddle and one crate on the deck. `crates/layout/src/structure.rs` and `builder.rs` stamp it and build the raft mask; `crates/game/src/raft.rs` builds the `Raft` entity from mask cells and turns it with the heading.
- **Blocks.** `data/blocks.toml` has `raft_planks` (25), `butcher_block` (26), `griddle` (27) and `crate` (28), all `collision = "full"`. There is no `counter`, `chest` or `plate_rack`. The doc says "none of them are blocks" for the stations; they are in fact blocks (full cubes) with no behaviour.
- **Art.** `art/PROMPT.md` lists `butcher_block_top/side/bottom`, `knife`, `rice_grains`, `nori`, `cucumber`, `raw_salmon`, Batch 5 dishes (`salmon_sashimi`, `salmon_nigiri`, `cucumber_maki`, `rice_bowl`, `bread`, ...) and `puff_0..7`. `crate_top/side/bottom` are drawn in `art/tools/draw_7_12.py`. Not listed: `counter_*`, `chest_*`, `plate_rack`, `plate`, `salmon_slices`, `cooked_rice`, `cucumber_slices`.
- **Hands and dispatch.** `crates/game/src/interactions.rs` loads `data/interactions.toml`; its `held` values are `empty`, `any`, `any_tool`, `tool:<name>` and `item:<name>`, buttons include `primary`, `secondary` and `interact`, and `target_face` exists. There is no `any_item` held value, no `block:station` target class, and no `put_down` action. Interact while carrying is pinned as a no-op (basic-gameplay section 6, decided in #215).
- **Items.** `data/items.toml` has tools `sword`, `hoe` and items `seeds`, `raw_beef`, `leather`, `raw_pork`, `raw_chicken`, `feather`, `wheat`, two flowers. It has no `knife`, `raw_salmon`, `rice_grains`, `nori`, `cucumber`, `plate` or any food from this idea. `data/fish.toml` ships no species (world-layout, decided in #93), so nothing yields `raw_salmon` yet.
- **Events.** `crates/game/src/events.rs` emits the basic-gameplay section 11 events. `item_put_down` is listed but never emitted (`crates/game/tests/events.rs`, decided in #471).
- **Phase clock.** `crates/game/src/phase.rs` and `data/phases.toml` alternate gathering and service (120 s each) from tick 0. The level file (`data/levels/v0.toml`) has `week = 1` per level but no supplies table. There is no round-start hook other than reading the clock.
- **Block edits on land.** The world edit layer decided in #512 is `FarmWorld` in `crates/game/src/farm.rs` (a per-position `edits` map consulted by `block_at`). Planters on land are placed by the test scene and a console command (#451).
- **Inventory view.** `client/inventory.js` and `client/inventoryui.js` show only the belt and hands (`ui-art-and-inventory.md` Phase C, done). The multi-slot inventory was deferred to a separate design (#731); no multi-slot view exists.
- **Particles.** `client/particles.js` exists; no `puff` particle is wired (no `puff` in `client/`).
- **Nothing in this doc is built yet.**

## Reconciliation notes

1. **Stations already exist as blocks.** The intro says the butcher block, griddle and crate "are not blocks". They are in `data/blocks.toml` (ids 26 to 28) and stamped by `raft_start`; what they lack is behaviour. Phase A only adds `counter`, `chest` and `plate_rack`, and removes `crate` from the palette.
2. **Removing `crate` touches world-layout.** `world-layout.md` section 4.4 and its Current state name `crate` in `raft_start`. The doc says `crate` "is not added as a block", yet it exists as id 28. Suggest keeping the block id (ids are stable and the existing `crate_*` art stays) but dropping it from `raft_start`'s palette, rather than deleting the block. Either way `crates/layout` tests that stamp the raft (`raft_tests.rs`, `structure_tests.rs`) and the exclusion and validation sizes change if the deck grows beyond 5 by 5, as the doc allows. Also check the raft footprint and flat-grass checks (world-layout Phase B) against the larger deck.
3. **Interaction grammar needs extending.** The doc's `held = "any_item"` and `target = "block:station"` do not exist in `interactions.rs`. Phase B must add them (and a station target class that covers `counter`, `butcher_block`, `griddle`, `chest`, `plate_rack`). `tool:knife` needs a `knife` entry in `data/items.toml` before a rule can name it. This also revises basic-gameplay section 6's "interact does nothing unless a station or counter is targeted" (consistent: it is the planned exception).
4. **Hands rule is consistent.** The doc follows basic-gameplay section 6 and 7 (one thing carried, tools stowed while carrying). Using the knife needs empty hands, which matches. Putting down is the "arrives with stations" exception; no conflict.
5. **Station state per raft cell.** The doc keeps items on stations in the raft's own coordinates so they turn with the raft. The raft's cells are `[i32; 3]` mask cells (`raft.rs`); the state belongs in `crates/game` next to the raft (a map keyed by raft-local cell), not in the layout crate. On land (section 2.1) the key is the world position and edits go through the block-edit layer (`FarmWorld`, `farm.rs`). The two keyings should share one station-state type so processing and timers are written once.
6. **Chest view is not an inventory grid yet.** Section 7.2 reuses "the inventory view from ui-art-and-inventory", but that view only shows belt and hands; the multi-slot inventory is a separate, undesigned doc (#731, `ui-art-and-inventory.md` note 1). Phase C must build a chest-slot picker mode in `client/input.js` and `client/controller.js` (a new mode like inventory mode, sim keeps ticking with neutral input, as decided in #665 and #666), drawn from the Batch 10 slot art. It should not be assumed to be the future inventory.
7. **No source for fish or food items.** `raw_salmon` and the staples exist as art but not as items, and no species yields salmon (`data/fish.toml` is empty). The Phase E test ("builds salmon nigiri from a raw salmon") must put the raw salmon in a chest via the level's supplies or the console `give`, not wait on fishing. Suggest listing `raw_salmon` in test supplies.
8. **Level supplies and restock timing.** The level file has `week` but no supplies, and "every day" restock needs a round-start signal. `PhaseClock::at(tick)` gives the phase but no edge event. Suggest the sim detects the gathering-start edge from the clock and calls restock; "once a week" restocks on the first gathering phase of a level (there is no weekend or multi-day week code yet: `world-layout.md` says the weekend screen is not built). Treat "first day of the week" as the first round of the level until a week exists. Supplies live in `data/levels/*.toml` as an array of tables so `crates/layout`'s level loader must accept and validate them (known item names, known chest index).
9. **Determinism.** Station progress is counted in sim ticks and knife presses, never wall time, and swell never enters it, which meets CLAUDE.md's determinism invariant. Restocking and slot choice (first free slot) are deterministic. No randomness is needed.
10. **Invariants.** Nothing here touches `crates/terrain`, so `no_std` and world output are unchanged; the new blocks are never placed by the generator (like `farmland_*`). Recipes and capacities go in `data/recipes.toml` and the chest slot count in a data file, per the "data lives in `data/*.toml`" rule. New data files need loaders with `data_error.rs`-style validation, and `blocks.toml` additions must update the web crate's block list if the viewer legend enumerates blocks. `crates/game-web` and the client grow; report wasm sizes in the PRs.
11. **Art tasks.** New textures (`counter_top/side`, `chest_top/side/front`, `plate_rack`, `plate`, `salmon_slices`, `cooked_rice`, `cucumber_slices`) are added to `art/PROMPT.md` and drawn in `art/tools/draw_7_12.py` or a new batch tool (never by hand-editing generated JSON), and every item needs a texture, which `art/tools/test_art.py` already enforces for `data/items.toml` names. The `chest_front` face needs a facing, which the cube blocks do not have today; suggest a fixed front for v0 (all four sides use `chest_side`, `chest_front` on the face toward the raft's bow) or a block-state rotation in the raft's frame, to be decided (open question 2).
12. **Tick size.** Section 4 timings are "data, tuned for feel" and the examples (`work = 6`, `ticks = 200`) are placeholders; they are not tested values. The sim runs at 20 Hz (`ticks = 200` is 10 s).
13. **`[manual check]` criteria.** "Tuned for feel", the progress bar and done tick being "readable", the item "rendering resting on the station top", the `puff` look, and the chest "short shake" and sound hook are visual judgments. Measurable versions: progress and done state exposed as numbers that input scripts `expect` (the doc already asks for this); the shake as a `chest_refused` flag in the client state for one frame range; items on stations as model instances whose transform equals the station-top position in raft-local coordinates.
14. **`[external]` items.** None. Sound is a hook only (no audio assets in this repo).
15. **Overlap with other docs.** `basic-gameplay.md` owns hands, dispatch and events (this doc extends sections 6, 7 and 11). `world-layout.md` owns `raft_start` and the raft mask. `held-and-world-items.md` (done) owns held-item and loose item entities, which the doc keeps as is for Q-dropped items. `picking-plants.md` and decision #512 own the block-edit layer. `ui-art-and-inventory.md` owns the slot art. No conflict beyond notes 2, 3 and 6.
16. **Phase sizing.** Each phase below is large for the 200-line PR limit; the planner should split them into tickets. Phase D in particular (recipes, knife, butcher work, griddle timer, plate rack, progress bars) is likely several tickets.
17. **Phase order.** Phase C (chests) does not depend on D or E, and Phase F (land) only needs B's state type if it is keyed by position from the start. Suggest building B's state keyed so that F is a small change (note 5). Phase E's test uses the chest from C and the stations from D.

## Mince Raft: Cooking stations and storage chests (author's text)

# Cooking stations and storage chests

Status: idea
Scope: game (station and chest blocks on the raft, putting items down, processing, plates and dishes, storage), plus the blocks and art that the raft structure needs

The raft is a kitchen. `data/structures/raft_start.toml` already places a butcher block, a griddle and a crate on the deck, but none of them are blocks, and the hands rule (basic-gameplay section 6) says put-down "arrives with stations". This idea makes those stations work, replaces the crate with storage chests that hold the day's supplies and the food you gather, and gets ingredients all the way to a plated dish. Taking orders and serving villagers is a later idea.

All timings and capacities are this game's own, live in data files and are tuned for feel.

---

## 1. Design rules

1. Everything follows the Overcooked hands rule: you carry one thing, and you put it down on a surface to free your hands.
2. Stations are blocks, usually on the raft deck, where they move and turn with the raft along with everything sitting on them. Levels and the console can also place them on land.
3. Each station holds at most one thing on top. A plate counts as one thing, however many ingredients are on it.
4. Ingredient changes are data: a table of "this item on this station becomes that item", not one code path per food.
5. Interact (E) is the only button for putting down and taking. Station work uses the tool the station needs.
6. Cooking is straightforward: nothing burns, and the rocking deck never gets in the way of work.
7. The sim stays deterministic, including progress.

---

## 2. Blocks

Add to `data/blocks.toml` (all full collision, all part of the raft mask when stamped by `raft_start`):

| Block | Role | Art |
|---|---|---|
| `raft_planks` | Deck | exists (`raft_planks`, `raft_edge`, `raft_bottom`) |
| `counter` | Plain surface, holds one thing | new: `counter_top`, `counter_side` |
| `butcher_block` | Chopping and slicing | exists |
| `griddle` | Cooking over time | exists |
| `chest` | Storage, stocked with the day's supplies | new: `chest_top`, `chest_side`, `chest_front` |
| `plate_rack` | Dispenser of clean plates | new |

Extend `raft_start` so the starting deck has at least: one butcher block, one griddle, two counters, two chests and one plate rack. The `crate` cell in today's `raft_start` becomes a chest, and `crate` is not added as a block (the existing `crate_*` art stays unused). The deck may grow from 5 by 5 to fit them; world-layout's exclusion and validation must follow.

Stations are not breakable or movable in this idea.

### 2.1 Stations on land

Stations and chests also work on land, where a level file or the console (`setblock`-style, as for planters in #451) places them. On land they are recorded in the world block-edit layer (#512) and their contents are kept per world position instead of per raft cell. Everything else in this idea behaves the same on land and on the raft.

---

## 3. Putting down and taking

- **Put down:** interact while carrying, targeting the top of an empty station, places the item on it. The item renders resting on the station top. Emits `item_put_down`.
- **Take:** interact with empty hands, targeting a station with something on it, picks it up. Emits `item_picked_up`.
- Interact while carrying, targeting a station that already has something, tries to **combine** (section 6). If nothing combines, nothing happens.
- Items on stations are not item entities: they don't slide with the deck push and can't fall off. They are kept per raft cell, in the raft's own coordinates, so they turn with the raft.
- Items dropped with Q onto the deck stay loose item entities and slide, as today.
- Add the interaction rules to `data/interactions.toml` (`held = "any_item"`, `button = "interact"`, `target = "block:station"`, `action = "put_down"`, and so on) so the dispatch table stays the only path.

---

## 4. Processing

### 4.1 Recipe data

`data/recipes.toml` lists every transform:

```toml
[[process]]
station = "butcher_block"
input = "raw_salmon"
output = "salmon_slices"
mode = "work"          # needs the player working it
work = 6               # knife presses

[[process]]
station = "griddle"
input = "rice_grains"
output = "cooked_rice"
mode = "timed"         # runs on its own
ticks = 200
```

There is no burning: a finished item waits on the griddle until it is taken.

### 4.2 Butcher block (work)

- A new belt tool, `tool:knife` (texture exists). Secondary press with the knife on a butcher block holding a processable item adds one unit of work. When the work total is reached, the item becomes its output. Emits `item_processed`.
- Progress is kept if you walk away and come back.
- Deck tilt and swell never slow, interrupt or undo work.
- Hands must be empty to use the knife (the tool rule).

### 4.3 Griddle (timed)

- An item on the griddle cooks by itself. When `ticks` pass it becomes its output and stays that way. The `puff` particles play while cooking.
- Taking the item off stops the timer. Putting a half-cooked item back continues it.
- Emits `item_processed`.

### 4.4 Progress display

A small progress bar above any station with work or cooking in progress, and a done tick when it finishes. Readable from the client, so input scripts can `expect` progress.

---

## 5. Plate rack

- The **plate rack** dispenses clean plates with no limit: interact with empty hands takes one. Dirty plates and washing are out of scope.
- Ingredients never come from a dispenser; bought staples come from the chests (section 7).

---

## 6. Plates and dishes

- A `plate` item holds an unordered set of ingredients. Putting an ingredient onto a plate (interact while carrying the ingredient, targeting a plate on a station, or the reverse) adds it if the result is still part of some dish.
- `data/recipes.toml` lists dishes:

```toml
[[dish]]
id = "salmon_sashimi"
ingredients = ["salmon_slices"]

[[dish]]
id = "salmon_nigiri"
ingredients = ["salmon_slices", "cooked_rice"]

[[dish]]
id = "cucumber_maki"
ingredients = ["nori", "cooked_rice", "cucumber_slices"]
```

- When a plate's set matches a dish, the plate shows that dish's art (batch 5 already has the sashimi, nigiri, maki, rice bowl and bread icons). A partial plate shows its ingredients.
- Emits `dish_assembled`.
- Ingredients that need art and don't have it yet (`salmon_slices`, `cooked_rice`, `cucumber_slices`, `plate`) are added to `art/PROMPT.md` and drawn.

---

## 7. Storage chests

Chests are the raft's pantry. They start each round stocked with the supplies the raft has bought, and they hold whatever food the player gathers. Unlike counters, they hold many things.

- A chest has a fixed number of slots (data, default 18). Each slot holds one item; there is no stacking.

### 7.1 Stocking and restocking

- Each level lists the raft's supplies (data in the level file): items and counts, for example 8 `rice_grains`, 4 `nori`, 4 `cucumber`, and which chest each goes in. The counts are set so a level's days can be cooked without running out of staples; gathered fish, meat and crops are what vary.
- The level also says how often supplies come: every day (default) or once a week.
- At the start of each round (the gathering phase that begins each day, or the first day of the week for weekly supply), each chest is topped back up to its listed counts. Restocking only adds; items the player stored stay. If a chest is too full to take its whole restock, the rest is lost and the HUD says so.
- Restocking emits `chest_restocked` with what was added.

### 7.2 Using a chest

- **Deposit:** interact while carrying, targeting a chest with a free slot, puts the item in the first free slot. Emits `item_stored`.
- **Withdraw:** interact with empty hands, targeting a chest, opens a small chest view (the inventory view from ui-art-and-inventory, showing the chest's slots). Picking a slot puts that item in your hands and closes the view. Emits `item_taken`.
- A full chest refuses deposits with a short shake and a sound hook.
- Chest contents are kept per raft cell and persist across the day and phase changes within a session. Saving them to disk is a later save-game question.
- There is no spoilage in this idea.

---

## 8. Events

Add to basic-gameplay section 11:

| Event | Fields |
|---|---|
| `item_put_down` | item, target (already listed, now emitted) |
| `station_worked` | station, item, progress |
| `item_processed` | station, from, to |
| `dish_assembled` | dish, position |
| `item_stored` | item, chest, slot |
| `item_taken` | item, chest, slot |
| `chest_restocked` | chest, items |

---

## 9. Phases

**A. Blocks and the deck.** Section 2 (not 2.1): blocks in `data/blocks.toml`, new art, the extended `raft_start`. Done when a fresh level stamps the new deck, every station cell is in the raft mask, and the stations render and collide.

**B. Put down and take.** Section 3, with per-raft-cell station state. Done when an input script puts a raw fish on a counter, walks away while the raft turns, comes back and takes it; and a headless test shows items on stations keep their raft-relative cell through 1,000 ticks of swell and raft travel.

**C. Storage chests.** Section 7, including the chest view and restocking. Done when an input script deposits three different items, withdraws the second one by slot and ends with it in hand; a full chest refuses a deposit; and a headless test shows chests topped up to the level's list at each round start, with stored items kept.

**D. Processing.** Sections 4 and 5: recipe data, knife, butcher block work, griddle timing, plate rack, progress bars. Done when tests show slicing takes exactly the data's work count, cooking finishes on exactly the data's tick count and the result never changes after, taking an item off pauses its timer, and work progress is the same with and without swell.

**E. Plates and dishes.** Section 6. Done when an input script builds salmon nigiri from a raw salmon and rice from a chest on the deck, and an invalid combination is refused.

**F. Stations on land.** Section 2.1. Done when the console places a griddle and a chest on the test islet and an input script cooks rice on it and stores the result.

---

## 10. Decisions

1. No crate. Chests are stocked with each level's supplies and topped up between rounds (section 7.1).
2. Withdrawing from a chest opens a chest view to pick a slot (section 7.2).
3. Stations and chests also work on land (section 2.1).
4. No burning (section 4.3).
5. Swell and tilt never affect station work (section 4.2).

## Open questions

1. **Keep or delete the `crate` block?** (note 2)
   - A. Keep id 28 `crate` in `data/blocks.toml`, unused by any structure (stable ids, `crate_*` art stays).
   - B. Delete the block and renumber nothing; leave id 28 unused.
   - C. Delete it and shift later ids (breaks saved edits and tests; not recommended).
2. **Which way does the chest face?** (note 11)
   - A. Fixed: `chest_front` is on the +Z face in raft-local coordinates, and the layout places chests so that face points at the deck.
   - B. Add a facing to the block placement in `raft_start` (a palette variant such as `chest_n`, `chest_e`).
   - C. No front for v0: all four sides use `chest_side`; `chest_front` is drawn later.
3. **What is "the first day of the week" for weekly supply?** (note 8)
   - A. The first gathering phase after the level loads (no week structure exists yet).
   - B. Wait for the weekend screen design before building weekly supply; build only "every day" now.
4. **Where does the chest view's code live, given that the multi-slot inventory (#731) is undesigned?** (note 6)
   - A. A chest-only picker mode, built now, reused later if the inventory design agrees.
   - B. Wait for the inventory design and build chests with deposit and withdraw-first-slot only.
5. **Which items must exist for tests before fishing exists?** (note 7)
   - A. Add `raw_salmon`, `rice_grains`, `nori`, `cucumber` and their derived items to `data/items.toml` with the recipes, supplied via level supplies and the console `give`.
   - B. Wait for the fishing design to supply `raw_salmon`.
