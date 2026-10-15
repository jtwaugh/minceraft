---
title: Basic gameplay
status: accepted
scope: game
source: "#13"
superseded_by: ""
phases:
  1: todo
  2: todo
  3: todo
  4: todo
  5: todo
  6: todo
  7: todo
  8: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **No gameplay code.** There is no sim, player, input, camera, collision,
  entity, item, tool, animal or crop code in this repo. `crates/terrain` is the
  generator; `crates/web/src/lib.rs` exports only generation, height, climate
  and biome functions.
- **Viewer.** `viewer/` has map tiles, a 3D view and the flyover
  (`viewer/flyover/`). `viewer/flyover/player.js` is the flyover *video*
  player (camera clock and chunk loading), not a game player. No pointer lock,
  bindings or tick loop exist.
- **Block data.** `data/blocks.toml` has a `collision` field
  (`none | full | inset`) per block; this is the only collision data. Blocks
  present: terrain, water, ice, logs, leaves, plants, `sugar_cane`, `cactus`,
  `pumpkin`, `snow_layer`. Not present: `farmland`, crop-stage blocks,
  `planter`, raft blocks, slabs or half blocks.
- **Textures.** `art/PROMPT.md` lists `hoe`, `seeds`, `wheat_0..3`, `rice_0..3`,
  `farmland_dry`, `farmland_wet`, `planter_side`, `heart_full`, `heart_half`.
  It lists no sword and no animal textures. Check `art/batches_*.json` for
  what is actually drawn.
- **Spawn lists.** SPEC 14 gives Beta's creature spawn lists (sheep, pig,
  chicken, cow, wolf). It says nothing about hit points, knockback or flee
  behaviour.
- **Determinism.** The generator already guarantees a block depends only on
  seed and world coordinate (CLAUDE.md), which the doc's replay test relies on.

Not present: `MECHANICS_SPEC.md`, `TickInput`, the dispatch table, the tool
belt, hands, item entities, the event system and the HUD.
`WORLD_LAYOUT_SPEC.md` is `docs/design/world-layout.md`.

## Reconciliation notes

1. **Almost all of this is game scope.** Input, camera, movement, targeting,
   hands, tools, animals, crops, HUD and events are the sim and client.
   > **Maintainer, 2026-10-15:** game code now lives in this repo (see
   > CLAUDE.md, "What this is"), so these phases are planned and built here.
   > The `[game]` tags below just mark game code. This supersedes the
   > answer to #30.
2. **The idea issue is incomplete.** The text jumps from section 2.1 to
   section 12; sections 2.2 to 11 (look, movement, targeting, hands, sword,
   hoe, dispatch table, events) are missing. Phases 2 to 8 and the tests refer
   to sections 4.1 to 4.7, 5, 6, 7, 8, 9 and 11. They are not reconstructed
   here. The maintainer should add them (open question 3).
3. **No source for physics values.** The doc takes physics from Beta via
   `MECHANICS_SPEC.md`, which is not in the repo, and `SPEC.md` has no player
   physics (gravity, jump, step height, swimming, sneak edge, knockback,
   invulnerability). Per CLAUDE.md, those Beta facts reach `SPEC.md` only
   through the maintainer's research. Open question 4.
4. **Beta claims in the doc.** "No sprinting, no attack cooldown, sneak
   prevents walking off edges", the wooden-sword hit counts and the
   invulnerability window are not in `SPEC.md`. Treat them as the game's design
   targets, not Beta facts, until they are.
5. **Collision data.** Step-up on half blocks and slab-height edges needs
   partial-height collision. `blocks.toml` supports only `none`, `full` and
   `inset`. Open question 2.
6. **Blocks and art needed from this repo.** `farmland` (dry, wet) and crop
   stage blocks are not in `data/blocks.toml`, although textures for them are
   listed in `art/PROMPT.md`. No sword texture is listed. The raft and station
   blocks are already requested by `world-layout.md` (its note 8). The tilling
   and growth logic is `[game]`.
7. **Rule 2 versus the world.** "Players cannot break or place blocks", yet the
   hoe turns grass into farmland and resets crops. That is a game-side world
   overlay on top of `generate_decorated`; `terrain` must not learn about it
   (determinism invariant). The test "hoe never affects other blocks" should
   target the overlay.
