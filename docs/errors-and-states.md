# Errors and state machines: audit

Phase A of `docs/design/error-and-state-conventions.md`. Audit only; no code
changed. Checked against `master` (grep for every `impl fmt::Display` and
`impl std::error::Error` under `crates/`, plus a read of `raft.rs`,
`stream.rs`, `interactions.rs`, `walk.rs`, `client/controller.js` and
`client/input.js`).

## Final counts

- **13 hand-written error types**: 9 in `crates/game`, 4 in `crates/layout`.
  Nothing was missed by the design doc's list; `crates/terrain` and
  `crates/web` have none, and `crates/game-web` returns `JsError` (not
  hand-written).
- By shape: **10 enums** (8 in `game`, 2 in `layout`), **3 structs**
  (`ForageError`, `RetryError`, `CheckResult`). So the idea's "10 enums" is
  right for enums, but the full set is 13 types.
- **12 implement `std::error::Error`**; `CheckResult` is `Display` only (it
  is a check outcome, not an error).
- **8 are data-file loader errors** (`PhysicsError`, `AnimalsError`,
  `ItemsError`, `TargetingError`, `BindingsError`, `InteractionsError`,
  `ItemEntityError`, `CropsError`) plus `ForageError`, making **9**.
  `StructureError` is a template-parse error (embedded TOML, see below) and
  `LevelFileError` is a level-file error; they are not `data/*.toml` loaders.
- **State machines: 2 real ones, both in the client** (fly/walk, pointer
  lock). **0 in Rust.** `bindings::Mode` has no transitions.

## Error types

| Type | File | Kind | Variants | `Display` format | Derives | Data-file loader? |
|---|---|---|---|---|---|---|
| `PhysicsError` | `game/src/physics.rs:54` | enum | `Parse(String)`, `Missing(String)`, `Invalid(String)` | `player.toml: {e}`, `player.toml: missing \`{k}\``, `player.toml: {e}` | `Debug` | yes (`player.toml`) |
| `AnimalsError` | `game/src/animals.rs:58` | enum | `Parse(String)`, `BadEntry{kind, problem}`, `UnknownItem{kind, item}` | `animals.toml: {e}`, `animals.toml: {kind}: {problem}`, `animals.toml: {kind}: drop \`{item}\` is not in items.toml` | `Clone, Debug, PartialEq, Eq` | yes |
| `ItemsError` | `game/src/items.rs:33` | enum | `Parse(String)`, `BadEntry{kind, index, problem}`, `Duplicate{kind, name}`, `UnknownHeld{held}`, `UnknownName{kind, name}` | `items.toml: {e}`, `items.toml: {kind} {index}: {problem}`, `items.toml: duplicate {kind} \`{name}\``, `interactions.toml: \`{held}\` is not in items.toml`, `unknown {kind} \`{name}\`` (no file prefix) | `Clone, Debug, PartialEq, Eq` | yes |
| `TargetingError` | `game/src/targeting.rs:193` | enum | `Parse(String)`, `Missing(String)`, `Invalid(String)` | `targeting.toml: {e}`, `targeting.toml: missing \`{k}\``, `targeting.toml: {e}` | `Debug` | yes |
| `BindingsError` | `game/src/bindings.rs:94` | enum | `Parse(String)`, `UnknownAction(String)`, `MissingAction(&'static str)`, `DuplicateKey{key, first, second}`, `Invalid(String)` | `input.toml: {e}`, `input.toml: unknown action \`{a}\``, `input.toml: missing action \`{a}\``, `input.toml: key \`{key}\` listed twice for action \`{first}\`` (when `first == second`) or `... bound to both \`{first}\` and \`{second}\``, `input.toml: {e}` | `Clone, Debug, PartialEq` | yes (`input.toml`) |
| `InteractionsError` | `game/src/interactions.rs:79` | enum | `Parse(String)`, `UnknownField{rule, field}`, `MissingField{rule, field}`, `WrongType{rule, field}`, `UnknownHeld{rule, value}`, `UnknownButton{..}`, `UnknownTarget{..}`, `UnknownAction{..}` | `interactions.toml: {e}`; others `interactions.toml: rule {rule}: unknown field \`{field}\``, `missing field`, `wrong type for`, `unknown held`/`button`/`target`/`action \`{value}\`` | `Clone, Debug, PartialEq, Eq` | yes |
| `ItemEntityError` | `game/src/item_entities.rs:39` | enum | `Parse(String)`, `Missing(String)`, `Invalid(String)` | `items.toml: {e}`, `items.toml: missing \`{k}\``, `items.toml: {e}` | `Debug, PartialEq, Eq` | yes (`items.toml`) |
| `CropsError` | `game/src/crops.rs:39` | enum | `Parse(String)`, `BadEntry{crop, problem}`, `UnknownItem{crop, item}` | `crops.toml: {e}`, `crops.toml: {crop}: {problem}`, `crops.toml: {crop}: \`{item}\` is not in items.toml` | `Clone, Debug, PartialEq, Eq` | yes |
| `ForageError` | `game/src/forage.rs:23` | tuple struct `(pub String)` | one message | `items.toml: forage: {msg}` | `Clone, Debug, PartialEq, Eq` | yes (`items.toml` `[forage]`) |
| `StructureError` | `layout/src/structure_error.rs:8` | enum | `Toml(String)`, `Field(&'static str)`, `BadPaletteKey(String)`, `UnknownBlock(String)`, `UnknownSymbol(char)`, `LayerCount{expected, found}`, `RowCount{layer, expected, found}`, `RowLength{layer, row, expected, found}` | `{self:?}` (Rust debug output, line 39) | `Clone, Debug, PartialEq, Eq` | no (structure templates; file is included by path from `build.rs`, so it must stay dependency-free) |
| `LevelFileError` | `layout/src/level.rs:40` | enum | `Parse(String)`, `SeaLevelMismatch{found}`, `DuplicateLevelId(String)`, `Write(String)` | `invalid level file: {m}`, `sea_level is {found} but must be {SEA_LEVEL}`, `duplicate level id \`{id}\``, `cannot write level file: {m}` | `Debug` | no (level files, read and written) |
| `RetryError` | `layout/src/retry.rs:26` | struct | fields `start_seed`, `attempts`, `last_seed`, `last_report` | `no valid level in {attempts} attempts from seed {start_seed} (last seed {last_seed}); failing checks: [{check}]...` | `Clone, Debug, PartialEq` | no |
| `CheckResult` | `layout/src/validation.rs:58` | struct | fields `name`, `measured`, `required`, `passed` | `{name}: {measured} (need {required}) pass` or `FAIL` | `Clone, Debug, PartialEq` | no; `Display` only, no `Error` impl |

