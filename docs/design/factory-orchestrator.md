---
title: Factory orchestrator
status: accepted
scope: both
source: "#566"
superseded_by: ""
phases:
  A: done
  B: todo
  C: todo
  D: todo
  E: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **No orchestrator.** The factory is 12 workflows in `.github/workflows/`
  (`ci`, `claude`, `claude-idea`, `claude-plan`, `claude-spec`,
  `claude-review`, `claude-docs`, `review-gate`, `pr-size`, `project-sync`,
  `stalled`, `resolve-conflicts`) and 11 scripts in `.github/scripts/`. Each
  workflow reacts to its own slice of issue, label, comment and PR events.
  `FACTORY.md` describes this design and lists it as intended (decisions 3
  and 10).
- **State is in GitHub text.** Plans are "Plan:" issues with a hidden marker
  naming doc and phase (`plan_targets.py`, `claude-plan.yml`); tickets are
  attached as sub-issues (`attach_subissues.py`); dependencies are
  `**Depends on:**` and `Blocked by #n` text parsed by `tickets.py`; phase
  status lives in each design doc's header on `master`; the review verdict
  is parsed from a comment by `review_gate.py`; the implementer's result is
  checked by `expect.py`.
- **Observers already added.** `stalled.py` (and `stalled.yml`, daily plus
  after the docs updater) reports accepted docs with no phases, a `todo`
  phase with no plan, and leftover `docs/*` branches. `tickets.py` has
  `blocked`, `ready` and `check`, and predicts files from backticked names
  in the ticket body (the "predicted-files regex" of section 3). The
  implementer's `start-ready` step and the `factory-auto-start` comment
  marker live in `claude.yml`.
- **Project board.** `project_sync.py` (421 lines) rebuilds the whole board
  using the `PROJECT_TOKEN` secret (a classic PAT). `project-sync.yml` runs
  on `issues` and `pull_request` events plus cron `7,37 * * * *`, and
  cancels superseded runs. That is the event-driven shape behind the API
  storm in section 1.
- **Agents.** Every agent runs through `anthropics/claude-code-action` with
  `claude_args` and `CLAUDE_CODE_OAUTH_TOKEN`
  (`claude.yml`, `claude-idea.yml`, `claude-plan.yml`, `claude-review.yml`,
  `claude-spec.yml`, `claude-docs.yml`, `resolve-conflicts.yml`). No
  workflow requests a structured output schema.
- **Invariants partly enforced already.** CI checks the design-doc header
  (`status`, `scope`, `phases`) in `.github/workflows/ci.yml`;
  `plan_targets.py` skips docs with no phases; PR size is enforced by
  `pr_size.py`; the `review-verdict` status is `review_gate.py`.