8. **Rendering and size.** The game renderer is not this repo's viewer
   (CLAUDE.md: no frameworks in `viewer/`; wasm size matters). The selection
   outline, HUD and debug overlays belong to the game.
9. **Overlap with `world-layout.md`.** The raft mask, raft blocks and spawning
   the player on the deck are `[game]` there too. The raft deck at
   `y = SEA_LEVEL` (`lib.rs`, 64) agrees with the standing height assumed here.
   No conflict found.
10. **`[manual check]` criteria.** Phase 1 "feels smooth at 60 fps", Phase 2
    "can walk around the island", Phase 3 "feel the deck push" and the
    visible "ready" cue. Measurable versions: p99 frame time under 16.7 ms
    with the sim at 20 Hz for Phase 1; a scripted path crossing the island in
    a headless sim for Phase 2. The Section 14 tests are automatable.
11. **Phase status.** No phase is done.

## Mince Raft: Basic Gameplay Specification

> **Reconciler:** the text below is the author's, unchanged. Notes beside it
> are marked like this one. Sections 2.2 to 11 are absent from the idea issue
> (note 2).

Status: draft
Scope: player input, camera, movement (land, water, raft deck), the interaction model, and the first two tools: sword and hoe.

Companion to `WORLD_LAYOUT_SPEC.md`. Physics values come from Beta 1.7.3 via `MECHANICS_SPEC.md`. Numbers marked **(verify)** are from memory of Beta's behavior and must be checked against the extracted spec before they are treated as final. Everything tunable lives in data files so feel can be adjusted without code changes.

> **Reconciler:** `[game]`. `MECHANICS_SPEC.md` does not exist here (note 3).
> Game tunables would live in the game's data files, not in `data/*.toml`
> read by `terrain`.

---

## 1. Design rules

1. The simulation runs at a fixed 20 ticks per second. Rendering interpolates between the last two ticks.
2. Players cannot break or place blocks. They can transform a small set of blocks through tools (grass to farmland, crop stage resets) and repair raft cells.
3. Movement should feel like Beta: no sprinting, no attack cooldown, sneak prevents walking off edges.
4. The player has a **tool belt** (a short hotbar of tools) and **hands** that carry at most one item, Overcooked style. There is no inventory grid.
5. Every player action goes through one interaction dispatch table, so new tools and items are data, not new code paths.
6. The sim is deterministic: the same seed and input sequence produce the same state.

> **Reconciler:** `[game]`. Rule 2 needs a game-side world overlay (note 7).
> Rule 3 is a design target, not a SPEC fact (note 4).

---

## 2. Input

### 2.1 Bindings (desktop)

Action | Default | Edge or hold
-- | -- | --
Move | W A S D | hold
Look | mouse (pointer lock) | continuous
Jump / swim up | Space | hold
Sneak | Left Shift | hold
Primary (attack) | Left mouse | press edge
Secondary (use) | Right mouse | press edge, plus hold for repeat where noted
Interact (pick up, put down, use station) | E | press edge
Drop carried item | Q | press edge
Select tool | 1 to 5, mouse wheel | press edge

> **Reconciler:** `[game]`.

---

## 12. HUD (minimum)

- Crosshair.
- Tool belt with selected slot.
- Hands indicator showing the carried item.
- Health hearts.
- Day phase timer (gathering or service, seconds left).

> **Reconciler:** `[game]`. `heart_full` and `heart_half` textures are listed
> in `art/PROMPT.md`; the rest of the HUD art is not.

---

## 13. Implementation phases

Each phase ends with something playable in the browser test scene.

> **Reconciler:** no browser test scene exists in this repo; the only page is
> the viewer (`viewer/index.html`).
> **Maintainer, 2026-10-15:** the game lives in this repo, so the browser
> test scene is built here too, superseding the earlier answer to #30
> ("the game repo"). Where in the repo it goes is open question 1.

**1. Input and camera.** Pointer lock, bindings, `TickInput` accumulation with latched edges, free-look camera with FOV and interpolation. Done when looking and moving a debug camera feels smooth at 60 fps with the sim at 20 Hz.

> **Reconciler:** `[game]` `[manual check]` (note 10).

**2. Land movement and collision.** Sections 4.1, 4.2, 4.4, 4.5, 4.6. Done when the movement tests pass and the player can walk around the island, step up half blocks, jump onto one-block ledges, and sneak to the edge without falling.

