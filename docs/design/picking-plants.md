---
title: Picking plants and the seed loop
status: accepted
scope: game
source: "#503"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
  D: todo
---

## Current state

Checked against the code:

- **Interaction rules** are in `data/interactions.toml`: `till`, `plant` (`item:seeds` on `block:farmland`, top face, air above), `harvest` (empty hands or any tool, `block:crop`, `require_state = "mature"`), `forage` (empty hands, `block:tall_grass`) and `attack`. The first match wins. There is no `pick` action and no rule targeting `block:dandelion` or `block:rose`.
- **Actions** live in `crates/game/src/`: `plant.rs`, `harvest.rs` (resets the crop to stage 0 and returns the yields from `data/crops.toml`), `forage.rs` (`Forager`, per-block cooldown), `till.rs`, `farm.rs`.
- **Foraging data** is `[forage]` in `data/items.toml`: item `seeds`, `chance = 6`, `cooldown_ticks = 1200` (one minute at 20 Hz; basic-gameplay 9.5 says one day by default). A forage inside the cooldown does nothing and rolls nothing; any other forage starts the cooldown, hit or miss. Tilling grass uses `[till]` `chance = 8`.
- **Events**: `foraged` and `crop_harvested` exist (`crates/game/src/events.rs`, basic-gameplay section 11). There is no event for a miss, so the client has nothing to show when a forage drops nothing.
- **Items** in `data/items.toml`: `seeds`, `wheat`, animal drops, two tools. There are no flower items. `art/PROMPT.md` lists `seeds` and `wheat` but has no flower item icons.
- **Flower blocks** `dandelion` (id 17) and `rose` (id 18) are in `data/blocks.toml` (collision none, tinted tiles in `client/tiles.js`). They are placed by terrain decoration, so they exist in the world and on islands.
- **Tests**: `client/test/scripts/farming.input` plants with seeds from `give item seeds`; `client/test/scripts/aim.input` checks a placed tall grass shows `rule.secondary = forage`. No input script forages and expects seeds, and none plants seeds that came from foraging. `crates/game/tests/farming.rs` and `events.rs` cover the headless side.
- **Overlap**: `docs/design/biome-flora.md` open question 3 ("Do new plants drop anything?", option B "Berries and flowers drop items") overlaps with this doc. Picking flowers is the answer for dandelion and rose.

## Reconciliation notes

