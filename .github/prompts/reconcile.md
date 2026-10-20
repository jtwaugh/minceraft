# Reconciler instructions

You reconcile a design doc against the repository. Read `CLAUDE.md` and
`docs/design/README.md` first; the README defines the doc format and
lifecycle you must follow. The workflow that started you says which mode
you are in.

## What you may change

Only one file: the design doc under `docs/design/`. Never touch code,
SPEC.md, CLAUDE.md or any other file.

Inside the doc you may:

- write or update the header;
- write or update the **Current state** and **Reconciliation notes**
  sections;
- add `> **Reconciler:** ...` notes beside the author's text;
- tag items `[external]` or `[manual check]`;
- set phase status to `done` when the code shows the phase is done;
- add phases to a doc that has none (see step 9 below);
- add questions to **Open questions**.

You may not delete, reword or reorder the author's text. If you think
something the author wrote is wrong, say so in a note beside it.

## How to reconcile

1. Read the idea or doc in full.
2. Read the code it touches. For every concept the doc relies on (a type,
   hook, data file, convention, algorithm), find whether it exists, under
   what name, and where. Cite file paths. Check `README.md`,
   `crates/terrain/src/`, `crates/web/src/`, `data/`, `viewer/`.
3. Check SPEC.md's headings and any section the doc touches. Where the
   doc states how Beta behaves, check SPEC.md says the same. If it
   doesn't, add a reconciliation note; never add Beta facts yourself.
4. Check the other docs in `docs/design/` for overlap or conflict.
5. Check CLAUDE.md's invariants (determinism, `crates/terrain` stays
   `no_std` with no runtime dependencies, data in `data/*.toml`, wasm size,
   no viewer frameworks). Anything the doc needs that would break one goes
   in Reconciliation notes with the smallest way to avoid it (for example
   a new crate for code that needs std or dependencies).
6. Everything is built in this repo, generator and game alike. Set the
   header's `scope` to the part the doc is about (`generator`, `game` or
   `both`), and tag `[external]` only what lives outside the repo
   (hosting, app stores, third-party services).
7. Find "done when" criteria that no test could check (visual judgments,
   "fast on a laptop") and tag them `[manual check]`, with a note
   proposing a measurable version if there is one.
8. Turn the doc's own open questions, and any conflict you can't resolve
   from the code, into entries under **Open questions**, with options.
9. Make sure the doc can be planned. The planner only plans phases, so a
   doc with none is never built. If the author's text has no phases, add
   `phases:` to the header (`A`, `B`, ...) and, after the author's text,
   a `## Phases (reconciler)` section with one `**Phase A.** ...`
   paragraph per phase: which of the author's items it covers and when
   it's done. Order them smallest and most independent first, each ending
   with something testable in this repo, and put items that wait on an
   open question after the ones that don't. Keep the author's text as it
   is.

Write **Current state** as a short list a reader can verify: what exists,
where, and which parts of the doc it already satisfies. Write
**Reconciliation notes** as a list of specific mismatches, each with what
the doc says, what the code does, and what you suggest. Be brief.

## Opening the pull request

- New docs, and docs that were `idea`, get `status: accepted`: the open PR
  is the proposal, and merging it is acceptance, so the file must already
  say `accepted` when it lands. Existing docs keep their status unless
  every phase in this repo is now done, in which case set `done`.
- File name: `docs/design/<short-kebab-name>.md`.
- One PR, changing only that file. Title: `Design: <title> (<status>)`.
- PR body: three to six bullets summarising what you found (already done,
  conflicts, game scope, open questions), then `Closes #<issue>` if you
  were started from an idea issue.
- If, in drift or phase-done mode, nothing in the doc needs to change,
  open no PR.
