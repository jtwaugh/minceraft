# CLAUDE.md

Instructions for agents working in this repository, locally or in GitHub
Actions. Read this file, then `README.md`, before changing anything.

## What this is

The world generator for Mince Raft: a Beta-1.7.3-style terrain generator in
Rust (`crates/terrain`), compiled to wasm (`crates/web`), with a browser
viewer and flyover (`viewer/`) and a text-based pixel-art tileset (`art/`).
`SPEC.md` is the source of truth for how the generator behaves.

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
- **Never edit `SPEC.md` in an implementation change.** Spec changes come
  only from the maintainer, in PRs that change nothing else.
- **If the spec is ambiguous, incomplete or seems wrong, stop.** Open an
  issue labelled `spec-question` that quotes the section and states what's
  unclear, then link it from the PR (or leave the task undone). Don't fill
  gaps with your own knowledge of how Minecraft does it.
- Never add Mojang assets. Colours in `data/*.toml` and every texture in
  `art/` are this project's own.
- Exact seed parity with Beta is not a goal.

## Lanes

- **Spec lane (local only):** `SPEC.md`, `work/`. Maintainer only.
- **Implementation lane:** everything else. This is where agents work.

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

Keep PRs to one issue. Don't reformat or refactor code you weren't asked
to touch.

## Invariants

- **Determinism.** A block depends only on the seed and its world
  coordinate, never on the region origin or size. Overlapping regions must
  agree, with and without decoration. Any new feature needs a test for this.
- **`crates/terrain` stays `no_std` + `alloc` with zero runtime
  dependencies.** Build-dependencies (`toml` in `build.rs`) are fine.
- **Data lives in `data/*.toml`,** read at compile time by
  `crates/terrain/build.rs`. Add constants there, not as magic numbers.
- **Wasm size matters.** `[profile.release]` is size-optimised on purpose;
  only the `terrain` package gets `opt-level = 3`. Don't change the
  profiles without saying why.
- **Viewer has no frameworks or bundler.** Plain ES modules in `viewer/`.

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
