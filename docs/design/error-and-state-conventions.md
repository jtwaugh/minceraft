---
title: Consistent error reporting and state machine conventions
status: accepted
scope: both
source: "#302"
superseded_by: ""
phases:
  A: todo
  B: todo
  C: todo
  D: todo
  E: todo
---

## Current state

Verified against the code on `master`:

- **Hand-written error types.** Each has its own `Display` and
  `impl std::error::Error`, with no shared helper:
  - `crates/game`: `PhysicsError` (`physics.rs`), `AnimalsError`
    (`animals.rs`), `ItemsError` (`items.rs`), `TargetingError`
    (`targeting.rs`), `BindingsError` (`bindings.rs`), `InteractionsError`
    (`interactions.rs`), `ItemEntityError` (`item_entities.rs`),
    `CropsError` (`crops.rs`), and `ForageError(pub String)` (`forage.rs`).
  - `crates/layout`: `StructureError` (`structure_error.rs`),
    `LevelFileError` (`level.rs`), `RetryError` (`retry.rs`, a struct), and
    `CheckResult` (`validation.rs`, `Display` only).
- **Data-file variants differ.** `AnimalsError`: `Parse`, `BadEntry`,
  `UnknownItem`. `TargetingError`: `Parse`, `Missing`, `Invalid`.
  `CropsError`: `Parse`, `BadEntry`, `UnknownItem`.
- **Prefixes are hand-written** in each `Display`
  (`"targeting.toml: ..."`, `"items.toml: forage: ..."`).
- **`StructureError`'s `Display` is `write!(f, "{self:?}")`**
  (`structure_error.rs:37`).
- **Derives differ.** `TargetingError` is `#[derive(Debug)]` only
  (`targeting.rs:192`); `CropsError` and `StructureError` derive
  `Clone, Debug, PartialEq, Eq`.
- **Tests.** 14 files under `crates/` use `unwrap_err` or `is_err()`.
- **Client state machine.** `client/controller.js` keeps
  `state.flying` (fly/walk) and the pointer-lock state; the DSL in
  `client/test/dsl.mjs` drives it with DOM events and `expect` lines.
- Nothing in this doc is built yet.

## Reconciliation notes

- **`bindings::Mode` is not a state machine.** The issue calls it "the only
  explicit" one. `Mode` (`bindings.rs:67`) is `Hold | Edge | Continuous`,
  describing how an action's input is read, with no transitions. Phase E
  should not start there. Suggestion: the first real targets are the
  client's fly/walk and pointer-lock handling (`client/controller.js`),
  which the DSL already covers, and any per-frame state in `raft.rs`,
  `stream.rs` and `interactions.rs`, which Phase A must audit first. I found
  no explicit state enum in those files.
- **Count.** The issue says 10 enums; the code has 9 in `game` (counting
  `ForageError`) and 4 in `layout`, of which two are not enums. Phase A's
  table settles the list.
- **`crates/terrain` invariant.** It stays `no_std` with no runtime
  dependencies. None of the error types above are in it, so the shared type
  belongs in a std crate. `structure_error.rs` is included by path from
  `crates/layout/build.rs:25`, so it must not import a shared type from
  another module. Suggestion: give `StructureError` a plain `Display` and
  leave it out of any shared-type conversion.
- **Where shared code lives.** `game` and `layout` are separate crates.
  Suggestion: put a data-file error helper in `crates/game` first, and move
  it to a small shared crate only if `layout` needs the same type. Don't
  make `game` depend on `layout` or the reverse for this.
- **Wasm size.** `game-web` ships to the browser (the issue quotes about
  156 KB gzipped; I did not rebuild to check). Phase B's ticket must report
  the gzipped size before and after, per `CLAUDE.md`. See the first open
  question.
- **World output.** No change. Determinism tests are unaffected.
- **Test helpers.** Test-only helpers must sit behind `#[cfg(test)]` or in
  a dev-only module so they don't reach the wasm build.
- **Other docs.** No overlap with `world-layout.md`, `basic-gameplay.md`,
  `biome-flora.md` or `viewer-rendering.md` beyond touching their error
  types.
- **`[manual check]`:** none; every criterion below is testable.

## Idea (author's text)

Requested by @jtwaugh on PR #300: a plan for cosmetic fixes to the error
system, with a codebase-wide audit of error reporting and of state machine
architecture. Errors and tests should share code where they can. Nothing
here changes world output.

### Audit so far (first pass, not exhaustive)

**Error types.** There are 10 hand-written error enums, each with its own
`Display` and `impl Error`, and no shared helper or convention:

