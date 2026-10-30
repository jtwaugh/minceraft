---
title: Fish and fishing
status: accepted
scope: game
source: "#1523"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
  D: todo
  E: todo
---

## Current state

- **Spawn query exists, no species.** `species_for(&FishColumn)` and the `data/fish.toml` loader are in `crates/layout/src/fish.rs` (`Fish::shipped`, `Fish::species_for`); `data/fish.toml` is schema comments only (decided in #93). `FishColumn` carries habitat, temperature and depth; `Layout::habitat_map` (`crates/layout/src/habitat_map.rs`) produces them per region. Habitat ids in `data/habitats.toml`: `frozen`, `warm_shallows`, `deep_water`, `rocky_shore`, `cold_open`, `temperate_open`.
- **Nothing in `crates/game` uses fish or habitats.** `crates/game` depends on `layout` with `default-features = false` and has no habitat or `species_for` call.
- **Art exists.** `salmon`, `tuna`, `mackerel` and `squid` box models are in batch 11 (`art/tools/draw_7_12.py`, `art/batches_7_12.json`); `raw_salmon`, `raw_tuna`, `raw_mackerel`, `raw_squid` and `fishing_rod` are in `art/batches_1_6.json`; `bobber`, `splash_0..3` and `fishing_wake` are in `art/batches_7_12.json`. No game code or client code references any of them.
- **Items.** `data/items.toml` has `sword` and `hoe` tools and animal drops (`raw_beef`, `raw_pork`, `raw_chicken`) but no fish items and no `fishing_rod` tool. `PlayerState::new` (`crates/game/src/items.rs`) leaves belt slots 3 and 4 as `None` (fishing rod and bucket reserved).
- **Interaction rules** (`data/interactions.toml`, `crates/game/src/interactions.rs`): `parse_target` accepts any `entity:<kind>`; `parse_action` knows `attack`, `till`, `plant`, `pick`, `forage` only. The sword rule today targets `entity:animal`.
- **Animals** (`data/animals.toml`, `crates/game/src/animal_entities.rs`, `animal_steer.rs`) already have health, hurt, flee, death and drops, with water swimming. They are the closest model for fish entities.
- **Phase clock.** `crates/game/src/phase.rs` and `data/phases.toml` alternate `gathering` and `service` from tick 0. There is no day counter or "start of day" event, and the animal daily top-up (basic-gameplay 8.4) is not built.
- **RNG.** `crates/game/src/rng.rs` has the seeded `Rng` (`range`, `chance`, `angle`).
- **Events.** `crates/game/src/events.rs` holds the events of basic-gameplay section 11; none of the fish events exist.
- **Client.** Textured box models and the model atlas are in `client/models.js`, `client/modelgl.js` (raft-scene-feedback Phase A). Input-script DSL is `client/test/dsl.mjs`.
- **SPEC 14** lists "Water creatures (5): Squid 10" only: a weight and a cap, nothing about squid behaviour.

## Reconciliation notes

1. **Squid rules.** Section 2.1 says squid "behave as SPEC 14's water spawn describes". SPEC 14 gives only a weight (10) and a population cap (5); it says nothing about swimming, depth or where squid spawn. Suggest: Phase E uses the cap and weight from SPEC 14 and takes squid movement and body data from this game's own data file, like fish; any further Beta fact is a `research` `spec-question`, not filled in by the implementer.
2. **No day counter.** Sections 3.1 and 6 say stock resets "at the start of each day". `PhaseClock` only knows gathering and service. Suggest: a day is one gathering plus one service, and the reset happens on the tick the clock enters gathering (Phase B derives it from `PhaseClock::at`). See open question 1.
3. **Phase-change spawning needs a phase-change signal.** `PhaseClock::at(tick)` returns the current phase only; Phase B must detect the change (compare to the previous tick) or add an event.
4. **Habitat map in the game.** Section 3.1 needs a habitat, temperature and depth per water column inside `crates/game`. The game has no habitat access yet; `Layout::habitat_map` is the source. Phase B should add the smallest accessor over the streamed regions (`crates/game/src/stream.rs`). Frozen columns: species with `habitats` unset match `frozen` too, and `species_for` has no "no fish" rule, so "frozen columns have no fish" must be a spawner check (or every species lists habitats). Suggest the spawner check.
5. **Tuna habitat.** "`deep_water`, `warm_shallows` edge" is not expressible in the current schema as an edge. Suggest `habitats = ["deep_water", "warm_shallows"]` plus a `depth_min` so only the deeper part of warm shallows qualifies; tune in data.
6. **Body data location.** Section 2 offers `[[species.body]]` in `data/fish.toml` or `data/fish_bodies.toml`. `fish.toml` is parsed by `crates/layout/build.rs`, which would reject unknown fields and which the layout crate should not own (game-only data). Suggest `data/fish_bodies.toml`, read by `crates/game`.
7. **Interaction rules.** `target = "entity:fish"` parses today. `action = "attack"` exists, but the sword hit path is animal-specific (`animal_entities.rs`); fish need their own entity store and the attack dispatch must reach it. The rod needs new actions (`cast`, `reel`) in `parse_action`. The rod tool needs a `[[tool]]` entry in `data/items.toml` and a belt slot in `PlayerState::new`; the existing test of the reserved slots (`items.rs`) will change.
8. **Determinism (CLAUDE.md).** Fish spawning, swimming and bites must be a pure function of seed and input, independent of region origin or size. Streamed regions load and unload; the spawner must not depend on which regions happen to be loaded beyond the water around the raft. Phase B's "two runs produce identical positions" test covers the replay; add a test that the fish chosen for a column do not change with the loaded region set.
9. **Wasm size.** Fish entities and the box models go to `crates/game`/`game-web` and `client/`; report gzipped wasm size in each PR.
10. **Section 4.2 and 4.3 are rod details with no existing hook.** Item-entity physics (`item_entities.rs`, `data/item_motion.toml`) is for items; the bobber is a new entity kind and "floats on water" needs a water-surface rest rule. Particles (`splash`, `fishing_wake`) have art but there is no particle system in `client/`; the client draws sprites only if one is added. Suggest Phase D builds the sim side and events, and draws the bobber and line as a simple model/line, with particles only if a particle path exists by then. See open question 2.
11. **[manual check]** "Cold water and warm shallows should feel different" (rule 2) and "fish below the surface are seen through the water" (3.3) are visual judgments. Measurable version: over a sample of seeds, `species_for` returns different weighted lists for a `cold_open` and a `warm_shallows` column; and a client test checks the fish model draw happens in the model pass after the water pass.
12. **Overlap with other docs.** `basic-gameplay.md` sections 6, 7, 8.3, 8.4 and 11 are extended here (belt, interaction rules, kill and drops, daily top-up, events). `raft-scene-feedback.md` owns the box-model path used in Phase C. `held-and-world-items.md` draws the held item, so a carried raw fish needs no new hands art. No conflict, but section 4.6's events should be added to the basic-gameplay event list by the Phase D ticket, not by editing that doc here.
13. **Phase D's "casting while carrying does nothing"** matches basic-gameplay section 6 (decided in #215): secondary uses the carried item, and a raw fish has no use.
14. **Stations are out of scope.** "Raw fish can reach the cooking stations" depends on stations, which do not exist yet (`item_put_down` is never emitted). This idea ends with the fish in the hands.

---

# Fish and fishing

Status: idea
Scope: game (fish data, fish entities, the fishing rod), with the layout crate's existing spawn query

The sea is the raft's pantry. Today `data/fish.toml` has its schema and loader but no species (decided in #93), nothing spawns, and the fishing rod slot on the belt is only reserved (basic-gameplay section 6). This idea fills the sea with fish you can see, and gives the player a rod to catch them, so raw fish can reach the cooking stations.

All numbers here (spawn counts, swim speeds, bite delays, reel windows) are this game's own, live in data files and are tuned to feel right. None of them is a claim about Beta.

---

## 1. Design rules

1. Fish are visible creatures in the water, not a hidden loot roll. What you see near the raft is what you can catch.
2. Which fish live where comes from the habitat map (world-layout Phase C). Cold water and warm shallows should feel different, so the route from cold to warm changes the menu.
3. Catching a fish puts it in your hands, like any other carried item. One fish, one trip. Fish are caught with the rod or with the sword.
4. Fish are a limited daily stock per anchorage. Overfish in the morning and the evening is thin.
5. The sim stays deterministic: spawning, swimming and bites use the sim RNG, keyed so that the same seed and input give the same fish.

---

## 2. Species

Ship three fish and one squid, matching the art that already exists (`salmon`, `tuna`, `mackerel`, `squid` models in batch 11; `raw_salmon`, `raw_tuna`, `raw_mackerel`, `raw_squid` items in batch 4).

| Species | Habitats | Notes |
|---|---|---|
| mackerel | `cold_open`, `temperate_open`, `rocky_shore` | Common, small, the everyday catch |
| salmon | `cold_open`, `rocky_shore` | Cold water, early in the route |
| tuna | `deep_water`, `warm_shallows` edge | Rare, large, worth more later |
| squid | per SPEC 14 | Not a fish here (section 2.1) |

The three fish are spawned by this game's own rules (section 3). Each species gets weight, habitat list and optional temperature and depth ranges in `data/fish.toml` (the schema already exists), plus a game-side block in a new `[[species.body]]` or a separate `data/fish_bodies.toml`: box size, swim speed, turn rate, school size, and the raw item it becomes.

> **Reconciler:** the raw items' art is in `art/batches_1_6.json`, not batch 4 by file name; the names match. See notes 5 and 6.

`frozen` columns have no fish.

### 2.1 Squid

Squid spawn and behave as SPEC 14's water spawn describes; this idea adds no squid rules of its own and does not put squid in `data/fish.toml`. The rod never hooks a squid. A squid killed with the sword (section 4.5) gives `raw_squid` the same way a fish gives its raw item.

> **Reconciler:** SPEC 14 has weight and cap only. See note 1.

---

## 3. Fish in the water

### 3.1 Spawning

- **Daily stock.** Each anchorage has a daily stock of fish (data, default 24). It resets to full at the start of each day, like the animal top-up in basic-gameplay 8.4. Every fish caught or killed comes out of the stock; fish that swim off or despawn go back into it.
- **Spawn at phase changes only.** Fish are spawned at each phase change (the start of gathering and the start of service), never in the middle of a phase. At each change the spawner tops the water around the raft up to a target count (data, default 12 within 24 blocks), limited by what is left of the day's stock. It does not fill the whole loaded world.
- Each school is placed at a water column in a ring around the raft (data, default 12 to 24 blocks away, out of the player's direct view where possible). The spawner asks `species_for(column)` for the weighted list and spawns a small school of one species at a depth inside that species' range.
- Spawning ignores time of day; there is no day and night filter (the open item from #93 stays open).
- Fish farther than the despawn radius (default 40 blocks) are removed and returned to the stock. They are never saved.
- Spawn choices are drawn from the sim RNG so a replay produces the same fish.

> **Reconciler:** see notes 2, 3, 4 and 8. The 12 to 24 block ring is inside the 40 block despawn radius, which is consistent.

### 3.2 Swimming

- Fish swim only inside water cells. A fish never leaves water and never rests on the bottom.
- Movement is a simple wander: pick a heading and depth inside the water, swim towards it at the species' speed, turn smoothly, pick again after a random time. School members steer loosely towards their school's centre.
- Fish keep at least 1 block below the surface and 1 block above the bottom where the water allows.
- When the player is in the water within a few blocks, fish swim away from them (a scatter, no damage).
- Fish ignore the raft except as an obstacle.

### 3.3 Rendering

Fish render as box models from the existing art (batch 11), using the textured box-model path built for animals in raft-scene-feedback. The tail swings with speed. Fish below the surface are seen through the water the same way blocks are.

> **Reconciler:** `[manual check]` for "seen through the water"; see note 11.

---

## 4. Fishing rod

### 4.1 The tool

- `tool:fishing_rod` goes into the reserved belt slot. It uses the existing `fishing_rod` texture.
- Like every tool, it is stowed while the hands carry something (section 6 of basic-gameplay). You need empty hands to fish.

### 4.2 Cast

- Secondary press with the rod casts: a bobber flies out along the look direction (data: cast speed and upward kick), using item-entity style physics, and floats where it lands on water. The line is drawn by the client from the rod tip to the bobber.
- If the bobber lands on a block instead of water, it lies there; the next secondary press reels it back with nothing.
- Only one bobber per player. Switching tools or walking more than a set distance away (data, default 24 blocks) reels it in.

### 4.3 Bite

- While the bobber floats, a bite comes after a random delay (data, default 3 to 8 seconds). The species is drawn from the fish near the bobber if any are within a few blocks, otherwise from `species_for` at the bobber's column. A fish that is visibly swimming near the bobber is the one that bites, and it swims to the bobber first, with the `fishing_wake` particle trailing it.
- On the bite, the bobber dips (and the `splash` particles play). The reel window is open for a set time (data, default 1 second).

### 4.4 Reel

- Secondary press during the reel window catches the fish: it is removed from the water and its raw item lands in the player's hands (the rod stows, as when picking up an item).
- Pressing outside the window reels in an empty line. Missing the window lets the fish swim off, and a new bite can come later.
- Fishing cannot be done while carrying, so a catch never displaces a carried item.
- The bite is a single timed press. There is no hold or reel minigame, for any species.

> **Reconciler:** see notes 7, 10 and 13. The rod's "stows" when the hands fill matches basic-gameplay section 6. "Species drawn ... otherwise from `species_for` at the bobber's column" can pick a species with no fish entity in the water; the bite must then spawn or take a stock-limited fish. See open question 3.

### 4.5 Sword

- A fish (or squid) in the water is an entity target like an animal. Primary with the sword, or a punch, within entity reach hits it, whether the player is swimming or leaning over the raft edge.
- Fish have little health (data, default one sword hit). A killed fish's raw item goes straight into the player's hands, which are empty since a tool was in use. It counts against the daily stock.
- Hit fish that survive scatter, as in 3.2.
- Add the rules to `data/interactions.toml` (`target = "entity:fish"`, `action = "attack"`).

> **Reconciler:** see note 7. "Within entity reach" uses the existing targeting numbers in `data/targeting.toml`; check the water surface does not block the ray to a submerged fish.

### 4.6 Events

Add to the events of basic-gameplay section 11:

| Event | Fields |
|---|---|
| `line_cast` | player, position |
| `fish_bite` | player, species |
| `fish_caught` | player, species, item, method (`rod` or `sword`) |
| `fish_escaped` | player, species |
| `line_reeled` | player, caught |

---

## 5. Phases

**A. Species and items.** Species list in `data/fish.toml`, body data, raw fish items in `data/items.toml`. Done when `species_for` returns shipped species in every non-frozen habitat over a sample of seeds, and each raw item can be spawned with the console and carried.

**B. Fish entities.** Daily stock, spawning at phase changes, despawn, swimming and scatter (section 3) in `crates/game`, with a headless test. Done when, over several in-game days with the raft at anchor, fish appear only on the tick of a phase change, the count never exceeds the target or the remaining stock, the stock resets each morning, no fish is ever outside water, every fish's species is one `species_for` allows for its column, and two runs with the same seed and input produce identical fish positions.

**C. Fish in the client.** Box models and swim animation in the browser scene. Done when fish are drawn in the water around the raft and an input script can expect a fish entity near a position.

**D. Fishing rod and sword catches.** Section 4 and its events. Done when an input script casts, waits for a bite, reels in the window and ends with a raw fish in the hands; a press outside the window catches nothing; casting while carrying does nothing; and a sword hit on a fish ends with its raw item in the hands and the stock one lower.

**E. Squid.** Section 2.1, following SPEC 14. Done when squid spawn as SPEC 14 says, the rod never hooks one, and a sword kill gives `raw_squid`.

> **Reconciler:** Phase A is smallest and independent. Phase A's "every non-frozen habitat" needs the species list to cover `warm_shallows`, `deep_water`, `rocky_shore`, `cold_open` and `temperate_open`; the table covers all five only if tuna's habitats include `warm_shallows` and `deep_water` (they do). Phase B's "every morning" needs note 2's day definition; its "stock resets" and "fish appear only on a phase-change tick" are testable headless. Phase C's "an input script can expect a fish entity" needs a DSL expectation (`client/test/dsl.mjs`) that does not exist yet; extend the DSL as CLAUDE.md says. Phase D's "waits for a bite" needs the bite delay seeded so the script is deterministic. Phase E is limited by note 1.

---

## 6. Decisions

1. Fish are caught with the rod and with the sword (section 4.5); not by hand.
2. Each anchorage has a daily stock that fishing depletes, reset each morning (section 3.1).
3. Time of day is ignored; fish spawn only at phase changes (section 3.1).
4. Squid follow SPEC 14's water spawn and can't be hooked with the rod (section 2.1).
5. The bite is a single timed press for every species (section 4.4).

## Open questions

1. **What is a "day" for the stock reset?**
   - A: one gathering plus one service; reset when gathering starts (suggested).
   - B: wait for a real day clock (basic-gameplay 8.4 defers it) and build the fish stock after it.
2. **Particles for `splash` and `fishing_wake`.** The client has no particle system.
   - A: Phase D ships without particles and a follow-up adds them (suggested).
   - B: add a minimal sprite-particle path to the client in Phase D.
3. **A bite on a species with no fish nearby (section 4.3).**
   - A: the bite spawns a fish of the drawn species just out of view and swims it to the bobber, taking one from the stock (suggested; keeps rule 1 approximately true).
   - B: no bite if no fish is within a few blocks (the rod needs visible fish).
4. **Squid body and swim data (note 1).** A: game-owned `data/fish_bodies.toml` entry like the fish (suggested). B: open a `research` question for the facts SPEC 14 lacks.
5. **Stock when the stock is empty and the bobber is cast.** A: no bites until the next reset (suggested; matches rule 4). B: allow a slow trickle.
