---
title: Play critic agent
status: accepted
scope: game
source: "#732"
superseded_by: ""
phases:
  A: done
  B: done
  C: done
  D: done
  E: todo
---

## Current state

Checked against the code on `master`:

- **Phase A done.** Plan #738: PR #752 (`createSession` and `exec` in `client/test/dsl.mjs`, `run` rebuilt on it) and PR #769 (session `transcript()` replays as an `.input` case); tests in `client/test/session.test.mjs`. The script-runner bullet below describes the code before these PRs.
- **Phase B done.** Plan #773: PR #779 (path classification and player/debug observations in `client/tools/observe.mjs`), PR #782 (`client/tools/play.mjs`, JSON lines over `createSession`) and PR #960 (player mode refuses debug paths, verify mode allows them); tests in `client/test/observe.test.mjs` and `client/test/play.test.mjs`. The "No critic tooling" bullet below predates these PRs for `client/tools/`.
- **Phase C done.** Plan #968: PR #985 (`hudLayout` crosshair, belt, hearts in `client/hudlayout.js`), PR #990 (timer and hands rectangles), PR #992 (`mountHud` places elements from `hudLayout`), PR #991 (overlap and off-screen checks in `client/hudcheck.js`), PR #1002 (scale, crosshair clear zone and distance checks), PR #1004 (`layout <w> <h>` statement in `client/test/dsl.mjs`) and PR #1005 (`play.mjs` returns layout results). The HUD and `layout` bullets below describe the code before these PRs.
- **Phase D done.** Plan #1007: PR #1021 (finding schema, `client/tools/finding.mjs`), PR #1025 (verify pass, `verify.mjs`), PR #1024 (grouped idea-issue report, `report.mjs`), PR #1023 (critic prompt, `.github/prompts/critic.md`), PR #1036 (local runner, `critic.mjs`) and PR #1037 (filing with duplicate search, `file.mjs`); tests in `client/test/` (`finding`, `verify`, `report`, `criticprompt`, `critic`, `file`). The "No critic tooling" bullet below predates these PRs.
- **Script runner (section 1).** `client/test/dsl.mjs` exports `parse(src)` and `run(c)`. `world()` builds the fake canvas, document and window, the sampler, `attachInput`, the wasm `Walk`/`FlyCamera`/`Scene` and `createController`. `step(w, {op, args})` runs one statement. `run(c)` builds the world and loops over a whole case; there is no way to keep a world open and feed it a line at a time. `client/test/scripts.test.mjs` runs every `client/test/scripts/*.input` case through `parse` and `run`.
- **State reads (section 2).** `read(w, path)` in `dsl.mjs` serves every path in the header comment: player-visible ones (`hearts.*`, `hud.selected`, `hud.hands`, `phase`, `phase.left`, `box`, `target.kind`, `console.last`, `console.lines`, `mode`, `locked`) and debug ones (`rule.primary|secondary|last`, `events`, `field.crop`, `planter.*`, `sim_tick`, `animals*`, `eye.*`). Nothing marks which are which.
- **HUD (section 3).** `client/hud.js` has `hudModel` (plain data), `hudScale` (`uiScale(w, h, 80, 45)`, integer scale), `digitNames`, `heartNames`, `slotView` and `mountHud`. `mountHud` lays out with CSS flex (`#hud-bottom`, `#hud-belt`, `#hud-hearts`, `#hud-timer`, fixed-pixel margins and gaps such as `bottom: 12px`, `gap: 2px`) and reads `view.innerWidth`/`innerHeight` only for the scale. `client/uipx.js` has `textureSize(set, name)`. There is no `hudLayout` and no `layout` command. `client/test/hud.test.mjs` tests the model only.
- **Menu (section 3).** `client/menu.js` has `menuModel(alpha, debug)`, `renderMenu` and `styleMenu`; also CSS-laid-out.
- **Debug console (open question 5).** `docs/design/debug-console.md` Phase 2 is done: `?debug=1` or a scene turns it on (`client/debug.js`), `Backquote` opens it, and `client/test/scripts/console.input` covers it. Opening it releases pointer lock; Enter re-requests it and Escape leaves the pointer unlocked, by design (debug-console, console mode section).
- **No critic tooling.** No `client/tools/`, no `.github/prompts/` file for a critic and no `claude-critic.yml`. Existing agent prompts: `.github/prompts/reconcile.md` only; the other workflows (`claude-plan.yml`, `claude-docs.yml`, `claude-spec.yml`, `claude-idea.yml`, `claude-review.yml`) hold their prompts inline.
- **Related issue #446** became `docs/design/raft-scene-feedback.md`, the model for the "idea issue" findings format.

