# CLAUDE.md

Instructions for agents working in this repository, locally or in GitHub
Actions. Read this file, then `README.md`, before changing anything.

## What this is

Mince Raft: the game and its world. This repo holds both:

- the **world generator**, a Beta-1.7.3-style terrain generator in Rust
  (`crates/terrain`), compiled to wasm (`crates/web`), with a browser viewer
  and flyover (`viewer/`) and a text-based pixel-art tileset (`art/`);
- the **game** built on top of it (layout, simulation, client), designed in
  `docs/design/` and added crate by crate as the design docs are built.

`SPEC.md` is the source of truth for how Beta behaves, and so for the
generator and any Beta mechanic the game reuses. Design docs are the source
of truth for everything that is this project's own.

## Clean-room rules (do not break these)

This project is a clean-room reimplementation. Breaking these rules can
contaminate the codebase in a way that can't be undone.

- **Implement from `SPEC.md` only.** Never consult, search for, paste or
  paraphrase Minecraft source code, decompiled or otherwise, from any
  source: this repo, the web, or memory. If you recognise original code
  structure or obfuscated names, don't use them.
- **Never read or touch `work/` or any `*.jar`.** They exist only on the
  maintainer's machine, are gitignored, and must never be committed,
  copied, quoted or summarised.
  Exception: on the maintainer's machine, when the maintainer asks for a
  spec-lane research task, an agent may read `work/` and the jar. It
  reports facts and numbers only (no code, names or code structure),
  writes nothing to the repo, and does no implementation work in the same
  session.
- **Never edit `SPEC.md` in an implementation change.** Spec changes are
  decided and merged by the maintainer, in PRs that change nothing else.
  The only agent that drafts them is the spec editor
  (`.github/workflows/claude-spec.yml`), which records the maintainer's
  `/decide` answers and never adds facts about the original game.
- **If the spec is ambiguous, incomplete or seems wrong, stop.** Open an
  issue labelled `spec-question` that quotes the section, states what's
  unclear and lists the options, plus `decision` (a project choice) or
  `research` (needs facts about the original), then link it from the PR
  (or leave the task undone). Don't fill
  gaps with your own knowledge of how Minecraft does it.
- Never add Mojang assets. Colours in `data/*.toml` and every texture in
  `art/` are this project's own.
- Exact seed parity with Beta is not a goal.

## Lanes

- **Spec lane:** `SPEC.md`. Research (anything needing `work/`) happens
  only on the maintainer's machine. Decisions on `decision` questions are
  drafted by the spec editor on GitHub and merged by the maintainer.
- **Design lane:** `docs/design/`. The maintainer's own designs built on
  top of the generator. Only the reconciler, the docs updater and the spec
  editor change these files, and only as `docs/design/README.md` describes.
- **Implementation lane:** everything else. This is where agents work.

## Design docs

`docs/design/README.md` defines the format and lifecycle (idea, accepted,
done, superseded). When a ticket comes from a design doc:

- The doc says what to build; SPEC.md still says how Beta behaves. Never
  take Beta facts from a design doc.
- Read the doc's Current state and Reconciliation notes before starting,
  and check them against the code.
- Game code is built here like everything else. Don't build anything tagged
  `[external]`; it lives outside this repo (hosting, app stores, third-party
  services).
- Don't edit the design doc in an implementation PR. The docs updater
  marks phases done after the plan's tickets close.

## The loop

1. Ideas arrive as issues labelled `idea`. The reconciler
   (`claude-idea.yml`) opens a PR adding `docs/design/<name>.md` marked
   `accepted`; merging the PR is the acceptance.
2. The planner (`claude-plan.yml`) turns a SPEC section, or a phase of an
   accepted design doc, into `planned` tickets and `spec-question` issues,
   attached as sub-issues of a "Plan:" issue. It runs by itself when SPEC.md
   or a design doc changes on `master`: changed SPEC sections, or a design
   doc's next `todo` phase (one phase at a time, in header order).
3. The maintainer labels tickets `factory`; the implementer (`claude.yml`)
   opens a PR; CI and the reviewer (`claude-review.yml`) check it.
4. The maintainer answers `decision` questions with `/decide <option>`; the
   spec editor (`claude-spec.yml`) opens a PR changing only the file the
   question is about.
5. When a plan's last ticket closes, the docs updater (`claude-docs.yml`)
   closes the plan and marks the design-doc phase done. Every Monday it
   re-reconciles accepted docs against the code.

