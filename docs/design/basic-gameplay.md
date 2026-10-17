---
title: Basic gameplay
status: accepted
scope: game
source: "#13"
superseded_by: ""
phases:
  1: done
  2: done
  3: done
  4: done
  5: done
  6: todo
  7: todo
  8: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **Phase 1 done (plan #59).** `crates/game` has the input bindings (`data/input.toml`, #68), `TickInput` with latched edges (#71), the fixed 20 Hz clock and free-look camera (#77); `crates/game-web` and `client/` add pointer lock, input sampling (#72), the debug camera scene and frame-time overlay with p99 reporting (#79). The test scene lives in `client/` (#67). Eye height, bobbing, deck roll and hand rendering are left to later phases.
- **Phase 2 done (plan #87).** `crates/game` has the collision boxes and world (#119), the player physics data file `data/player.toml` (#117), swept AABB resolution (#121), step-up (#123), the land movement tick (#124), fall distance and damage (#126), sneak edge protection (#127), the headless island walk test `crates/game/tests/island_walk.rs` (#132) and the walking player in the browser test scene (#134). Partial-height shapes live in the game-side table (#116) and the physics numbers are the game's own (#115).
- **Phase 3 done (plan #137).** `crates/game` has the water tunables and body-in-water detection (#152), the water movement tick and swim up (#154), climbing out (#155), the raft entity with collision and carrying the player (#156, `crates/game/src/raft.rs`, `data/raft.toml`), the swell push on the deck (#163), and the headless tests `crates/game/tests/swim_climb.rs` (#159) and `crates/game/tests/raft_stability.rs` (#164). The browser test scene has swimming and a rocking raft at `client/?scene=raft` (#166). Movement events (4.8) belong to Phase 8.
- **Phase 4 done (plan #173).** `crates/game` has the `Target` type, ray-vs-box test and `data/targeting.toml` (#187), block selection boxes and the block raycast (#190, shapes decided in #174), entity targets with the nearest-hit rule (#193), the per-tick eye ray on the walking player (#196), `data/interactions.toml` and its loader (#186), first-match rule resolution with stub actions (#197), and the selection outline and rule overlay in `client/` (#200, `client/overlay.js`). Held tool and hands state are Phase 5.
- **Phase 5 done (plan #213).** `crates/game` has `data/items.toml` and the belt and hands in `PlayerState` (#228, `items.rs`, `player.rs`), tool selection and the derived held thing (#229), item entities with the world store and targeting (#230, `item_entities.rs`), item gravity and settling (#232), pickup with tools stowed while carrying (#233), Drop throwing the item (#239), interact-while-carrying pinned as a no-op per #215 (#238), the headless test `crates/game/tests/hands.rs` (#244) and the belt in the browser test scene with input scripts, e.g. `client/test/scripts/belt.input` (#249). Put-down arrives with stations.
- **No gameplay code beyond Phase 5.** There is no animal, crop, event or HUD code in this repo. `crates/terrain` is the
  generator; `crates/web/src/lib.rs` exports only generation, height, climate
  and biome functions.
- **Layout crate.** `crates/layout` (world-layout Phase A: bias terms,
  channels, regions) is the only game-side crate. It holds no player, input
  or sim code.
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
  It lists no sword and no animal textures. `art/batches_1_6.json` draws all
  of the listed ones (`hoe`, `seeds`, `wheat_0`, `rice_0`, `farmland_dry`,
  `farmland_wet`, `planter_side`, `heart_full`, `heart_half`); no sword or
  animal texture exists in `art/batches_*.json`.
- **Spawn lists.** SPEC 14 gives Beta's creature spawn lists (sheep, pig,
  chicken, cow, wolf). It says nothing about hit points, knockback or flee
  behaviour.
- **Determinism.** The generator already guarantees a block depends only on
  seed and world coordinate (CLAUDE.md), which the doc's replay test relies on.

Not present: `MECHANICS_SPEC.md`, animals, crops, the event system and the HUD.
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
   `inset`. Decided in #88: see section 4.4.
6. **Blocks and art needed from this repo.** `farmland` (dry, wet) and crop
   stage blocks are not in `data/blocks.toml`, although their textures are
   now drawn (`art/batches_1_6.json`). No sword texture is listed. The raft and station
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
11. **Phase status.** Phases 1 to 5 are done; the rest are not.

## Mince Raft: Basic Gameplay Specification

Status: draft
Scope: player input, camera, movement (land, water, raft deck), the interaction model, and the first two tools: sword and hoe.

Companion to `WORLD_LAYOUT_SPEC.md`. The physics numbers in this document are this game's own values, not claims about Beta, and no Beta parity is intended. They live in a data file and are tunable, and the tests check against those numbers (where a test says "the mechanics spec", it means these values). The **(verify)** marks on them no longer mean "check against Beta"; they only flag numbers likely to be retuned for feel. Everything tunable lives in data files so feel can be adjusted without code changes. (Decided in #90.)

---

## 1. Design rules

1. The simulation runs at a fixed 20 ticks per second. Rendering interpolates between the last two ticks.
2. Players cannot break or place blocks. They can transform a small set of blocks through tools (grass to farmland, crop stage resets) and repair raft cells.
3. Movement should feel like Beta: no sprinting, no attack cooldown, sneak prevents walking off edges.
4. The player has a **tool belt** (a short hotbar of tools) and **hands** that carry at most one item, Overcooked style. There is no inventory grid.
5. Every player action goes through one interaction dispatch table, so new tools and items are data, not new code paths.
6. The sim is deterministic: the same seed and input sequence produce the same state.

---

## 2. Input

### 2.1 Bindings (desktop)

| Action | Default | Edge or hold |
|---|---|---|
| Move | W A S D | hold |
| Look | mouse (pointer lock) | continuous |
| Jump / swim up | Space | hold |
| Sneak | Left Shift | hold |
| Primary (attack) | Left mouse | press edge |
| Secondary (use) | Right mouse | press edge, plus hold for repeat where noted |
| Interact (pick up, put down, use station) | E | press edge |
| Drop carried item | Q | press edge |
| Select tool | 1 to 5, mouse wheel | press edge |

Bindings live in `data/input.toml`. Touch and gamepad mappings come later and map onto the same `TickInput`.

### 2.2 Per-tick input

The platform layer samples devices every frame and accumulates them into one record per sim tick:

```rust
struct TickInput {
    move_forward: f32,   // -1..1
    move_strafe: f32,    // -1..1
    yaw: f32,            // absolute, radians
    pitch: f32,          // absolute, clamped to +/- 90 degrees
    jump: bool,          // held
    sneak: bool,         // held
    primary: bool,       // pressed since last tick
    secondary: bool,     // pressed since last tick
    secondary_held: bool,
    interact: bool,      // pressed since last tick
    drop: bool,          // pressed since last tick
    tool_slot: Option<u8>,
}
```

Rules:
- Press edges are latched: a click between two ticks is never lost, and one physical click triggers exactly one action.
- Look is applied every frame for smooth camera motion, but the sim reads the value at the tick boundary. This keeps camera feel responsive while keeping the sim deterministic and replayable.
- Recorded input sequences are the basis for replay tests.

---

## 3. Camera

| Parameter | Default | Notes |
|---|---|---|
| Field of view | 70 degrees | This project's default, not a claim about Beta. (Decided in #61.) |
| Eye height | 1.62 blocks | 1.62 minus about 0.08 while sneaking **(verify)** |
| View bobbing | on | Driven by distance walked, disabled in water and in air |
| Mouse sensitivity | data | Linear sensitivity set in data; no Beta parity claimed. (Decided in #61.) |
| Deck roll | data | Camera rolls with raft tilt, scaled down (default 50 percent) to reduce motion sickness |

A first-person hand renders the current tool or carried item, with Beta's swing animation on primary and a smaller use animation on secondary.

---

## 4. Movement

### 4.1 Player body

| Parameter | Value |
|---|---|
| Bounding box | 0.6 wide, 1.8 tall |
| Step height | 0.5 |
| Max health | 20 (10 hearts) |

### 4.2 Land movement per tick

Order of operations each tick, following Beta's entity update:

1. Read input. Scale forward and strafe by 0.98. If sneaking, scale them by 0.3 **(verify)**.
2. Choose acceleration:
   - On ground: `accel = 0.1 * (0.16277136 / f^3)` where `f = slipperiness * 0.91` and slipperiness is 0.6 for normal blocks. This works out to 0.1 on normal ground **(verify)**.
   - In air: `accel = 0.02`.
3. Add acceleration to horizontal velocity in the facing direction, normalized so diagonal input is not faster.
4. If jump is held and on ground, set vertical velocity to 0.42.
5. Move with collision (section 4.4).
6. Apply gravity: `vy = (vy - 0.08) * 0.98`.
7. Apply horizontal friction: multiply by `f` on ground (0.546 on normal blocks), 0.91 in air.

Expected results (use as tests): walking about 4.3 blocks per second, sneaking about 1.3, jump apex about 1.25 blocks, about 12 ticks in the air on flat ground **(verify all)**.

### 4.3 Water movement

When the player's body intersects water:

- Acceleration 0.02 in all cases.
- Velocity multiplied by 0.8 each tick on all axes, then gravity of 0.02 subtracted from vertical velocity.
- Holding jump adds 0.04 to vertical velocity each tick (swim up).
- Exiting water: if the player is pushing into a block face and the space 0.6 above would be clear, set vertical velocity to 0.3 so they climb out onto the raft or shore **(verify)**.

Values from the mechanics spec override these. No drowning in v0; add an air meter later if diving becomes a mechanic.

### 4.4 Collision

- Collect block collision boxes overlapping the swept bounding box.
- Resolve vertical first, then x, then z, clipping movement against each box per axis (Beta's order).
- If horizontal movement was blocked and the player is on ground, retry with the box raised by step height. Keep the stepped result only if it moved farther horizontally.
- `on_ground` is true when vertical movement was clipped while moving downward.
- Plants (tall grass, flowers, crops) have no collision.
- Partial-height collision shapes (slabs, half blocks) live in a game-side shape table in `crates/game`. `data/blocks.toml` and `crates/terrain` are unchanged, and test slabs are defined in the game's own test world. (Decided in #88.)

### 4.5 Sneak edge protection

While sneaking on ground, before moving, shrink horizontal motion in small steps (0.05 blocks) on each axis until the moved box would still have ground under it. The player cannot walk off a ledge or off the raft deck while sneaking.

### 4.6 Falling

Track fall distance while airborne, reset on landing or entering water. On landing, damage is `ceil(fall_distance - 3)` if positive. Falling into water does no damage.

### 4.7 Standing on the raft

The raft is a moving rigid body. Phase 3 adds a minimal raft entity in `crates/game` that owns the raft mask, anchors at the anchorage, and each tick produces a `RaftFrame { displacement: [f64; 3], yaw_delta: f64 (radians), tilt: [f64; 2] (degrees about the x and z axes) }` from a data-driven swell model (amplitude, period and direction in `data/raft.toml`). Raft travel and steering are not part of Phase 3. (Decided in #139.) For any entity whose `on_ground` support comes from a raft cell:

- Before the entity's own movement, translate it by the raft's displacement this tick, and rotate it around the raft's center by the raft's yaw change.
- The raft's collision boxes are its mask cells, translated by the raft's displacement and rotated in 90-degree steps only, so they stay axis-aligned for the collision resolver of 4.4. A carried entity gets the same quarter turns around the raft's center. (Decided in #140.)
- Deck tilt from swells does not move the player directly. Instead, it adds a small horizontal push down-slope each tick (data, default 0.004 per degree of tilt), so standing still on a rocking raft means drifting slowly unless you sneak. Sneaking cancels the push. The sign of `tilt` is the direction of the push: a positive `tilt[0]` pushes towards +z and a positive `tilt[1]` pushes towards +x, each axis separately (push on z is `0.004 * tilt[0]`, push on x is `0.004 * tilt[1]`). (Decided in #158.)
- Loose item entities on the deck get the same push at double strength, so dropped items slide.

### 4.8 Movement events

Emit `landed { fall_distance }`, `entered_water`, `exited_water`, and `stepped_onto_raft` / `stepped_off_raft` for sound, particles, and gameplay hooks.

---

## 5. Targeting

Each tick, cast a ray from the eye along the look direction:

- Block reach: 4.5 blocks **(verify)**. Entity reach: 3 blocks **(verify)**.
- Test entity bounding boxes (expanded by 0.1) and block selection boxes. The nearest hit wins.
- Plants have selection boxes even without collision boxes, so they can be targeted. Every non-air block, liquids included, has the unit-cube selection box (the full cell), except where the block's collision box is smaller than the cell, in which case that box is used. Only air is never targeted, so a ray stops on a water surface and can target water. (Decided in #174.)
- Result: `Target::Block { pos, face }`, `Target::Entity { id }`, or `Target::None`.
- Render a selection outline on the targeted block and a subtle highlight on the targeted entity.

---

## 6. Hands and tool belt

```rust
struct PlayerState {
    tool_belt: [Option<ToolInstance>; 5],
    selected: u8,
    hands: Option<ItemInstance>,   // carried ingredient, dish, or plate
    // ... physics, health, etc.
}
```

Rules:
- If the hands are empty, primary and secondary use the selected tool.
- If the hands hold an item, tools are stowed. Primary does nothing, secondary uses the carried item if it has a use (for example, seeds on farmland), and interact does nothing unless a station or counter is targeted; only Drop releases the item, and put-down arrives with stations. This keeps the Overcooked rule that you can't swing a sword while carrying a fish. (Decided in #215.)
- Interact on a loose item entity picks it up if the hands are empty. Interact on a station or counter places or takes items (stations are specified later).
- Drop throws the carried item forward as an item entity. Drop velocity and the item entity's numbers (throw speed, upward kick, gravity, drag, box size) are the game's own, not Beta's: they live in `data/items.toml` and are tuned to feel right, as with player physics. (Decided in #214.)
- Walking over an item entity does not auto-pick it up. Pickup is always deliberate, so the deck doesn't vacuum up ingredients.
- v0 starting belt: hand (empty slot), wooden sword, wooden hoe. Fishing rod and bucket slots are reserved.

---

## 7. Interaction dispatch

Every action resolves through one table keyed on what is held, which button, and what is targeted:

```toml
# data/interactions.toml

[[rule]]
held = "tool:sword"
button = "primary"
target = "entity:animal"
action = "attack"

[[rule]]
held = "tool:hoe"
button = "secondary"
target = "block:grass"
require_air_above = true
action = "till"

[[rule]]
held = "tool:hoe"
button = "secondary"
target = "block:dirt"
require_air_above = true
action = "till"

[[rule]]
held = "tool:hoe"
button = "secondary"
target = "block:planter_dirt"
action = "till"

[[rule]]
held = "item:seeds"
button = "secondary"
target = "block:farmland"
target_face = "top"
require_air_above = true
action = "plant"

[[rule]]
held = "empty"
button = "secondary"
target = "block:crop"
require_state = "mature"
action = "harvest"

[[rule]]
held = "any_tool"
button = "secondary"
target = "block:crop"
require_state = "mature"
action = "harvest"

[[rule]]
held = "empty"
button = "secondary"
target = "block:tall_grass"
action = "forage"

[[rule]]
held = "any"
button = "primary"
target = "entity:animal"
action = "attack"     # fallback: punch
```

Resolution: the first matching rule wins, so specific rules go above general ones. Each action is a function in code: `attack`, `till`, `plant`, `harvest`, `forage`. Adding a tool means adding item data and rules, plus a new action function only if the behavior is new.

Every action that succeeds triggers the hand swing (primary) or use animation (secondary), emits an event, and applies tool durability if enabled.

---

## 8. Sword

### 8.1 Attack

- Triggered on the primary press edge. No cooldown; damage does not depend on timing (pre-1.9 combat).
- Holding primary does not auto-attack. One click, one swing.
- Damage by tool, in half-hearts:

| Weapon | Damage |
|---|---|
| Hand | 1 |
| Wooden sword | 4 **(verify)** |
| Stone sword | 6 **(verify)** |
| Iron sword | 8 **(verify)** |

- On hit: apply damage, knockback, and hurt state as in section 8.2.
- Swinging at a block or at nothing plays the swing animation and does nothing else. Swords never damage blocks.

### 8.2 Hurt and knockback

From the mechanics spec's living-entity rules:
- Invulnerability window after a hit (about 10 ticks of effective immunity) **(verify)**. Hits during it are ignored unless they deal more damage than the last hit, in which case only the difference applies.
- Knockback: horizontal push away from the attacker of about 0.4, plus a small upward push **(verify)**.
- Red flash for the duration of the hurt time.
- On death: death animation (tip over, about 20 ticks), then drops spawn and the entity is removed.
- Animals respond to being hit by fleeing in a random direction for a few seconds. Beta animals only wander, so this is a Mince Raft addition; keep the flee speed modest so chasing a cow around an island stays funny rather than frustrating.
- The animal numbers in this section and the next (box size per kind, wander speed and pause lengths, flee speed and duration, knockback, upward kick, invulnerability ticks, death ticks) are the game's own, not Beta's: they live in `data/animals.toml` and are tuned to feel right, as with player physics. The **(verify)** marks on them do not mean they need research. (Decided in #254.)

### 8.3 Animals and drops

Beta 1.7.3 cows drop only leather and chickens only feathers; raw beef and raw chicken came later. Mince Raft defines its own drops:

```toml
# data/animals.toml

[cow]
health = 10
drops = [{ item = "raw_beef", min = 1, max = 3 }, { item = "leather", min = 0, max = 1 }]

[pig]
health = 10
drops = [{ item = "raw_pork", min = 1, max = 3 }]

[chicken]
health = 4
drops = [{ item = "raw_chicken", min = 1, max = 1 }, { item = "feather", min = 0, max = 2 }]
```

Expected: a wooden sword kills a cow or pig in 3 hits and a chicken in 1.

Drops spawn as item entities. Their scatter velocity is the game's own, in `data/animals.toml`, tuned to feel right. Since pickup is deliberate (section 6), the player carries drops one at a time. That makes hunting trips a real cost in a two-minute gathering phase, which is intended. (Decided in #254.)

### 8.4 Animal respawn

Island animal counts are topped up at the start of each day (not continuously) to the level's target count, spawning on valid grass tiles out of the player's view.

---

## 9. Hoe

### 9.1 Till

- Triggered on secondary press. Holding secondary repeats every 4 ticks on newly targeted blocks so a row can be tilled by sweeping the mouse.
- Grass or dirt with air above becomes farmland. Planter dirt becomes planter farmland.
- Tilling is a block transform, allowed under the no-breaking rule.
- Tilling grass has a 1 in 8 chance to drop seeds (data). This is Mince Raft's own value; check whether Beta's tilling had a seed drop and match it if so.

### 9.2 Plant

- With seeds in hands, secondary on the top face of farmland with air above places a stage-0 crop and consumes the seed.
- One seed item plants one crop. Carrying seeds in hands means planting a row takes several trips to the crate, so seed bags (one item, multiple plantings) are a likely early upgrade.

### 9.3 Growth

From the mechanics spec's crop rules:
- Crops advance stages on random ticks with a chance that depends on hydration and neighboring crops.
- Farmland is hydrated if water is within 4 blocks horizontally at the same level or one below. Raft planters sit at deck level next to the sea, so they are always hydrated unless a level disables it.
- Trampling is disabled.
- Crop growth must be scaled for a 4-minute day. Target: a crop planted Monday is ready by Wednesday's gathering phase. Keep Beta's relative odds and scale the per-tick chance in data.

### 9.4 Harvest

- Secondary on a mature crop, with empty hands or any tool selected, drops its produce and resets the crop to stage 0. No seed is needed to replant, since breaking is not allowed and replanting by hand would add a chore without a decision.
- Harvest yields come from `data/crops.toml`, for example wheat gives 1 wheat and 0 to 2 seeds.
- Harvesting an immature crop does nothing.

### 9.5 Forage

- Secondary on tall grass with empty hands has a chance to drop seeds (data, default 1 in 6). The tall grass stays.
- Each tall grass block has a per-block cooldown (default one day) so foraging can't be spammed.

---

## 10. Durability

Off by default in v0. When enabled, swords lose 1 per hit on an entity and hoes lose 1 per till. Values per material from the mechanics spec. A broken tool vanishes from the belt and a replacement can be bought over the weekend.

---

## 11. Events

The sim emits these for the game layer (orders, scoring, sound, particles, tutorials):

| Event | Fields |
|---|---|
| `swing` | player, tool |
| `entity_hurt` | entity, attacker, damage |
| `entity_killed` | entity, kind, attacker |
| `item_dropped` | item, position, source |
| `item_picked_up` | item, player |
| `item_put_down` | item, target |
| `block_tilled` | position, from, to |
| `crop_planted` | position, crop |
| `crop_harvested` | position, crop, yields |
| `foraged` | position, yields |
| `landed`, `entered_water`, `exited_water` | player, details |

---

## 12. HUD (minimum)

- Crosshair.
- Tool belt with selected slot.
- Hands indicator showing the carried item.
- Health hearts.
- Day phase timer (gathering or service, seconds left).

---

## 13. Implementation phases

Each phase ends with something playable in the browser test scene.

> **Maintainer:** the browser test scene is a separate web client folder
> (for example `client/`) with its own page, sharing `viewer/`'s meshing and
> textures. (Decided in #60.)

**1. Input and camera.** Pointer lock, bindings, `TickInput` accumulation with latched edges, free-look camera with FOV and interpolation. Done when looking and moving a debug camera feels smooth at 60 fps with the sim at 20 Hz.

**2. Land movement and collision.** Sections 4.1, 4.2, 4.4, 4.5, 4.6. Done when the movement tests pass and the player can walk around the island, step up half blocks, jump onto one-block ledges, and sneak to the edge without falling.

**3. Water and raft.** Sections 4.3 and 4.7. Done when the player can swim, climb back onto the raft, stay on a moving raft, and feel the deck push during swells.

**4. Targeting and dispatch.** Section 5 and the dispatch table from section 7 with stub actions. Done when the selection outline tracks blocks, plants, and animals correctly, and a debug overlay shows which rule matched.

**5. Hands and tool belt.** Section 6. Done when tools can be selected, item entities picked up and dropped, and tools are stowed while carrying.

**6. Sword and animals.** Section 8. Done when cows, pigs, and chickens wander, take hits with knockback and flashes, flee, die, and drop items that can be carried to a crate.

**7. Hoe and crops.** Section 9. Done when grass and planters can be tilled, seeds planted, crops grow over a few in-game days, and mature crops harvested and reset.

**8. HUD and events.** Sections 11 and 12. Done when every action produces its event and the HUD reflects state.

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

3. **Missing sections 2.2 to 11.**
   - A: paste them into this file so the planner can read them.
   - B: keep them in the game repo; this doc stays a summary.