## Reconciliation notes

1. **Section 1 is a small, safe refactor.** `world`, `step` and `run` already have the right seams: `createSession(startArgs)` can be `world(opts)` plus a `step` call per line, with `run(c)` rewritten on top. `start` is only legal as the first statement today (`run` reads `c.steps[0]`), so `createSession` must take the start flags as arguments, and `exec('start ...')` after creation should be an error. `exec` throws on a failed `expect`; the session should catch that and return it as a result instead.
2. **Transcript equals script (section 1).** Recording every line sent only gives a valid `.input` file if it includes the `case` and `start` lines and drops lines that errored. State this in Phase A's done-when.
3. **Player vs debug observation (section 2).** The split is a classification of `read()` paths, not new state. Keep it as one table in `dsl.mjs` (or a sibling module) so the harness and the critic's prompt can't drift apart. Some paths sit in between: `target.id`, `target.face` and `box` are visible only as the outline; `console.last` is only visible while the console is open; `phase.left` is visible only as digits.
4. **"Hands, timer" and the HUD tells.** The player observation can be built from `hudModel`'s output rather than `read()` paths, which makes it exactly what the HUD draws. Suggest that.
5. **`rule.*` and `events` stay debug.** basic-gameplay section 4's "debug overlay shows which rule matched" is a debug feature; the critic must never see it while playing. A harness-level check (the player tool refuses debug paths) is testable.
6. **Section 3 vs `mountHud` (open question 4).** basic-gameplay section 12 lists what the HUD shows, not how it is laid out, so replacing CSS flex with `hudLayout` is an implementation detail and needs no doc change. Positions the CSS now fixes (`bottom: 12px`, `top: 8px`, 2px and 4px gaps, 16px crosshair) must move into `hudLayout` as scaled constants (or `data/` if they should be tunable), or the layout and the pixels will differ. `hudLayout` must not import the DOM, in line with `uipx.js`, which imports nothing.
7. **Text in the HUD.** The timer's phase word and the hands' item name are DOM text (`font: 12px ui-monospace`), not pixel-art textures, so their width can't be computed exactly from `textureSize`. Those two labels are to be drawn from textures, so every `hudLayout` rectangle, the timer's and the hands' included, is exact. The art for them is a later ticket, not part of `hudLayout` itself. (Decided in #969.)
8. **Mechanical checks (section 3)** are measurable and testable as written (overlap, off-screen, `hudScale` below 2, crosshair clear zone, belt-to-hands distance). The thresholds (clear-zone radius, "far apart") are not given; they need numbers, ideally in a `data/*.toml` or constants with tests.
9. **Viewport sweep.** `uiScale` returns an integer scale from the viewport; phone portrait will likely hit scale 1, so the "scale below 2" check will fire there. Decided in #970: advisory on portrait viewports (section 3).
10. **Critic prompt and runner placement (open question 1).** Nothing in the repo is a model-driven tool other than the workflows, whose prompts live in `.github/prompts/` (reconcile) or inline. CLAUDE.md puts `.github/` and `client/` in the implementation lane, so either location is allowed; but `.github/workflows/` files can't be changed by the agent app, so Phase E's workflow file will need the maintainer. The prompt goes in `.github/prompts/critic.md` and the runner and verify code in `client/tools/`. (Decided in #1008.)
11. **Verify pass (section 5).** Deterministic replay is supported: the sim is a fixed 20 Hz tick with a deterministic RNG (basic-gameplay phase 6, `crates/game/src/rng.rs`), and `run` is already used that way. A repro that fails to replay, or whose `expect` lines don't hold, is dropped.
12. **Filing (section 5).** The critic opens `idea` issues, which trigger `claude-idea.yml`. That workflow needs the title and body in the shape the reconciler expects (a `# Mince Raft: <title>` heading, `Status: idea`, `Scope:` line). Rate: one grouped issue per run is already the proposal; also guard against duplicates of earlier findings (the critic should search open and closed `idea` issues first). Needs a token that can create issues; an agent-created issue labelled `idea` is the maintainer-controlled gate only if the label is added by the maintainer (the workflow triggers on the label, see `claude-idea.yml`).
13. **Clean room.** The critic judges against design docs and UX principles. Its prompt must repeat the CLAUDE.md rules: no Minecraft comparisons, no Beta facts outside SPEC.md, never touch `work/` or `*.jar`. The tool lives in the implementation lane and never edits `SPEC.md` or design docs.
14. **Invariants.** No change to `crates/terrain`, no new wasm in the terrain crate, no data that affects world output. `viewer/` is untouched. The play tool is Node-only (`client/tools/`), not shipped to the browser. `hudLayout` is browser-shipped JS and should stay small and framework-free.
15. **Testing rule.** CLAUDE.md says not to use screenshots to check behaviour; this doc's design (geometry, scripts) follows that. The "screenshot pass only when the maintainer asks" line matches CLAUDE.md.
16. **`[manual check]` items.** "Findings are useful", "the critic doesn't know the controls" (it is an agent's behaviour, not code). Measurable versions: the player observation contains none of the debug-only paths (test); the prompt contains no key names (grep test over `critic.md`); every finding in a run has a `repro` that replays (the verify pass itself).
17. **`[external]`.** None. The critic runs on Claude Code or the Agent SDK, which the existing workflows already use; scheduling is a GitHub workflow in this repo.
18. **Phases.** The author's suggested A to E are kept below. Phase C does not depend on A or B and could be planned in parallel, but the planner does one phase at a time in header order, so order is A, B, C, D, E.

## Idea

# Mince Raft: Play critic agent

Status: idea
Scope: game

An agent that plays the game through the headless memory-read harness and gives feedback on control flows and UI layout, with a replayable `.input` script for every finding. It plays the way a player would: it sees only what the game shows and doesn't know the controls in advance. Its findings arrive as `idea` issues like #446, so the reconciler decides what becomes design. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. A step-by-step play session

`client/test/dsl.mjs` already runs the real input layer, controller and wasm sim against a fake canvas/document/window, and reads state from memory. But it only runs whole scripts.

- Want: a session an agent can drive one statement at a time (`createSession(startArgs)` with `exec(line) -> result`). `scripts.test.mjs` runs on top of it with no behaviour change.
- Want: a tool (e.g. `client/tools/play.mjs`) speaking JSON lines over stdin/stdout: send DSL lines, get an observation back.
- Every line sent is recorded, so a session's transcript is a valid `.input` script.

> **Reconciler:** see notes 1 and 2. `world`, `step` and `run` in `client/test/dsl.mjs` are the seams.

## 2. Show only what a player can see

If the critic can read `rule.secondary`, raw `events` or `field.crop`, it can't tell that something is hard to discover.

- Want: the default observation holds only what a player could perceive: the HUD (hearts, belt, selected slot, hands, timer), crosshair target and whether it's outlined, console output, mode and pointer lock.
- Want: a separate debug observation (the full DSL paths and events), used only to verify findings and never while playing.
- Controls are not given to the critic in advance. If it can't find them from what the game shows, that is a finding.
- The debug console does not count as what the game shows. The critic plays without `?debug=1`, so the player observation never contains console output, and goals about the console are not tested by the critic. (Decided in #774.)

> **Reconciler:** see notes 3, 4 and 5. `[manual check]` "can't find the controls from what the game shows"; measurable version in note 16.

## 3. UI layout as geometry, not screenshots

The HUD's layout is done by CSS flex in `mountHud` (`client/hud.js`), so Node can't see it. CLAUDE.md says no screenshots to check behaviour.

- Want: a pure `hudLayout(model, set, viewport)` that returns a rectangle for each HUD element (crosshair, belt, hearts, hands, timer). Everything is integer-scaled pixel art (`hudScale`, `textureSize`), so this can be exact. `mountHud` then places elements at those rectangles.
- Want: a `layout <w> <h>` command that returns the boxes plus mechanical checks: overlaps, anything off-screen, scale below 2, crosshair clear zone, and how far apart related elements sit (belt vs hands). Scale below 2 is a finding on every viewport except a narrow one, where it is advisory: reported, but not counted as a finding. A viewport is narrow when it is portrait (height greater than width), so phone portrait is covered with no pixel threshold to tune. (Decided in #970.) The thresholds for the crosshair clear zone and the belt-to-hands distance are left to the implementer: constants in the check module, each with a test, to be tuned later. (Decided in #971.)
- The critic reviews layout over a sweep of viewports (phone portrait, 1366×768, 1920×1080, ultrawide). The menu (`menuModel`) may get the same treatment later.
- Screenshots stay a separate pass, only when the maintainer asks for one.

> **Reconciler:** see notes 6, 7, 8 and 9. The timer and hands labels are DOM text, so "exact" holds only for the texture-drawn elements unless the labels change.

## 4. The critic

- Goals come from the "done when" lines of accepted design docs, e.g. basic-gameplay phase 7 (till, plant, wait, pick on the raft scene), or opening and leaving the debug console.
- For each goal it reports: whether it finished, how many steps it took, where it got stuck or backtracked, states that trap the player (e.g. console open without pointer lock), and actions whose effect nothing visible reported.
- Findings use a fixed schema: `kind` (control-flow | layout), severity, goal, observation, suggestion, `repro` (`.input` text).
- Clean room: it judges against the design docs and general UX principles, and never against how Minecraft does things. Beta facts only from SPEC.md.

> **Reconciler:** see notes 13 and 16. Phase 7 of basic-gameplay is a real "done when" (section 13). The "console open without pointer lock" trap is specified in debug-console (Escape closes the console and leaves the pointer unlocked), so the critic may report it as designed; it should say so rather than treat it as a bug.

## 5. Verify, then file

- A second pass replays each `repro` deterministically with the debug observation and drops any finding that doesn't reproduce.
- What survives is grouped into one `idea` issue per run, in this format, filed without maintainer review after searching open and closed `idea` issues for duplicates. The `idea` label that starts the reconciler stays maintainer-only, which keeps a gate. The critic never edits code or design docs. A confirmed repro is attached to the issue only; it is not added to `client/test/scripts/` as an expected-failing case. (Decided in #1011 and #1010.)
- Runs locally (`claude -p` or the Agent SDK) and later as a workflow (e.g. `claude-critic.yml`), weekly and after client PRs merge.

> **Reconciler:** see notes 10, 11 and 12. A workflow file is outside what the agent app can edit.

---

## Phases (suggested)

- A: session refactor (1)
- B: play tool with player and debug observations (1, 2)
- C: `hudLayout` and the `layout` command (3)
- D: critic prompt, runner, verify pass and filing (4, 5)
- E: scheduled workflow

> **Reconciler:** phases kept as written. Each ends with something testable in this repo: A when `scripts.test.mjs` passes unchanged on `createSession` and a recorded transcript replays as a case; B when a test feeds lines through `play.mjs` and the player observation contains no debug-only path; C when `hudLayout` has unit tests over the viewport sweep and `mountHud` places elements from it; D when a fixture finding's `repro` replays and a non-reproducing one is dropped; E when `claude-critic.yml` exists and runs on schedule (the maintainer must add the workflow file).

## Open questions

4. Does `hudLayout` replacing CSS flex in `mountHud` need a basic-gameplay section 12 change, or is it an implementation detail?

## Open questions (reconciler)

4. **Q4 `hudLayout`.** Implementation detail (note 6); no change to basic-gameplay section 12. Suggest closing it as such.
