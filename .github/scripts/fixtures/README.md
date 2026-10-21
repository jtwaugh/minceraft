# Factory tick fixtures

`history.json` is a snapshot of the 2026-10-19/20 stall history
(`docs/design/factory-orchestrator.md` section 1, reconciliation note 8).
Later tick tickets replay it: one `tick` over it must produce the action or
violation below for each case, and a second `tick` must produce none.

Shape: `issues` (number, title, state, labels, `kind`, `parent` plan,
`depends_on`, excerpt of the body), `prs` (head branch, changed files, issues
closed), `docs` (design-doc header: status and phases), `branches`. No tokens
or secrets.

## Provenance

Issues, PR files and branch names were read with `gh issue view`,
`gh issue list`, `gh pr list` and `git branch -r`. Two things could not be
read back and are **reconstructed from the doc's section 1**: sub-issue
parents (`parent` is `null` where the doc says the ticket had no plan;
`expected_parent` is the plan it should have had) and the state at the
moment of the stall (`stall_state_note`, and #519's dependency on #513, which
master's copy has since corrected). Design-doc headers are the stalled
shapes, not today's `master`.

## Cases and expected result

| Case | Expected by the tick |
| --- | --- |
| #452 follow-up filed after plan #287 closed | Violation `orphan-ticket` (no parent plan). Action: adopt it into plan #287's phase (`expected_parent`), reopening nothing; phase 7 of basic-gameplay is not marked done while it is open. |
| #500 and #470, phase clock waiting on a decision | Violation `answered-decision-without-ticket` for #470 while #500 is missing, then `orphan-ticket` for #500 (`expected_parent` #469). Phase 8 stays `todo` and is reported as `phase-without-plan` instead of silent. |
| #507 and #521, split-off half | #521 is a violation `orphan-ticket` (`split_from` #507, `expected_parent` plan #169). Action: adopt it into #507's plan. |
| #513 and #519, ticket depends on its own plan | Violation `plan-as-dependency`: #519 lists plan #513 in `depends_on`. Action: none automatic; reported to the violations issue. |
| `raft-scene-feedback.md` accepted with no phases | Violation `accepted-doc-without-phases`. |
| Leftover `docs/*-phase-done-N` branches | Violation `stale-branch` for each branch whose plan is closed and whose work is merged (name collisions: `-v2`, `-2`, `-r2` suffixes). Action: report, never delete. |

After the first tick every violation above is already reported (or fixed), so
a second tick over the same input produces nothing new.