- **Phase A progress (plan #570, closed).** The tick exists:
  `factory_tick.py` with the invariant, stall, report, ready-ticket,
  plan-closing and board passes (PRs #600, #603, #605, #608, #610, #611,
  #602, #614, #617, #674, #678), the replay test (#617), `factory-tick.yml`
  (#754, #757), the retirement of the old workflows (#785) and the
  once-per-head-commit re-run of failed checks (`factory_rerun.py`, #786).
  The Blocked fix attempts, nits pass, retry budget and question issues of
  Phase A are now built too (see Reconciliation note 14).
- **Phase A remainder built since note 13** (PRs #933, #938, #941, #942,
  #946, #947, #949): `factory_blocked.py` (Blocked-fix pass),
  `factory_nits.py`, `factory_fix.py` (attempt markers and the retry
  budget), `factory_questions.py` (question issues, linked to tickets),
  `claude-fix.yml` (the fix run), `factory_needs_you.py` (the `needs-you`
  label, Phase E's start) and the fixtures in `test_factory_replay.py`. The
  tick's pass order is in `factory_tick.py`. `resolve-conflicts.yml` is
  still present.
- **Phase A done (plan #884, closed).** `resolve-conflicts.yml` was retired
  in #1101 and `conflicts.py` with the resolver docs in #1107; the nits,
  Blocked-fix, retry-budget and question pieces are #896, #926, #931, #933,
  #938, #941, #942, #946, #947, #949.
- **Not present** (the `factory-tick` workflow and the `needs-you` label
  now exist, see Phase A progress): any state store, any `factory-state`
  branch, file leases, per-transition history, a "needs
  you" queue, a GitHub App identity, self-hosted runners, or any reference
  to these in `docs/design/`. No other design doc covers the factory;
  `docs/design/README.md` only describes the doc lifecycle the factory runs.

## Reconciliation notes

1. **Header scope.** The doc says `Scope: factory`, but CI accepts only
   `generator`, `game` or `both` (`.github/workflows/ci.yml`, the header
   check). The header uses `both` so the check passes; nothing here touches
   the generator or the game. Suggest adding `factory` to the allowed scopes
   (maintainer-only, `ci.yml` is a workflow file) and changing the header.
2. **Agents cannot edit workflows.** `FACTORY.md` and `CLAUDE.md` say the
   Claude GitHub app cannot change `.github/workflows/`, and `CODEOWNERS`
   requires the maintainer's review for workflows. Phase A creates
   `factory-tick` and removes `project-sync.yml`, `stalled.yml` and parts of
   `claude.yml` and `claude-docs.yml`; Phase C changes the `claude_args` of
   every agent workflow. The implementer can write the scripts and tests
   (`.github/scripts/`, Python); the workflow edits must be done by the
   maintainer. The planner should split each phase that way and tag the
   workflow tickets `[manual check]`. The maintainer makes each workflow
   change and the agent prepares the script side; the factory gets no token
   allowed to change workflows, so `FACTORY.md` decision 1 stands.
   (Decided in #582.)
3. **Rule 1 vs. design docs.** Rule 1 puts "docs, phases" in the store, but a
   doc's `status` and `phases` header is read by CI, `plan_targets.py` and
   `stalled.py`, and is changed only by merged PRs (`docs/design/README.md`).
   Suggest the header stays the source of truth for docs and phases (a human
   merge is the acceptance), and the store mirrors it and owns plans,
   tickets, decisions, runs and attempts.
4. **Rule 1 vs. `FACTORY.md`.** `FACTORY.md` decision 3 ("three sources of
   truth") and decision 10 ("ticket progress lives in GitHub's sub-issues")
   would become false after Phase B. Updating `FACTORY.md` is not part of
   this PR; the last ticket of Phase B should do it.
5. **Rule 8 holds, with one tension.** `/decide` is handled by `claude-spec.yml`
   and `/override` by `review-gate.yml`; both are "human inputs read back"
   under rule 1, so they stay as parsers. The tick must keep the existing
   guard that only the maintainer's account counts (`FACTORY.md`
   decision 6 and 11).
6. **`[external]` items.** Phase D's machines (self-hosted runner or worker
   pool) are already tagged `[external]` by the author. Also external: a
   hosted database (open question 1, option B), a long-running service
   (open question 2), creating and installing the GitHub App and its
   secrets (open question 3), and a push-notification channel in Phase E.
   The planner should plan only the repo side of each (for example, a
   timing report script for Phase D).
7. **Self-hosted runners on a public repository.** `FACTORY.md` decision 11
   (public repo, never run on fork PRs) applies to Phase D: a self-hosted
   runner must only take jobs from the factory workflows triggered by the
   maintainer, never from `pull_request` events from forks. Add this to the
   Phase D ticket's acceptance.
8. **`[manual check]` criteria.**
   - Phase A, "replaying tonight's issue history ... produces no stall the
     tick doesn't report or fix": testable only once fixtures exist.
     Measurable version: fixtures of the issues, PRs and plans named in
     section 1 (#452, #500, #470, #507, #521, #513, #519,
     `raft-scene-feedback.md`) are checked in under `.github/scripts/`, and
     a Python test asserts one `tick` over each produces the listed action
     or violation, and a second `tick` produces none. The phase's first
     ticket should be capturing those fixtures.
   - Phase B, "deleting and recreating the board from the store loses
     nothing": measurable as a test that renders views from a store and
     compares them to a snapshot, run twice.
   - Phase D, "median wall time ... reported before and after": needs real
     runs on real machines. `[manual check]`; the repo side is a script
     that computes both numbers from workflow run data.
   - Phase E, "every item the maintainer had to find by hand on 2026-10-20
     would have appeared": measurable as a fixture test over the same
     history as Phase A.
9. **Phase order and dependencies.** Phase B waits on open question 1.
   Phase D has no code dependency on A to C. Phase C is independent of B
   (it needs only a schema per role and a place to store results; the
   tick can create issues from the JSON before the store exists). The
   author's order is kept; the planner plans one phase at a time in header
   order.
10. **API budget (section 1, #565).** The doc proposes a GitHub App token
    (rule 6). Today `claude.yml` and `project-sync.yml` use `GITHUB_TOKEN`
    and `PROJECT_TOKEN`. A single scheduled tick with conditional requests
    cuts the call count even without the App, so Phase A can ship before
    open question 3 is answered.
11. **Structured output flag.** Phase C assumes `claude -p` or the Agent SDK
    can return JSON against a schema through `claude-code-action`. Whether
    the pinned action version passes the needed flag through `claude_args`
    is not checked in this repo. `[manual check]` before planning Phase C.
12. **Scheduled run limits.** Phase A's "five-minute schedule" is GitHub's
    minimum cron interval; scheduled runs can be delayed and only run from
    the default branch. Webhook-triggered runs (`workflow_dispatch` or
    `repository_dispatch`) are what make this feel fast.
13. **Phase A is incomplete (plan #570).** The plan's tickets all closed, but
    none covered the part of Phase A that follows the re-run step of
    section 3, "Blocked tickets": a fix attempt per Blocked cause (conflict,
    failed check that fails again, reviewer block), the retry budget with
    `fix_attempt` transitions, one `needs-you` question issue when the budget
    is spent or the fix needs the maintainer, and the single nits pass with
    at most one question. `.github/scripts/factory_*.py` has no such code, and
    `resolve-conflicts.yml`, which the nits and fix steps should replace, is
    still present. Phase A stays `todo` so the planner plans these.

14. **Phase A is nearly done; one maintainer step is left.** The code that
    note 13 found missing now exists (Current state). What remains is the
    "old workflows are removed as the tick covers them" part for the nits
    and conflict steps: `resolve-conflicts.yml` is still in
    `.github/workflows/` and `CLAUDE.md` (The loop, step 3) and
    `.github/scripts/tickets.py` still refer to it. Removing a workflow is
    maintainer-only (note 2). Phase A stays `todo` until the maintainer
    removes it; then mark it `done`.
    > **Reconciler:** resolved. #1101 and #1107 removed
    > `resolve-conflicts.yml`, `conflicts.py` and the references; Phase A is
    > `done`.

---

# Mince Raft: Factory orchestrator

Status: idea
Scope: factory (`.github/`, `FACTORY.md`)

> **Reconciler:** `Status` above is the original idea text; the header of
> this file (`accepted`) is authoritative. See note 1 on `Scope`.

Replace the event-driven web of workflows with one orchestrator that owns the factory's state, recomputes what should happen on a loop, and gets structured results from agents. GitHub stays where humans review and merge; it stops being where the state lives. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. Why

On 2026-10-19/20 the factory stalled several times, and every cause was the same shape: state spread across issue bodies, labels, comments and branch names, parsed by regexes in workflows that each react to a slice of events.

- **Phases left `todo` with nothing planning them.** basic-gameplay Phases 7 and 8 closed their plans with a piece missing: the planter (#452), and the phase clock (#500), which was waiting on decision #470. Nothing noticed until a human asked.
- **Orphaned follow-ups.** Tickets filed after their plan closed (#452), and the implementer's split-off halves (#521 after #507), had no parent plan, so closing them never marked the phase done.
- **A deadlock.** #519 named its own plan (#513) as a dependency.
- **Missing phases.** `raft-scene-feedback.md` was accepted with no phases, and the planner skipped it silently.
- **Name collisions.** Old `docs/*-phase-done-N` branches made the docs updater pick suffixed names, and `expect.py` then reported its successful runs as failures.
- **A confused reviewer.** It answered "@claude fix the nits" instead of reviewing, because tag mode made the newest mention its task (fixed in #463).
- **An API storm.** Project sync ran 447 times in an hour and used up the workflow token's 1,000-requests-an-hour budget. The review gate, the reviewer and the implementer's dependency check all failed with it (#565).
- **Nobody saw what was waiting on the maintainer.** Merges, branch deletions and relabels waited unnoticed.
- **Blocked PRs waited for a "pls fix".** A PR the reviewer blocked, or whose check failed, sat in Blocked until the maintainer noticed it and commented `@claude` asking for a fix. Many only needed a failed CI run re-run, most of the rest needed no judgment, and the few that did weren't asked as questions.

Each fix so far (`stalled.py`, orphan adoption, debounce) adds another observer. The goal is one owner.

## 2. Design rules

1. **One state store is the source of truth.** Docs, phases, plans, tickets, dependencies, decisions, runs and attempts are rows (SQLite to start), not text in issues. Issues, labels and the project board are views the orchestrator writes; it never parses them back, except for human inputs (`/decide`, `/override`, labels the maintainer sets, merges).

> **Reconciler:** see notes 3 and 4 (docs and phases stay in their headers;
> `FACTORY.md` needs updating when this lands).

2. **Level-triggered, not edge-triggered.** A `tick` reads the state and GitHub, computes what should be true, and does the difference: start ready tickets, close finished plans, mark phases done, flag anything stuck. It is idempotent and safe to run twice. Webhooks only make the next tick come sooner. A missed event costs one tick, not a stuck phase.

> **Reconciler:** "mark phases done" means opening the PR the docs updater
> opens today (`claude-docs.yml`); the tick cannot push to `master`
> (protected, `FACTORY.md` decision 1). Runs of the tick should share one
> `concurrency` group so two ticks never overlap.

3. **Invariants live in the schema.** A ticket belongs to exactly one plan. A plan is never a dependency. A phase can't be closed while it has an answered decision with no ticket, or a ticket that isn't done. A doc can't be `accepted` without phases. A violated invariant is an error the tick reports, not a stall.

> **Reconciler:** before Phase B there is no schema; in Phase A these are
> checks in the tick's code, reported to one issue (as the Phase A text says).

4. **Agents return structured results.** Every agent run (planner, implementer, reviewer, reconciler, docs updater, spec editor) returns JSON against a schema, for example `{pr, follow_ups: [...], questions: [...], blocked_on: [...]}` or `{verdict, reviewed_commit, blockers, nits}`. The orchestrator creates the issues, links and comments. No `expect.py`, no "Blocked by" scraping, no verdict regexes.

> **Reconciler:** see note 11. This rule changes who creates issues: today
> the planner and implementer create them with `gh` under their tool
> allow-lists (`claude_args`). The allow-lists shrink as each role moves.

5. **One job per agent run.** A run gets its task from the orchestrator, never from the newest comment on the thread.
6. **One identity, one budget.** The factory uses its own GitHub App token, conditional requests (304s are free), batched GraphQL reads, and writes only what changed.

> **Reconciler:** `[external]` setup of the App itself; see notes 6 and 10.

7. **A "needs you" queue.** The orchestrator knows which items wait on the maintainer (merge, decide, override, approve). It keeps one list, ordered by how much work each item blocks, with time in state, and alerts past a threshold.
8. **CLAUDE.md lanes and the design-doc lifecycle don't change.** Clean-room rules, spec and design lanes, `/decide`, the 200-line limit, the reviewer's gate: same rules, new plumbing.

> **Reconciler:** see notes 2 and 5. `.github/` is the implementation lane
> except for workflow files, which only the maintainer can change.

9. **Blocked is a job, not a resting place.** A ticket whose PR is Blocked gets a fix attempt from the tick without anyone asking. It only waits on the maintainer through a question issue that says exactly what input or action is needed (section 3, "Blocked tickets").

## 3. Ticket and plan states

Ticket: `proposed` → `approved` (maintainer) → `waiting` (dependencies, file leases) → `running` → `in_review` → `merged` → `done`, plus `blocked_on_question` and `failed` (retry count, last error). Plan: `open` → `built` → `phase_done`. Every transition is recorded with a timestamp and a reason, so "why is this stuck" is a query.

File leases replace `tickets.py`'s predicted-files regex: a ticket claims the files it expects to touch while it runs and is in review, and a ticket that needs a claimed file waits.

> **Reconciler:** `tickets.py` today also treats open PRs' changed files as
> claims; leases must cover that case (a PR opened outside the factory) or
> keep reading open PR file lists. "Expects to touch" still needs a source:
> the ticket's backticked files today. Suggest the planner emits a `files`
> list in its structured result (Phase C) and the tick uses the regex until
> then.

### Blocked tickets

A ticket is Blocked when its PR can't merge as is: it conflicts with `master`, a check failed on the head commit, or the reviewer said Blocked about the head commit and nobody used `/override` (the board's Blocked column today). Each tick, for every Blocked ticket:

1. **Re-run first.** If the cause is a failed check, the tick re-runs the failed jobs once for that head commit before anything else, since many failures are flaky or infrastructure errors (a runner lost, a network timeout, the API budget of section 1). A re-run is not a fix attempt and costs no agent run. If it passes, the ticket goes back to waiting on review; the tick records a `rerun` transition with the check's name, so a check that keeps needing re-runs shows up as a query (and in the needs-you queue past a threshold, as a flaky test to fix). If it fails again, go to step 2 with the second run's log.
2. **Try a fix.** If no fix run is in flight and none has been tried for this PR, head commit and cause, start one. Its task comes from the tick (rule 5), built from the cause: merge `master` and resolve the conflicts; the failing check's name and log excerpt; or the reviewer's numbered blockers. The fix runs as the implementer under the same rules (CLAUDE.md, the 200-line limit, every check before pushing), and its push gets reviewed again like any other. This replaces the maintainer's "pls fix" comment and `resolve-conflicts.yml`.
3. **Retry within a budget.** A fix that pushes a commit clears the attempt; if the new head is Blocked again, that counts as the next attempt. After two attempts on one ticket that end Blocked (a setting in the tick's config), or a run that pushes nothing, the tick stops trying and asks (step 4). Attempts and their outcomes are recorded as transitions (`fix_attempt`, with the cause and the run).
4. **Ask when it needs the maintainer.** The fix run stops, and the tick opens a question issue, when the fix needs something only the maintainer can give:
   - the spec or a design doc is ambiguous, incomplete or seems wrong (the usual `spec-question` with `decision` or `research`; never filled from knowledge of the original);
   - the agent disagrees with a reviewer blocker (an `/override` or a reply to the reviewer);
   - a conflict that needs a real decision about which side wins;
   - a change outside the implementation lane (a workflow file, `SPEC.md`, a design doc, a secret, branch protection);
   - the PR can't fit 200 lines and splitting it would leave a broken first part (`oversize-ok` or a different split);
   - the retry budget is spent.

   The question names the ticket and PR, quotes the blocker, says what was tried, and lists options (A, B, ...) or the one action needed (merge, label, `/override <reason>`, edit a workflow), so it can be answered from the issue alone. Questions that aren't about the spec are labelled `needs-you`. The ticket moves to `blocked_on_question` and is linked as blocked by the question, so it restarts when the question is answered or closed. One open question per ticket and cause: later ticks update it instead of opening another.

The tick leaves a Blocked PR alone while the maintainer is working on it: a maintainer push or `@claude` comment after the block counts as the attempt. PRs not opened by the factory (not on a `claude/` branch) are never fixed automatically; they only appear in the needs-you queue.

**Nits.** A ticket whose PR is at Nits ("Ready, with nits" on the head commit) gets one nits pass before it reaches the maintainer: the tick starts a fix run on the reviewer's nits, which is what an "@claude fix nits" comment does today, but with the task given by the tick (rule 5; see #463 for what a comment as the task did to the reviewer). The fix addresses each nit or says why not, and the push is reviewed again. If the re-review says Ready, the ticket is Ready for Merge. If nits remain, the tick doesn't loop: it opens one `needs-you` question listing each remaining nit with a specific question (fix it, leave it, or make it a follow-up ticket), and the PR waits there instead of in a bare Nits column.

## 4. Implementation phases

**Phase A: One tick.** A single `factory-tick` workflow on a five-minute schedule (plus `workflow_dispatch` and webhook-triggered runs) takes over from find-ready and start-ready, project sync, `stalled.py`, and the docs updater's plan-closing step. It reads GitHub as today but is level-triggered and idempotent, and enforces the invariants of rule 3 by reporting violations to one issue. It also runs the Blocked-ticket and Nits loops of section 3: re-runs of failed checks, fix attempts, one nits pass (replacing `resolve-conflicts.yml`), the retry budget, and question issues when a fix needs the maintainer. Before Phase C the fix run reports that it stopped with a marker comment the tick reads. The old workflows are removed as the tick covers them. Done when replaying tonight's issue history in a test (fixtures of issues, PRs and plans) produces no stall the tick doesn't report or fix, and a second tick right after the first does nothing; and when a failed check gets exactly one re-run before any fix attempt, each Blocked cause (conflict, failed check that fails again, reviewer block) gets one fix attempt, and one whose attempts fail gets exactly one question issue, and a Nits PR gets one nits pass and then at most one question, on any number of ticks.

> **Reconciler:** `[manual check]` criteria and a measurable version in
> note 8. Workflow edits are maintainer-only (note 2). The review-gate and
> reviewer workflows are not in this list and stay as they are.

**Phase B: State store.** The tick keeps its state in SQLite (on a `factory-state` branch or a small hosted store) and writes issues, labels and the board as views. Human inputs (labels, `/decide`, `/override`, merges) are the only things read back. Done when deleting and recreating the board from the store loses nothing, and a ticket can't exist without a plan. The project board stays as a view the tick writes until Phase E exists. (Decided in #1113.)

> **Reconciler:** blocked on open question 1. A hosted store is `[external]`.
> The `factory-state` branch needs a branch-protection exemption for the
> tick's identity, since `master` is protected; a separate branch is fine.

**Phase C: Structured agent results.** Agent runs return JSON against a schema per role (implementer first, then reviewer, planner, docs updater, reconciler, spec editor), through `claude -p` with an output schema or the Agent SDK. A fix run's result includes `needs_maintainer: {kind, question, options}` when it stops, and the tick writes the question issue from it. Done when `expect.py` and the comment and verdict regexes are deleted.

> **Reconciler:** see note 11. Deleting the verdict regexes in
> `review_gate.py` changes the `review-verdict` check, which is required on
> `master`; do it last and keep `/override` working throughout.

**Phase D: Warm workers.** Agent runs move to a self-hosted runner or a small worker pool with a warm Rust and wasm toolchain, a cargo cache and a git worktree per ticket. GitHub-hosted runners keep CI. The machines themselves are `[external]`. Done when a ticket's median wall time and the time from ticket start to first commit are reported before and after.

> **Reconciler:** `[external]` machines; see notes 6, 7 and 8. The shared
> setup today is `.github/actions/toolchain/`.

**Phase E: Needs-you queue and alerts.** A view (an issue, a page, or a push notification) listing what waits on the maintainer, ordered by blocked work, with time in state and an alert threshold. Open `spec-question` and `needs-you` issues from Blocked tickets are queue items, ranked by the work they block. Done when every item the maintainer had to find by hand on 2026-10-20 would have appeared there.

> **Reconciler:** a push notification channel is `[external]`; an issue or
> page can be built here. See note 8 for a measurable version.

## Open questions

1. Where the store lives: a SQLite file committed to a `factory-state` branch (simple, auditable, merge races), or a hosted database (needs a secret and something `[external]`).
2. Whether the tick stays a GitHub Actions workflow, or becomes a long-running service with webhooks (faster, but `[external]` hosting).
3. GitHub App for the factory's identity: worth setting up now, or after Phase A shows the remaining API load?
5. Migration: do in-flight plans and tickets move into the store, or does the store start with the next plan?

---
_Generated by [Claude Code](https://claude.ai/code)_

## Open questions (reconciler)

Options for the author's questions, and new ones from the notes above.

1. **Store location** (author's question 1). A: SQLite file on a
   `factory-state` branch; the tick's concurrency group removes most merge
   races, and history is auditable. B: hosted database, `[external]`. Suggest
   A, with the tick as the only writer.
2. **Tick host** (author's question 2). A: stays a GitHub Actions workflow
   (cron plus dispatch). B: long-running service, `[external]`. Suggest A
   until Phase A's tick time and latency are measured.
3. **GitHub App** (author's question 3). A: after Phase A, using the
   measured request count. B: before, so Phase A starts with its own budget.
   Suggest A; the tick's batching already removes most calls.
5. **Migration** (author's question 5). A: the store starts with the next
   plan, in-flight work finishes under the old rules. B: import open plans
   and tickets at Phase B. Suggest B: Phase A's tick already has to read
   them, so the import is the same code run once.
6. **Header scope** (note 1). A: add `factory` to the CI scope check and use
   it here. B: keep `both`. Suggest A.
7. **Docs and phases in the store** (note 3). A: headers stay the source of
   truth, the store mirrors them. B: the store owns phase status and a PR
   writes it back. Suggest A.