> **Reconciler:** `[game]`. Sections 4.x are missing (note 2); half-block
> collision needs note 5.

**3. Water and raft.** Sections 4.3 and 4.7. Done when the player can swim, climb back onto the raft, stay on a moving raft, and feel the deck push during swells.

> **Reconciler:** `[game]` `[manual check]` for "feel the deck push".

**4. Targeting and dispatch.** Section 5 and the dispatch table from section 7 with stub actions. Done when the selection outline tracks blocks, plants, and animals correctly, and a debug overlay shows which rule matched.

> **Reconciler:** `[game]`. Sections 5 and 7 are missing.

**5. Hands and tool belt.** Section 6. Done when tools can be selected, item entities picked up and dropped, and tools are stowed while carrying.

> **Reconciler:** `[game]`. Section 6 is missing.

**6. Sword and animals.** Section 8. Done when cows, pigs, and chickens wander, take hits with knockback and flashes, flee, die, and drop items that can be carried to a crate.

> **Reconciler:** `[game]` `[manual check]` for "flashes". The `crate` block
> is requested by `world-layout.md`. Section 8 is missing.

**7. Hoe and crops.** Section 9. Done when grass and planters can be tilled, seeds planted, crops grow over a few in-game days, and mature crops harvested and reset.

> **Reconciler:** `[game]` for the logic; the repo-side part is the missing
> `farmland` and crop blocks and textures (note 6). Section 9 is missing.

**8. HUD and events.** Sections 11 and 12. Done when every action produces its event and the HUD reflects state.

> **Reconciler:** `[game]`. Section 11 is missing.

---

## 14. Tests

- Jump apex and airtime on flat ground match the mechanics spec within 1 percent.
- Walk and sneak speeds over 100 ticks match expected distances within 1 percent.
- No speed or angle lets the player pass through a block (fuzz test with random inputs against a walled box).
- Step-up succeeds on a slab-height edge and fails on a full block.
- Sneaking toward a ledge for 200 ticks never leaves the ledge.
- Swimming up from 5 blocks deep reaches the surface; climbing out onto a raft edge works from all four sides.
- A player standing still on a moving raft stays on it for 1,000 ticks with no swell; with swell and no sneak, drifts; with sneak, does not drift.
- One click produces one attack, even if the click lands between ticks.
- Wooden sword kills: cow in 3 hits, chicken in 1, with hits inside the invulnerability window ignored.
- Hoe tills grass and dirt only with air above; never affects other blocks.
- Seeds plant only on farmland tops; harvest works only on mature crops and resets to stage 0.
- Replay: a recorded 5,000-tick session with movement, combat, and farming produces an identical world hash on two runs.

> **Reconciler:** `[game]`. The first two depend on `MECHANICS_SPEC.md`
> values that are not available (note 3).

---

## 15. Open questions

- Should carried items slow the player down (heavier for whole fish, lighter for seeds)? It adds Overcooked texture but complicates movement tuning.
- Should the tool belt be fixed (one slot per tool type) or freeform? Fixed is simpler and more readable for a 20-minute game.
- Is the per-block forage cooldown visible to the player (tall grass looks trimmed), or invisible?
- Should animals fight back? A cow headbutt is funny but adds a combat layer the game may not need.
- Do crops need a visible "ready" cue beyond the final growth stage, given how fast the day moves?

> **Reconciler:** the author's questions are kept as written. Questions that
> affect this repo are below.

## Open questions (reconciler)

1. **Where the browser test scene lives in this repo.** (Reopened: #30
   chose a separate game repo, and the game now lives here instead.)
   - A: a new page in `viewer/` (plain ES modules, no bundler, like the
     viewer), with the sim compiled to wasm alongside the generator.
   - B: a separate web client folder (for example `client/`) with its own
     page, sharing `viewer/`'s meshing and textures.

2. **Partial-height collision data.**
   - A: a game-side shape table; `blocks.toml` unchanged.
   - B: add `collision` values (for example `"half"`) in `data/blocks.toml`
     and `build.rs`, so the game reads shapes from the generator's data.
3. **Missing sections 2.2 to 11.**
   - A: paste them into this file so the planner can read them.
   - B: keep them in the game repo; this doc stays a summary.
4. **Player physics facts.**
   - A: a `research` spec-question; the maintainer adds them to `SPEC.md`.
   - B: the game defines its own numbers and does not claim Beta parity.
