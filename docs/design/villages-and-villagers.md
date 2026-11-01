---
title: Villages and villagers
status: accepted
scope: both
source: "#1528"
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

Checked against the code on 2026-10-30. Nothing in this doc is built; there is no village, mainland or villager code anywhere in `crates/`, `data/`, `client/` or `viewer/`. What it relies on:

- **Structure templates.** `crates/layout/src/structure.rs` has `Structure` (size, `origin_offset`, `raft` flag, palette-resolved blocks) with `cells(origin, rotation)` turning in quarter turns. `build.rs` embeds only `data/structures/raft_start.toml`, and `Structure::named` knows only `"raft_start"`. There is no `kind`, `connectors`, `spawns` or `rotation` list, and no loader for a directory of templates.
- **Stamping.** `Layout.structures` holds `StructurePlacement`s (`crates/layout/src/region.rs`), stamped per region; `crates/layout/src/builder.rs` places the raft this way.
- **Exclusion.** `Exclusion` (`crates/layout/src/lib.rs`) has only `Circle` and `Corridor` variants. Decoration respects it; terrain does not.
- **Route and islands.** `crates/layout/src/route_layout.rs` puts every island on the left of the heading (`(dz, -dx)`, decided in #631). `data/route.toml` gives `channel_half_width = 6`, `channel_falloff = 6`, `channel_clear_above = 4` and `corridor_margin = 2`. `build_route_layout` returns `(Layout, Vec<Level>)`; `Level` (`level.rs`) has `id`, `week`, `anchorage`, `island_center`, `spawn_water_radius` and no village record.
- **Validation.** `data/validation.toml` and `validation*.rs`, `route_validation.rs` hold the 6.1 checks and the route navigability check (`navigability.rs`, `pathfind.rs`), with the retry loops in `retry.rs` and `route_retry.rs`.
- **Climate.** `AlongPath` climate bias on the route is built (world-layout Phase G), so the warm south already exists.
- **Entities.** `crates/game/src/animal_entities.rs` has the animal store, with `ANIMAL_KIND = "animal"` matched by `entity:animal` rules in `data/interactions.toml`. `item_entities.rs` is the other entity store. `targeting.rs` has `TargetableEntity`.
- **Events.** `crates/game/src/events.rs` has `GameEvent` including `Swing`. There is no `villager_greeted`.
- **Phase clock.** `crates/game/src/phase.rs` is a gathering/service clock from tick 0. There is no day and night cycle.
- **Rendering.** The textured box-model path from raft-scene-feedback Phase A is done (`client/models.js`, `client/modelgl.js`; animals drawn as models). `client/particles.js` exists (from picking-plants Phase C).
- **Art.** `art/PROMPT.md` lists `villager` (head, nose, robe, arms, legs) and the `happy_villager` and `angry_villager` particles.
- **Partial-height collision.** The game-side shape table (basic-gameplay 4.4, #88, #116) exists in `crates/game`.

## Reconciliation notes

1. **Template format.** The doc says templates use the `raft_start.toml` format plus `kind`, `connectors`, `spawns` and `rotation`. Today only `raft_start` is embedded and the parser rejects unknown keys (check `structure_rules.rs`). Suggest Phase A extends the parser and `build.rs` to read every file in `data/structures/villages/`, without changing `raft_start`.
2. **Block names.** Section 2.4 says `planks` and `log`. `data/blocks.toml` has no `planks`, `thatch` or `barrel`; it has `raft_planks`, `oak_log`, `birch_log` and `spruce_log`. Add new `dock_planks`, `dock_post`, `thatch`, `plank_step`, `drying_rack`, `net` and `barrel` blocks. Check the ids stay within the block-id limit the terrain crate uses. `plank_step` needs a game-side shape entry, not a `crates/terrain` change.
3. **Terrain crate.** Rule 3 holds: the coast is a new `DensityTerm`/bias in `crates/layout`. If a new bias needs a variant in `DensityTerm` (`crates/layout/src/lib.rs`) that is still layout code. `crates/terrain` stays untouched, so its `no_std` and zero-dependency invariants are not at risk.
4. **Exclusion shape.** Section 3.1 step 8 wants an exclusion over the footprint plus margin, but only `Circle` and `Corridor` exist. Suggest a union of circles (one per piece) or a new rectangle variant; either is in `crates/layout`. Decide in Phase C.
5. **Village record.** `Level` derives serde for the level file. A `village` record (pieces, doors, spawns) is computed from seed and level, not authored, so suggest keeping it out of the TOML and computing it alongside `build_route_layout`. Otherwise the level file format changes and `levels.toml` outputs need regenerating.
6. **World output changes.** The coast is a new density term on the right of every route, so for a fixed seed the terrain there differs from today. Existing seeds in `levels.toml` and their scores (Phase F of world-layout) were chosen without the coast; the coast may collide with extra islands the scoring counts, or fail the route checks for some. Phase B should re-run `mince-seeds` validation over the shipped levels and say what changes. Per CLAUDE.md, Phase B and C PRs must say world output changes and include before/after `stats`.
7. **Navigability and retries.** Section 3.0 requires the coast to stay clear of the channel and the eroded navigable set. Navigability counts any non-air block in the `SEA_LEVEL` to `SEA_LEVEL + 3` band, including leaves, so shoreline trees on the new coast must not hang over the channel. The 30 to 40 block default is well beyond the channel's 12 block half-width plus falloff, but validation of this stays in Phase B's done-when.
8. **Corridor extent.** "For the width of the corridor": the corridor is a region set (`corridor.rs`, margin 2), not a distance. Suggest the coast extends over a fixed distance (data) from the channel centre line, inside the corridor, and that its far edge fades out before the corridor's border so no region outside the corridor is needed.
9. **Region independence.** Rule 2 and Phase C hold if the village is computed once per level from seed, layout and level (the stated plan) and regions only stamp the cells inside them. Stilts and steps depend on the ground height under each piece, which comes from the generator, so computing them needs undecorated terrain sampled outside the region being generated. Suggest the village builder reads heights through the generator's height query (as `derived.rs` and the island checks do) and caches the result per level; Phase C's random-order test then covers it.
10. **Day and night.** Section 4.2 says "if a day and night cycle exists". `phase.rs` is the gathering/service clock, not a day/night cycle, so the "home at night" behaviour is not built in this doc. Treat as deferred, as the author already allows.
11. **Particles and events.** `happy_villager` is an art particle in `art/PROMPT.md`, and `client/particles.js` exists, but nothing draws a `happy_villager`. Phase F should add it with an input script. `villager_greeted` is a new `GameEvent` and must reach the client like the events in basic-gameplay section 11.
12. **Interaction rules.** Section 4.3 needs `entity:villager` rules and a no-damage primary that still emits `Swing`. `data/interactions.toml` only has `entity:animal` today, and the first-match rule means a villager rule must not be shadowed by a general `entity:*` rule. Check the `Swing` event is emitted whatever the target (the `events.rs` comment says it is).
13. **Villager art.** `art/PROMPT.md` describes the villager as "a sushi customer: darker skin, short gray hair, off-white robe with a deep-water sash". Section 4.1 wants several sash and hair colours. Suggest the extra colours are tints applied in the client, not extra textures, to keep the sheet small. Phase A's new block art goes through `art/tools` and `validate.py` (run from `art/`) as CLAUDE.md describes.
14. **Wasm size.** The terrain wasm is unchanged. `crates/layout` and `crates/game-web` grow; PRs report gzipped sizes.
15. **Performance.** Growing a village per level during `find_route` retries adds cost to every attempt. "Median attempts unchanged within 1" in Phase C is testable; a time limit is a `[manual check]`.
16. **`[manual check]`.** Phase B "tuned so the shore has some flat-ish stretches and some cliffs"; Phase D "debug mode outlines each village footprint, so seeds can be judged by eye"; Phase F "villagers are drawn in the browser scene". Measurable versions: Phase B, at least one column run of 8 or more shore columns with height spread of 1 or less, and one with a drop of 4 or more within 3 columns, over 100 seeds; Phase D, `preview.mjs` plans a route and the footprint overlay function returns one rectangle per level (client test); Phase F is covered by the input script the doc already asks for.
17. **No conflicting docs.** `world-layout.md` (routes, validation 6.1), `basic-gameplay.md` (entities, rules, shapes), `raft-scene-feedback.md` (box models) and `picking-plants.md` (particles) are built on, not contradicted. The doc's mention of "world-layout Level and Phase E; #631" matches the code. Section 3.2 adds rows to world-layout 6.1; this doc does not edit that one, so the Phase C ticket should add the village checks to `data/validation.toml` and note it in the world-layout current state through the docs updater.
18. **Phase A still incomplete (plan #1551).** Plan #1551's tickets are all closed: new blocks `thatch`, `barrel`, `plank_step`, `drying_rack` and `net` (#1583), their shapes (#1644) and art (#1591), the parser for `kind`, `connectors`, `spawns` and `rotation` (#1584), rotating connectors and spawns on `Structure` (#1593), and the `build.rs` loader for `data/structures/villages/` (#1657). Still missing: (a) `dock_planks` and `dock_post`, decided in #1559 but absent from `data/blocks.toml` and `art/`; (b) the piece templates themselves (plaza, dock, boardwalks, caps, decor, buildings), as `data/structures/villages/` does not exist; (c) the check that every connector lies on the piece's edge at floor height (`y = 1`, per #1558). The plan deferred (b) and (c) until #1558 and #1559 were answered; both are now answered, so they need a second plan.

---

# Villages and villagers

Status: idea
Scope: both (the mainland coast, village placement and building templates in the layout crate; villager entities and rendering in the game)

> **Reconciler:** the two header lines above are the author's; the front matter at the top of the file is now authoritative (`status: accepted`, `scope: both`).

The raft sails from anchorage to anchorage, one per week, with each level's island beside the channel on the left of the heading (world-layout, Level and Phase E; #631). The islands are where the crew gathers. Villagers don't live there: they live on the main landmass, a coast that runs along the other side of the route. This idea adds that coast, puts a small fishing village on it across the channel from each anchorage, and fills it with villagers. They are **inert** in this idea: they stand about, wander their village and turn to look at you. Ordering food, walking to the raft to eat, and working stations are later ideas that build on these entities.

There are no villages in Beta, so this is entirely Mince Raft's own design. Nothing here needs SPEC research. Every number lives in data and is tuned for feel.

---

## 1. Design rules

1. Villages are generated, not hand-placed: a village is a pure function of the seed, the layout and the level, like every other layout output.
2. A village never depends on the region it is generated in. A building straddling a region border appears whole and identical from either side (the determinism invariant).
3. `crates/terrain` does not learn about villages. Placement happens in `crates/layout`, through the same bias, exclusion and structure-stamping steps the raft already uses.
4. Every village is reachable: a dock out towards the anchorage, and a walkable path from the dock to every building door.
5. Villages never edit terrain. Buildings stand on stilts and reach the ground with steps, so the land under a village is exactly what the generator made.
6. Villages are never on a level's island. The island stays the gathering ground.
7. Villagers cannot be hurt, pushed out of their village or carried. They are scenery with a pulse until later ideas give them jobs.

---

## 2. How a village is put together

A village is a tree of prebuilt pieces joined at connector points. It starts from one plaza by the water, raised boardwalks branch out from it, boardwalks lead to more boardwalks or to buildings, and every dead end is capped. Buildings are leaves: they hang off boardwalks and don't carry the network any further. Life is attached separately: plazas and buildings carry spawn markers for villagers, so no villager is baked into a template.

The whole village stands on stilts at one floor height (rule 5), so the boardwalks are the streets.

### 2.1 Piece kinds

| Kind | Role | Connectors | Joins to |
|---|---|---|---|
| Plaza | The root, one per village, at the head of the dock | 3 to 6 | the dock, boardwalks, decor, villager spawns |
| Dock | One per village, from the plaza out over the water towards the anchorage | 1 | nothing further |
| Boardwalk | Streets: straight, corner, tee, crossing, and a stepped run where the ground drops | 2 to 4 | boardwalks, buildings, decor, caps |
| Building | Leaves: homes, workplaces, food | usually 1, up to 3 (a back deck) | decor, villager spawns |
| Landing | A small secondary plaza among the buildings; not a root, and may continue the network | 2 to 4 | boardwalks, buildings, decor, villager spawns |
| Cap | Closes a dead-end boardwalk: steps down to the ground, or a small landing over the water | 1 | nothing |
| Decor | Lantern post, net rack, barrel stack | 1 | nothing |

A landing is its own piece kind, `landing`: a boardwalk-like node with its own decor and spawns, separate from the one plaza and from the building leaves. (Decided in #1784.)

### 2.2 Templates and connectors

Templates are data in `data/structures/villages/` in the `raft_start.toml` format (size, origin offset, palette, layers), plus:

- `kind`: one of the kinds above.
- `connectors`: a list of `{ cell, facing, pool }`. A connector is a cell on the piece's edge at floor height, the direction it faces, and the pool its neighbour is drawn from (`boardwalks`, `buildings`, `decor`, `caps`).
- `spawns`: a list of `{ cell, kind }` markers (`villager` for now), on the piece's floor.
- `rotation`: allowed rotations, so pieces can face any of four ways.

A template's floor is always its layer `y = 0`, and the walkable surface is the top of that layer's blocks. Every connector and spawn cell is at `y = 1`, the first air layer above the floor. Templates declare no floor height of their own. (Decided in #1558.)

Two pieces join when a connector of each sits on the same cell face, facing opposite ways, at the same floor height. Stilt and step runs are not in the templates; they are generated down to the ground under each piece (section 3.1).

### 2.3 Building set

A fishing village, with our own pieces:

- **Homes:** several small huts, a medium house and a longhouse.
- **Workplaces:** fisher hut, net loft, smokehouse, fish market stall, boat shed and cooperage.
- **Food:** kelp drying racks, a small garden plot and a chicken pen.
- **Other:** small landings among the buildings (secondary plazas, with their own decor and spawns) and a lookout.

About a third of the buildings should be homes; the rest are workplaces, food and landings. Workplaces are scenery in this idea. Later ideas can give them jobs.

### 2.4 Blocks

Templates use existing blocks where possible (`planks`, `log`) and add only what is missing:

| Block | Use | Art |
|---|---|---|
| `thatch` | Roofs | new: `thatch_top`, `thatch_side` |
| `dock_planks` | Dock and boardwalk surface | new |
| `dock_post` | Stilts and dock legs | new |
| `plank_step` | Steps | new; half-height collision from the game-side shape table (basic-gameplay 4.4) |
| `drying_rack` | Food piece and decor | new, non-full collision |
| `net` | Net loft and net racks | new, non-full collision |
| `barrel` | Cooperage and decor | new |

`dock_planks` and `dock_post` are new blocks with their own art, not `raft_planks` and `oak_log`. (Decided in #1559.)

---

## 3. Placement

### 3.0 The mainland coast

The route today has islands on its left and open water elsewhere. Add a mainland on its right:

- A new density bias along the route, on the right of the heading: land rising from the sea at a set distance from the channel centre line (data, default 30 to 40 blocks, beyond the channel's falloff), continuing for the width of the corridor. It follows the route's bends, so the coast is continuous from the first anchorage to the last.
- The coast's height and roughness are data, tuned so the shore has some flat-ish stretches and some cliffs. The generator's own terrain does the rest.
- The channel and its navigability are unchanged: the coast must stay outside the channel's `clear_above` and the eroded navigable set (world-layout 6.2), and validation checks it.
- Climate is unchanged, so the coast warms along the route like everything else (world-layout Phase G), and villages in the warm south sit in different biomes from those in the north.

### 3.1 Siting and growing

Per level, after the route biases and before decoration. A village is built once per level, as a whole, from the seed and the level, never per region; regions then stamp the cells that fall inside them.

1. **Pick a plaza site.** On the mainland coast, abreast of the level's anchorage, score shore stretches by how close they are to the anchorage across the channel and how gentle the ground is around them. Take the best, ties broken by a hash of the seed and level, not a running RNG.
2. **Set the floor.** The village floor is one height for the whole village: a set amount above the highest ground under the plaza (data, default 1 block), and never below the dock's deck height.
3. **Place the plaza and the dock.** The plaza, rotated so its dock connector faces the anchorage, then the dock out over the water.
4. **Grow.** Work outwards from the plaza, breadth-first. For each open connector, order its pool's pieces by weight using a hash of the seed, the level and the connector's path from the plaza, and place the first piece (and rotation) that fits. A piece fits when:
   - it overlaps no placed piece;
   - every cell is within the village bound of the plaza centre;
   - the ground under every floor cell is at or below the floor and no more than the stilt limit below it;
   - nothing is in the channel, the navigable set, the raft exclusion, the level's island or outside the corridor.
5. **Limits and caps.** Pieces chain at most a set depth from the plaza. When a boardwalk connector reaches that depth, or nothing in its pool fits, it falls back to the `caps` pool. A building or decor connector may also stay empty, by drawing the pool's "none" entry.
6. **Stilts and steps.** Generate stilts under every floor cell down to the ground, and steps wherever a cap or a stepped boardwalk meets the ground.
7. **Check.** A village needs at least a set number of homes (data). If it has fewer, try the next-best plaza site; if no site works, the level fails validation and the retry loop moves on, as for any other failed level.
8. **Exclude decoration.** Add an `Exclusion` over the footprint and a margin so trees don't grow through buildings. Grass and flowers under the stilts are fine.
9. **Record.** The level gains a `village` record: footprint, floor height, pieces, building doors, dock end and villager spawn markers.

### 3.1.1 Parameters

All of these are this project's own starting values, in a new `data/village.toml`, tuned for a small village squeezed between the channel and the land. They are not taken from any other game.

| Parameter | Default |
|---|---|
| Max depth from the plaza | 4 |
| Village bound from the plaza centre | 32 blocks |
| Stilt limit below the floor | 6 blocks |
| Minimum homes | 3 |

Pool weights:

| Pool | Entries |
|---|---|
| `boardwalks` | straight 40, corner 25, tee 25, crossing 10 |
| `buildings` | none 15, homes 35, workplaces 30, food 15, landings 5 |
| `decor` | none 30, lantern 30, net rack 20, barrel stack 20 |
| `caps` | steps down 60, water landing 40 |

More corners and tees than a grid would give, so the boardwalks wind along the shore.

### 3.2 Validation

Add to world-layout section 6.1:

- The dock end is in water at least 2 deep and within a set distance of the anchorage (default 20 blocks).
- A walkable path exists from the dock end to every building door (the player walk rules: step height, no gaps).
- The village has at least the minimum number of homes, and every boardwalk dead end is capped.
- No village cell intersects the channel, the navigable set or the raft exclusion; only stilts and dock posts may stand in water.
- No village cell is on the level's island.
- The coast exists on the right of every route segment and leaves the channel navigable.

### 3.3 Viewer

The viewer's map and 3D view show the coast and villages, and a debug mode outlines each village footprint, so seeds can be judged by eye.

---

## 4. Villagers

### 4.1 Entities

- Each village spawns one villager per spawn marker in its record when the level loads (homes carry one or two markers, landings and the plaza one each). They are a new entity kind in `crates/game` alongside animals, with their own store and targeting (`entity:villager`).
- Villagers have the player's box size and collide with blocks and with the player (a gentle push apart, never off a ledge).
- Villagers stay off the dock and the raft. Coming to the raft to eat is for the serving idea.
- Each villager gets a home (the nearest home to its marker) and a look drawn from a hash of the seed, level and index: a few robe sash colours and hair colours, so a crowd is not identical.

### 4.2 Behaviour (inert)

- **Idle:** stand still for a random time, sometimes turning to face the boardwalk or the sea.
- **Wander:** walk to a random point on the plaza, a boardwalk or a building floor, using the same walk physics as the player and never stepping off the village, into water or onto the dock.
- **Look at the player:** within a set distance (default 6 blocks), the head turns to follow the player; the body doesn't.
- **Home at night:** if a day and night cycle exists, villagers walk to their home's doorway and stand inside until morning. Until then they ignore time.

Behaviour uses the sim RNG, so a replay gives the same villager positions.

### 4.3 Interaction

- The dispatch table gets rules for `entity:villager`: primary does nothing (no damage, no knockback, but still a `swing`), interact plays the `happy_villager` particle and emits `villager_greeted`. Nothing else.
- Villagers never pick up, carry or drop items in this idea.

### 4.4 Rendering

Villagers render as box models from the existing art (`villager` in `art/PROMPT.md`: head, nose, robe, folded arms, legs), using the textured box-model path from raft-scene-feedback. Legs swing while walking; the arms stay folded.

---

## 5. Phases

**A. Templates and blocks.** Section 2: the template format with `kind`, `connectors`, `spawns` and `rotation`; the piece set; the new blocks and art. Done when every template parses, its connectors and spawns rotate with it in all four directions, and every connector lies on the piece's edge at floor height.

**B. The mainland coast.** Section 3.0. Done when, over 100 random seeds, every route has continuous land on the right of every segment within the corridor, every consecutive anchorage pair is still navigable, and no coast column is in the channel.

**C. Placement.** Sections 3.1 and 3.2: siting, growing the piece tree, caps, stilts and steps, exclusion, the village record and validation. Done when no placed piece is deeper than the max depth or outside the bound, every join pairs two facing connectors at the same height, and, over 100 random seeds with a 5-level route, every level has a mainland village passing validation (after retries, median attempts unchanged from today within 1), no terrain block under a village differs from the same seed without villages, and generating regions in random order gives identical village blocks to row order.

**D. Viewer.** Section 3.3. Done when the viewer shows the village footprint mode and `preview.mjs` still plans a route.

**E. Villager entities.** Sections 4.1 to 4.3 in `crates/game`, with a headless test. Done when villagers spawn at the record's points, over 10,000 ticks none leaves its village, steps onto the dock or enters water, primary hits do nothing, and two runs with the same seed and input give identical villager positions.

**F. Villagers in the client.** Section 4.4. Done when villagers are drawn in the browser scene at the level's village and an input script can expect a villager near a position and a `villager_greeted` event on interact.

> **Reconciler:** Phases A to F have the right order and each ends in something testable. Phase A depends only on the layout parser; B is independent of A; C needs A and B; D needs C; E needs C (the village record); F needs E. Phase D's "village footprint mode" is `[manual check]` for the look, with the measurable version in note 16. Phase B's "continuous land on the right of every segment" needs a definition (suggest: every route segment has a land column, height above `SEA_LEVEL`, at each step along its right-hand normal between the near and far coast distance).

---

## 6. Decisions

1. Villages are not on the level's island; they are on the main landmass, a new coast along the right of the route (section 3.0).
2. No terrain edits: every piece stands on stilts, with steps to the ground (section 2, rule 5).
3. Island size is unchanged, since villages are not on the island.
4. Villagers can never be hurt (section 4.3).
5. Villagers stay off the dock and the raft until the serving idea.
6. A village is a tree of pieces joined at connectors, grown from a plaza, with this project's own pieces, weights and limits (section 2, section 3.1.1).
7. One village per anchorage, grown from a plaza across from it; there is no world grid for villages.
8. The piece set is a fishing village on stilts (section 2.3).
9. The mainland is always on the right of the heading, opposite the islands (section 3.0).

## Open questions

1. **Exclusion shape for the footprint (note 4).** A. Union of one circle per piece, no new variant. B. New rectangle variant in `Exclusion`. Suggest B if the Phase C ticket can stay within 200 lines, else A.
2. **Village record storage (note 5).** A. Computed alongside `build_route_layout`, not in the level file. B. Written into the level file by `mince-seeds`. Suggest A, so existing `levels.toml` files stay valid.
3. **Existing levels and seeds (note 6).** A. Re-search and regenerate `levels.toml` after Phase B. B. Keep the shipped seeds and retry only the failing ones. Suggest B first, A if more than a few fail.
5. **Villager look variation (note 13).** A. Tint the one villager texture in the client. B. Separate textures per look. Suggest A.
6. **Night behaviour (note 10).** A. Leave out until a day and night cycle exists. B. Tie it to the gathering/service phase clock. Suggest A, as the doc says.
