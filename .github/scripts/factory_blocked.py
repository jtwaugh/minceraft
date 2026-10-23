#!/usr/bin/env python3
"""Tick pass: one fix run per Blocked ticket PR, head commit and cause.

Step 2 of the Blocked-ticket loop. For each ticket PR on a `claude/` branch the
loader stores `mergeable_state`, `head_time` and `head_author`, the runs (a
check that failed after its re-run carries a log excerpt in `run["log"]`),
`blockers` (the numbered blockers of the reviewer's newest verdict when it is
Blocked about the head commit and no `/override` followed it), `claude_mentions`
(times of the maintainer's `@claude` comments) and `comments` (trusted ones).
The cause is the first of: conflict with `master`; a check that failed again
(`run_attempt > 1`, see factory_rerun.py); a reviewer block. A failed check
still on its first attempt gets only the re-run. A maintainer push, or an
`@claude` comment after the head commit, counts as the attempt: no fix.
(docs/design/factory-orchestrator.md Phase A, section 3 "Blocked tickets")
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_fix import blockers_task, check_task, conflict_task, markers  # noqa: E402
from factory_rerun import failed_runs, is_ticket_pr  # noqa: E402
from factory_tick import Action  # noqa: E402

MAINTAINER = "jtwaugh"
CAUSES = ("conflict", "check", "review")
SECTION = re.compile(r"\*\*Blockers\*\*:?(.*?)(?=\*\*(?:Nits|Notes)\*\*|\Z)", re.S | re.I)
ITEM = re.compile(r"^\s*\d+[.)]\s+(.*?)(?=^\s*\d+[.)]\s|\Z)", re.S | re.M)


def parse_blockers(body):
    """The reviewer's numbered blockers, each as one line of text."""
    m = SECTION.search(body or "")
    return [" ".join(i.split()) for i in ITEM.findall(m.group(1))] if m else []


def cause_of(snap, pr):
    """(cause, task) for a Blocked PR, or None."""
    if pr.get("mergeable_state") == "dirty":
        return "conflict", conflict_task()
    again = [r for p, r in failed_runs(snap) if p is pr and r["run_attempt"] > 1]
    if again:
        return "check", check_task(again[0]["name"], again[0].get("log", ""))
    if pr.get("blockers"):
        return "review", blockers_task(pr["blockers"])
    return None


def maintainer_working(pr):
    return (pr.get("head_author") == MAINTAINER
            or any(t > pr.get("head_time", "") for t in pr.get("claude_mentions", [])))


def blocked(snap):
    out = []
    for pr in snap.prs:
        if not (pr["head"].startswith("claude/") and is_ticket_pr(snap, pr)) or maintainer_working(pr):
            continue
        found = cause_of(snap, pr)
        if found is None:
            continue
        started = {sha for n, sha, c in markers(pr.get("comments", []))
                   if n == pr["number"] and c in CAUSES}
        if pr["sha"] not in started:
            out.append(Action("start_fix", f"pr#{pr['number']}", {
                "kind": found[0], "head": pr["sha"], "task": found[1]},
                f"Blocked on {pr['sha'][:7]} ({found[0]}); starting a fix"))
    return out