- `crates/game`: `PhysicsError`, `AnimalsError`, `ItemsError`,
  `TargetingError`, `BindingsError`, `InteractionsError`, `ItemEntityError`,
  and `CropsError` (from #300).
- `crates/layout`: `StructureError`, `LevelFileError`, plus `RetryError` (a
  struct) and `CheckResult` (Display only).

**Inconsistencies seen:**

- Data-file errors use different variant sets. `AnimalsError` has `Parse`,
  `BadEntry` and `UnknownItem`. `TargetingError` has `Parse`, `Missing` and
  `Invalid`. `CropsError` has `Parse`, `BadEntry` and a `check_items` error.
  The same kind of mistake gets a different variant depending on the file.
- Error prefixes are hand-written strings such as `"animals.toml: ..."` in
  every `Display`.
- `StructureError`'s `Display` is `{self:?}`, so it prints Rust debug output.
- In `CropsError`, a bad `seeds_item` is reported as `Parse`, and
  `check_items` reports it with `crop: "seeds_item"` (the review nits on
  #300).
- `TargetingError` doesn't derive `Clone`, `PartialEq` or `Eq`, unlike most
  of the others, so tests can't compare it directly.

> **Reconciler:** `CropsError` now has a separate `UnknownItem` variant, so
> the `check_items` half of the `seeds_item` nit may already be fixed.
> Phase C should re-check `crops.rs` before changing it.

**Tests.** About 40 call sites use `unwrap_err`, `is_err()` or
`matches!(.., Err(..))` across 12 files. Each test builds its own check by
hand, and few assert on the message text.

> **Reconciler:** a count of files with `unwrap_err` or `is_err()` gives 14.

**State machines.** The only explicit one in Rust is `bindings::Mode`. The
client's input state machine is exercised through the DSL in
`client/test/dsl.mjs`. Other per-frame logic (raft, stream, interactions)
keeps its state in ad-hoc fields. This still needs an audit: which of these
are state machines in practice, whether they share a transition-and-error
shape, and whether the invalid-transition errors read the same way.

> **Reconciler:** see the first reconciliation note: `Mode` has no
> transitions.

### Proposed scope

Each item should be its own ticket within the 200-line limit.

#### Phase A: Audit

A ticket that produces a written table, no code: list every error type, its
variants, its `Display` format, and its derives. List every state machine
and how an invalid transition is reported.

> **Reconciler:** a table in a doc doesn't count toward the 200 lines, but
> "no code" means the output should be a file under `docs/`. The implementer
> may not edit `docs/design/`; suggest `docs/errors-and-states.md`.

#### Phase B: Shared data-file error

One small type, or a helper, for "file X, entry Y, problem Z" and "unknown
item". The `*Error` enums for TOML loaders use it, and the prefix comes from
one place. Keep `crates/terrain` `no_std` and dependency-free, and keep
`structure_error.rs` free of dependencies because `build.rs` includes it by
path.

> **Reconciler:** depends on the first open question. If the answer is
> conventions only, this phase shrinks to a `file_prefix` constant per
> loader.

#### Phase C: Display cleanup

Replace the `{self:?}` `Display`s, make derives consistent, and fix the
`CropsError` nits.

#### Phase D: Shared test helpers

Assertion helpers, for example `assert_err_contains` and `assert_bad_entry`,
used by the error tests and the loader tests, so errors and tests build on
the same code.

#### Phase E: State machine conventions

A short convention for naming states, reporting invalid transitions, and
testing them. Apply it to `bindings::Mode` first. Extend the client DSL if it
can't express invalid transitions.

> **Reconciler:** replace `bindings::Mode` as the first target; see the
> first reconciliation note. Write the convention into `CLAUDE.md`
> only through the maintainer, since the implementer may not touch it
> casually.

## Open questions

1. **Shared error type or conventions only?** Is a shared error type worth
   the wasm size?
   - A: a shared type in `crates/game` (reported size delta in the PR).
   - B: conventions plus test helpers only; each enum keeps its own
     `Display`, with a documented variant set.
   - C: a shared type, but only if Phase B's measured gzipped growth of
     `game-web` is under a threshold you set.
2. **Naming and errors only, or restructure?** Should Phase E stop at
   conventions and tests, or also restructure existing code?
   - A: conventions and tests only.
   - B: also convert code that Phase A finds to be a de facto state machine
     into an explicit enum, one ticket each.
3. **First target for Phase E**, since `bindings::Mode` has no
   transitions:
   - A: client fly/walk and pointer lock (`client/controller.js`).
   - B: whichever Rust state Phase A finds.
   - C: skip Phase E if Phase A finds no real state machines.