### Data-file loader variant sets

| Loader error | Variant set |
|---|---|
| `AnimalsError`, `CropsError` | `Parse` / `BadEntry` / `UnknownItem` (`animals` keys on `kind`, `crops` on `crop`) |
| `TargetingError`, `PhysicsError`, `ItemEntityError` | `Parse` / `Missing` / `Invalid` |
| `ItemsError` | `Parse` / `BadEntry` / `Duplicate` / `UnknownHeld` / `UnknownName` (no `Missing`/`Invalid`/`UnknownItem`) |
| `BindingsError` | `Parse` / `UnknownAction` / `MissingAction` / `DuplicateKey` / `Invalid` |
| `InteractionsError` | `Parse` plus seven per-field variants, all carrying a 1-based `rule` |
| `ForageError` | no variants; one free-text string |

No two loaders share a full variant set. The same mistake maps differently:
an unknown item is `UnknownItem` (animals, crops), `UnknownHeld` /
`UnknownName` (items), or free text in `Invalid` / `ForageError`.
Prefixes are hand-written in every `Display` (`ItemsError::UnknownName` has
none, and `ItemsError::UnknownHeld` names `interactions.toml`, not
`items.toml`). `items.toml` is reported under three types (`ItemsError`,
`ItemEntityError`, `ForageError`) with the same `items.toml:` prefix.

### `CropsError` and `seeds_item` (as found in `crops.rs`)

- A `seeds_item` that is absent or not a string is
  `CropsError::Parse("`seeds_item` must be a string")` (`crops.rs:151-153`).
- A `seeds_item` that names an item `items.toml` does not define is
  `CropsError::UnknownItem { crop: "seeds_item", item }`, raised by
  `check_items` (`crops.rs:168-172`); the `crop` field holds the literal
  string `seeds_item` here, which is not a crop name.
- So the `check_items` half of the #300 nit is already fixed (it has its own
  variant); the remaining wart is that a wrong-typed `seeds_item` is `Parse`
  and the `crop` field is overloaded.

### Other observations

- Derives: `PhysicsError`, `TargetingError` and `LevelFileError` are
  `Debug` only; `ItemEntityError` has `Debug, PartialEq, Eq` but no `Clone`;
  `BindingsError`, `RetryError` and `CheckResult` have `PartialEq` but not
  `Eq`. Six types have the full `Clone, Debug, PartialEq, Eq`.
- Files using `unwrap_err` or `is_err()` under `crates/`: 14.

## State machines

| Machine | Where | States | Transitions | Invalid transition |
|---|---|---|---|---|
| Fly / walk | `client/controller.js` (`state.flying`) | walking, flying | `KeyF` keydown, only when `input.locked` and not `e.repeat`; teleports the other body to the current position (walk to fly: fly camera at the walk eye; fly to walk: walker at fly eye minus the eye height) | None possible: `F` while unlocked or repeating is silently ignored (early return). No error, no log. |
| Pointer lock | `client/input.js` (`locked`), read by `controller.js` | unlocked, locked | `click` on canvas while unlocked requests lock; `pointerlockchange` sets `locked = (document.pointerLockElement === canvas)` and clears held inputs on leaving; `window` `blur` also clears held inputs (state unchanged) | None possible: every other input handler returns early when unlocked, and `click` while locked is ignored. Browser owns the exit (Escape). |

### Checked and not state machines

- **`bindings::Mode`** (`bindings.rs:67`: `Hold | Edge | Continuous`) has no
  transitions. It is a per-action attribute fixed at load time describing how
  input is read.
- **`raft.rs`** (`Raft`, `Swell`, `SwellPush`): continuous values (`offset`,
  `yaw`, `turns`, tick clock). `yaw` accumulates and is converted into
  quarter `turns`; these are counters and no enum of modes exists.
- **`stream.rs`** (`Streamer.centre: Option<RegionCoord>`): `None` until the
  first update, then the region containing the feet; an update that does not
  change the region is a no-op. This is a cache cursor, not a machine; no
  errors.
- **`interactions.rs`** (`Dispatcher`): `last_rule` and `hits` are per-call
  results, reset or overwritten each dispatch. Rules are looked up per press
  edge; a press matching nothing sets `last_rule = None`. No states.
- **`walk.rs`** (`Walker`): `floating: Option<Floating>` and
  `streamer: Option<Streamer>` are optional components chosen at
  construction, and `target`/`state` are recomputed each tick. Nothing
  transitions between named modes.
- **`events.rs`** (`GameEvent`) is an event type, not a state.

Final count: **2 state machines (both client-side JS), 0 Rust.** In neither
is an invalid transition reported; both ignore it. Rust-side per-frame state
has no explicit state enum anywhere, so Phase E has no Rust target unless a
later feature introduces one.