1. **Rule 2 conflict.** basic-gameplay section 1 rule 2 says "Players cannot break or place blocks", and 9.1 relies on "allowed under the no-breaking rule". Section 1 here loosens it. This design supersedes that rule for plants only; the docs updater or a follow-up edit to `basic-gameplay.md` should reword the rule (the reconciler may not edit that doc). Terrain, logs and raft cells stay unbreakable.
2. **9.4 conflict.** basic-gameplay 9.4 resets a harvested crop and says no seed is needed to replant "since breaking is not allowed". Picking a crop changes that rationale. Decision: picking replaces harvest-and-reset. Pick removes a crop at any stage and drops its produce only if the crop is mature; replanting needs seeds, and harvest-and-reset goes away. (Decided in #511.)
3. **Removing a block.** `FarmWorld` (`farm.rs`) is a world overlay for crops and farmland; flowers are terrain blocks. Picking a flower must remove a terrain block, which no action does today. The game world (`CollisionWorld` and the client mesh) needs a way to record an edit and re-mesh the chunk. Decision: the world gets a general block-edit layer that later features (place, break) can use, not a plants-only overlay. The blocks that may be removed are limited to foliage if a limit is needed; terrain, logs and raft cells stay unbreakable (note 1). Whether edits persist across region reloads is open question 9. (Decided in #512.)
4. **Picking tall grass.** Section 3 keeps tall grass as a forage target (the grass stays). `pick` must apply to flowers and crops only, so the `forage` rule for tall grass is unchanged.
5. **Held items.** Item pickup and hands rules are in `items.rs` and `player.rs` (Phase 5 of basic-gameplay). "Hands full, drop as an item entity" matches the existing item-entity code (`item_entities.rs`); no new mechanism is needed.
6. **Art.** Each new item needs an icon in `art/` (one 16-colour palette, 8x8, `snake_case` names; `art/PROMPT.md` lists what should exist). Flower icons also need a `PROMPT.md` entry. That file is not in the design lane, so it is a ticket, not part of this doc.
7. **Input scripts.** Section 3 and 4 say "if it doesn't work, add an input script". Per CLAUDE.md, a script comes first either way. Phase D is section 4 only (the seed-planting script) and is mostly tests; if the script passes, nothing else changes. The foraging script of section 3 belongs to Phase C. (Decided in #510.)
8. **[manual check]** "In play that feels like foraging does nothing" (section 3) is a feel judgment. A measurable version: with the shipped chance and cooldown, a player foraging distinct tall grass blocks gets seeds within N tries with probability at least P (for example, 1 in 6 gives 90% within 13 tries). The tests can assert the chance and cooldown values and the feedback event; whether it feels right is for the maintainer.
9. **[manual check]** "I want the farming loop to feel natural" has no test; the loop script (forage, plant, grow, harvest) is the closest measurable check.
10. **Phases.** One phase per section of the author's text: Phase A is section 1 (the `pick` action for crops, loosening rule 2), Phase B is section 2 (flowers: items, icons, overlay), Phase C is section 3 (foraging) and Phase D is section 4 (seed planting script). (Decided in #510.)

---

# Mince Raft: Picking plants and the seed loop

Status: idea
Scope: game

I want the farming loop to feel natural: pick flowers and crops by hand, forage for seeds, and put seeds into farmland. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. Loosen "no breaking blocks"

basic-gameplay rule 2 says "Players cannot break or place blocks." Loosen it: the player can **pick** plants (flowers and crops), which removes the plant and gives its item. Breaking terrain (dirt, stone, logs, raft cells) stays off.

- Today a mature crop is harvested in place and resets to stage 0 (section 9.4), and the doc says "No seed is needed to replant, since breaking is not allowed." Decide whether picking a crop replaces that (pick removes it; replant with seeds) or sits beside it (harvest resets a mature crop; pick removes a crop at any stage).

> **Reconciler:** `harvest.rs` already returns yields and resets to stage 0; a `pick` action for crops would remove the `FarmTile::Crop` and leave the farmland. See note 2.

## 2. Flowers are pickable

Dandelions and roses (and any other flower block in `data/blocks.toml`) can be picked. Picking removes the flower and puts a flower item in the hands (or drops it as an item entity if the hands are full). Flowers need items in `data/items.toml` and icons in `art/`.

> **Reconciler:** `data/blocks.toml` has exactly two flowers today, `dandelion` and `rose`. `biome-flora.md` plans more (for example `cactus_flower`); a `pick` rule keyed on a block group would cover them without a rule per flower. See notes 3 and 6.

## 3. Foraging yields seeds

Foraging tall grass should reliably give seeds.

- Today: there's a `forage` rule for `block:tall_grass` with empty hands (`data/interactions.toml`), and `data/items.toml` `[forage]` drops seeds with probability 1 in 6, with a 60-second cooldown per block. Tilling grass drops seeds 1 in 8. In play that feels like foraging does nothing. Raise the odds, or always drop, or show something when a forage misses.
- If foraging doesn't work at all in the browser, that's a bug: add an input script that forages and expects seeds.

> **Reconciler:** confirmed: `chance = 6`, `cooldown_ticks = 1200`, tilling `chance = 8`. A miss starts the cooldown and emits nothing, so a miss is invisible. See note 8 and open question 2.

## 4. Place seeds in farmland

Holding seeds and using them on farmland plants a crop.

- Today: there's a `plant` rule for `item:seeds` on `block:farmland` (top face, air above), and `client/test/scripts/farming.input` plants with seeds from `give item seeds`. Check it works with seeds that came from foraging and in the raft scene, not just in the test script. If it doesn't, that's a bug for an input script.

> **Reconciler:** confirmed. Seeds are one item name whatever their source, so foraged seeds should plant the same; no script proves it yet (note 7). Farmland exists on land only; the raft scene has a tillable field, and raft planters wait for raft travel (basic-gameplay 9.1, #446).

## Open questions

2. Foraging: always drop seeds, or better odds plus feedback on a miss?
3. Do picked flowers do anything yet (sell, decorate, cook), or are they just items for now?
4. Can picked plants grow back (flowers respawning on grass over days), or are they gone?

## Open questions (reconciler)

6. **Foraging (author's question 2).**
   - A: always drop seeds, keep the cooldown.
   - B: better odds (for example 1 in 3) plus a `forage_missed` event so the client can show a cue.
   - C: both: a guaranteed drop on the first forage of a block, then the existing odds.
7. **Forage cooldown.** `data/items.toml` uses one minute; basic-gameplay 9.5 says one day by default. Which is meant?
   - A: keep one minute, and update 9.5.
   - B: one day, and raise the odds so a miss isn't a long wait.
8. **Flower items (author's question 3).**
   - A: items only for now, with icons; no use.
   - B: also a use in this doc (decoration or a sale at a station).
9. **Regrowth (author's question 4).**
   - A: picked plants stay gone; the generator still places new ones when a region reloads from the seed.
   - B: a flower regrows on its grass block after a number of days (data).
   - Note: with the block-edit layer (note 3), whether a reload restores the flower depends on whether the edits are kept. Decide that with this question.
