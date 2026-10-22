#!/usr/bin/env python3
"""Tick pass: once master is green again, bring back the PRs it broke.

    refresh_after_red_master(snapshot) -> [Action("update_branch", "pr#n", {"sha": head})]

While master's CI is red, every PR's CI fails too: it runs on the PR merged
into master. Re-running doesn't help (a re-run tests the same merge), so
those PRs stay red after master is fixed until their branch is updated.
Once the newest finished master run is green, a ticket PR (not a draft, not
`superseded`) whose head commit failed CI during a red spell on master gets
its branch updated. The new head gets fresh CI and a fresh review, and
auto-merge takes it from there. A failure after master went green is the
PR's own and is left to the re-run pass and the maintainer.

A red spell runs from the first master run after a green one, in a streak
with at least one failure (cancelled runs alone don't count), to the next
green run. It is level-triggered: the update moves the head, and the new
head's runs are after the spell.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_rerun import FAILED, is_ticket_pr  # noqa: E402
from factory_tick import Action  # noqa: E402

CI = "CI"


def red_spells(master_runs):
    """[(start, end)] times master was red; end is None while it still is."""
    spells, start, red = [], None, False
    for r in sorted(master_runs, key=lambda r: r["created_at"]):
        if r["conclusion"] == "success":
            if red:
                spells.append((start, r["created_at"]))
            start, red = None, False
        else:
            start = start or r["created_at"]
            red = red or r["conclusion"] in FAILED
    if red:
        spells.append((start, None))
    return spells


def refresh_after_red_master(snap):
    if snap.master_ci != "success":
        return []  # still red (or unknown): an update would only fail again
    spells = [(s, e) for s, e in red_spells(snap.master_runs) if e is not None]
    out = []
    for pr in snap.prs:
        if (pr.get("draft") or pr.get("merged") or "superseded" in pr.get("labels", [])
                or not is_ticket_pr(snap, pr)):
            continue
        hit = next((run for run in pr.get("runs", [])
                    if run["name"] == CI and run["conclusion"] in FAILED
                    and any(s <= run.get("created_at", "") < e for s, e in spells)), None)
        if hit:
            out.append(Action("update_branch", f"pr#{pr['number']}", {"sha": pr["sha"]},
                              f"CI failed on {pr['sha'][:7]} while master was red; "
                              "master is green again"))
    return out
