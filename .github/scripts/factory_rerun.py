#!/usr/bin/env python3
"""Tick pass: re-run a failed check once per head commit.

Step 1 of the Blocked-ticket loop. For each open ticket PR (head
`claude/issue-<n>-`, or "Closes #n" for an open `factory` ticket) the loader
stores the workflow runs on the head commit as `pr["runs"]`
(`id`, `name`, `conclusion`, `run_attempt`). A failed check workflow on
attempt 1 gets a `rerun_jobs` action; `run_attempt > 1` means it was already
re-run, so the pass is level-triggered and keeps no state. A re-run that
failed again becomes one `report`. Agent workflows are never re-run.
(docs/design/factory-orchestrator.md Phase A, section 3 step 1)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_tick import Action  # noqa: E402

CHECKS = {"CI", "PR size", "Review gate"}
FAILED = {"failure", "timed_out", "startup_failure"}
CLOSES = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?) #(\d+)\b", re.I)


def is_ticket_pr(snap, pr):
    m = re.match(r"claude/issue-(\d+)-", pr["head"])
    numbers = {int(m.group(1))} if m else {int(n) for n in CLOSES.findall(pr.get("body") or "")}
    return any(i["number"] in numbers and i["state"] == "open" and "factory" in i["labels"]
               for i in snap.issues)


def failed_runs(snap):
    """(pr, run) for each failed check run on a ticket PR's head commit."""
    return [(pr, run) for pr in snap.prs if is_ticket_pr(snap, pr)
            for run in pr.get("runs", [])
            if run["name"] in CHECKS and run["conclusion"] in FAILED]


def messages(snap):
    """(target, message) for each check that failed again after a re-run."""
    return [(f"pr#{pr['number']}",
             f"{run['name']} failed again after a re-run on {pr['sha'][:7]}")
            for pr, run in failed_runs(snap) if run["run_attempt"] > 1]


def rerun_failed(snap):
    out = [Action("rerun_jobs", f"pr#{pr['number']}", {"run_id": run["id"]},
                  f"{run['name']} failed on {pr['sha'][:7]}; re-running once")
           for pr, run in failed_runs(snap) if run["run_attempt"] == 1]
    return out + [Action("report", t, {"kind": "rerun"}, m)
                  for t, m in messages(snap) if (t, m) not in snap.reported]