## Commands

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                                   # terrain unit tests
./build.sh                                   # textures + wasm + wasm-opt, prints size
./build.sh --test                            # cargo test first
(cd art && python3 -I tools/validate.py)     # must print "ok: N ..." with N > 0
(cd art && python3 -I tools/build_sheet.py)  # art preview -> art/build/sheet.html
node viewer/flyover/preview.mjs 12345        # headless flyover plan (needs ./build.sh first)
node --test client/test/*.test.mjs           # client tests (needs ./build.sh first)
cargo run -p terrain --profile bench-native --example stats -- 12345   # timing, biome and vegetation stats
```

After changing Rust, rebuild the wasm before trusting anything the viewer
or `preview.mjs` shows; `viewer/pkg/` is stale until you do.

## Before opening a PR

1. `cargo fmt`, clippy and `cargo test` pass. CI runs clippy with
   `-D warnings`; the lints allowed on purpose are listed in `Cargo.toml`.
   Don't add `#[allow]`s to get a PR green unless the spec requires the
   flagged code (see the `3.14159` in `trees.rs`).
2. `./build.sh` succeeds. Put the gzipped wasm size, before and after, in
   the PR description if it changed.
3. If you touched `art/`, `validate.py` (run from `art/`) prints `ok` with a
   nonzero count. From any other directory it finds nothing and still says `ok`.
4. If you touched the viewer or flyover, `preview.mjs` still plans a route.
5. The PR description cites the `SPEC.md` sections the change implements
   (e.g. "SPEC 11.5 Big oak").
6. **Say whether world output changes.** If any block, height, biome or
   decoration can differ for a fixed seed and coordinate, say so, say why,
   and include before/after `stats` output.
7. **Stay within 200 reviewable lines.**
   `python3 .github/scripts/pr_size.py origin/master HEAD` exits 0. Tests,
   docs and generated files don't count, so never trim tests to fit. If the
   ticket can't fit, build a coherent first part that passes every check,
   open a follow-up `planned` ticket for the rest (the planner's ticket
   format, with `**Depends on:** #<this ticket>`) with
   `python3 .github/scripts/planner_issue.py ticket --title <t> --body-file <f>`,
   which refuses a header the factory can't read, and link it in the PR. CI fails larger PRs unless the maintainer labels them `oversize-ok`.

Keep PRs to one issue. Don't reformat or refactor code you weren't asked
to touch.

## Invariants

- **Determinism.** A block depends only on the seed and its world
  coordinate, never on the region origin or size. Overlapping regions must
  agree, with and without decoration. Any new feature needs a test for this.
- **`crates/terrain` stays `no_std` + `alloc` with zero runtime
  dependencies.** Build-dependencies (`toml` in `build.rs`) are fine. Other
  crates (layout, game) may use `std` and dependencies; keep whatever ships
  to the browser small, and report wasm sizes in PRs.
- **Data lives in `data/*.toml`,** read at compile time by
  `crates/terrain/build.rs`. Add constants there, not as magic numbers.
- **Wasm size matters.** `[profile.release]` is size-optimised on purpose;
  only the `terrain` package gets `opt-level = 3`. Don't change the
  profiles without saying why.
- **Viewer has no frameworks or bundler.** Plain ES modules in `viewer/`.

## Testing the client

Check input and camera behaviour from memory, not from pixels. Don't use
screenshots (browser automation or headless captures) to find out whether
something moves, turns, locks or switches mode; write an input script
instead.

- `client/controller.js` is the per-frame sim step, with no WebGL. Keep
  render-loop logic there, not in `scene.js`, so it can be tested.
- `client/test/scripts/*.input` are event-sequence scripts in the DSL
  documented at the top of `client/test/dsl.mjs`: DOM events in, frames
  advanced on the real wasm, `expect` lines on mode, look, eye and
  movement. `client/test/scripts.test.mjs` runs every case.
- A client bug report gets a failing script first, then the fix. If the
  DSL can't express the behaviour, extend the DSL; don't fall back to
  screenshots.
- Screenshots are for judging how things look (art, shading, fog), and
  only when the maintainer asks for that.

## Art

- `art/PROMPT.md` is the art brief and the list of what should exist. Read
  it before drawing anything.
- One 16-colour palette, `.` for transparent, 8×8 unless the batch's
  `sizes` says otherwise, `snake_case` names unique across all batches.
- `art/batches_7_12.json` is generated: edit `art/tools/draw_7_12.py` and
  rerun it from `art/` (`cd art && python3 -I tools/draw_7_12.py`). Never
  hand-edit the JSON; CI checks it matches the script.

## Generated and ignored files

Don't commit `target/`, `viewer/pkg/`, `viewer/textures.json`, `art/build/`,
`work/`, `*.jar` or `.vercel/`. Don't touch `Cargo.lock` unless you changed
dependencies.

## Further reading

- `viewer/flyover/README.md`: how the flyover works and the pitfalls already
  found (read "Things that took a while to find out" before changing it).
- `art/README.md`: texture format and tools.
