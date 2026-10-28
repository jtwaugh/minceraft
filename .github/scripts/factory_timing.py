#!/usr/bin/env python3
"""Timing maths for implementer runs: wall time and time to first commit.

Pure functions, no network. `runs` are `Claude implementer #N` run records,
dicts with `ticket`, `start`, `end` (ISO 8601, `Z` or offset) and
`conclusion`; `first_commits` maps a ticket number to the time of its first
commit on its `claude/issue-N-*` branch. `summarise` splits tickets into
those started before `cutoff` and those started on or after it and gives
each group's median wall time and median start-to-first-commit, in seconds.
Cancelled runs are ignored. A ticket's wall time is the sum of its runs; its
start is its earliest run's start. A ticket with no commit counts in
`no_commit` and is left out of the second median.
(docs/design/factory-orchestrator.md Phase D, Reconciliation note 8)
"""
from datetime import datetime, timezone
from statistics import median


def parse(s):
    t = datetime.fromisoformat(s.replace("Z", "+00:00"))
    return t if t.tzinfo else t.replace(tzinfo=timezone.utc)


def _group(tickets, first_commits):
    walls = [t["wall"] for t in tickets]
    to_commit = []
    for t in tickets:
        c = first_commits.get(t["ticket"])
        if c is not None:
            to_commit.append((parse(c) - t["start"]).total_seconds())
    return {
        "tickets": len(tickets),
        "median_wall": median(walls) if walls else None,
        "median_to_first_commit": median(to_commit) if to_commit else None,
        "no_commit": len(tickets) - len(to_commit),
    }


def summarise(runs, first_commits, cutoff):
    """{"before": {...}, "after": {...}} for tickets started before / on or after cutoff."""
    cut = parse(cutoff) if isinstance(cutoff, str) else cutoff
    per = {}
    for r in runs:
        if r.get("conclusion") == "cancelled":
            continue
        start, end = parse(r["start"]), parse(r["end"])
        t = per.setdefault(r["ticket"], {"ticket": r["ticket"], "start": start, "wall": 0.0})
        t["start"] = min(t["start"], start)
        t["wall"] += (end - start).total_seconds()
    tickets = list(per.values())
    return {
        "before": _group([t for t in tickets if t["start"] < cut], first_commits),
        "after": _group([t for t in tickets if t["start"] >= cut], first_commits),
    }
