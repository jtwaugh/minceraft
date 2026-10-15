# Design docs

Design docs describe what we want to build on top of the generator: the
world layout, game-facing features, tools. They are the maintainer's own
ideas. `SPEC.md` is different: it is the clean-room description of how
Beta behaves, written only from research on the maintainer's machine.
Design docs refer to SPEC sections by number and never restate Beta
behaviour themselves.

## Lifecycle

| Status | Meaning | Who moves it on |
|---|---|---|
| `idea` | Captured, unchecked. Never planned. | The maintainer labels the issue `idea`. |
| `proposed` | Reconciled against the code, in a PR. | The reconciler opens the PR; the maintainer edits and merges it. |
| `accepted` | Merged. The planner may plan its phases. | Merging the reconciler's PR. |
| `done` | Every phase in this repo is done. | The docs updater, when the last phase's plan closes. |
| `superseded` | Replaced by another doc (say which in the header). | The maintainer. |

1. **Capture.** Open an issue, paste the idea as the body, label it `idea`.
   Or commit a file here with `status: idea` and run the reconciler on it
   (`gh workflow run claude-docs.yml -f path=docs/design/<name>.md`).
2. **Reconcile.** The reconciler (`claude-idea.yml`, or `claude-docs.yml`
   for a file) checks the idea against the code, SPEC.md, CLAUDE.md and the
   other docs here, and opens a PR adding `docs/design/<name>.md` with
   `status: proposed`. The instructions it follows are in
   `.github/prompts/reconcile.md`.
3. **Accept.** Review the PR, edit anything, merge. Merging is acceptance.
4. **Plan.** `gh workflow run claude-plan.yml -f file=docs/design/<name>.md -f section="Phase A"`.
   The planner only plans `accepted` docs, skips phases marked `done`, and
   skips anything marked as game scope. Open questions become `decision`
   issues, answered with `/decide`.
5. **Build.** Label tickets `factory`, as for SPEC work.
6. **Keep it true.** When a plan's last ticket closes, the docs updater
   (`claude-docs.yml`) closes the plan and opens a PR marking the phase
   `done`. Every Monday it also re-reconciles each `accepted` doc against
   the code and opens a PR for any that have drifted.

## Format

Every doc starts with this header. CI checks it.

```yaml
---
title: World layout
status: proposed          # idea | proposed | accepted | done | superseded
scope: both               # repo | game | both
source: "#12"             # the idea issue, if there was one
superseded_by: ""         # only when status is superseded
phases:                   # omit if the doc has no phases
  A: todo                 # todo | done
  B: todo
---
```

Then, in this order:

1. **Current state** (written by the reconciler, kept current by the docs
   updater): what already exists in the code that this doc relies on or
   overlaps, with file paths, and which parts are already done.
2. **Reconciliation notes** (reconciler): conflicts with the code,
   CLAUDE.md invariants, SPEC.md or other design docs; what is game scope
   rather than this repo; criteria that can't be tested automatically.
3. The author's text, phases and all. Agents add notes beside it as
   `> **Reconciler:** ...` blockquotes and tag items `[game]` or
   `[manual check]`, but never delete or reword the author's intent.
4. **Open questions**, each with options A, B, ... where possible.

## Scope

`scope: repo` work is planned and built here. `[game]` items (the sim,
renderer, player, raft entity, streaming in the game client) are listed so
the design reads whole, but nothing here plans or builds them.
