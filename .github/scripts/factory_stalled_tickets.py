#!/usr/bin/env python3
"""Tick pass: approved tickets that were started but have no pull request.

The ready pass starts a ticket once (`factory-auto-start` marker). If that
run ends without a PR, nothing used to start it again, and a branch it
pushed without opening a PR blocked every restart (`tickets.claimed`), so
the ticket sat silently (#777). For each open `factory` ticket with a start
marker, no open PR, no `local` label and no implementer run in progress:

- a `claude/issue-<n>-` branch has work and no PR (`snap.stray[n]`): open
  the PR from it. CI and the reviewer then run, and the Blocked-fix pass
  takes it from there;
- otherwise, start it again, until it has been started RETRY_BUDGET times;
- then open one `needs-you` question on the ticket.

Implementer runs come from `snap.implementer_runs` (claude.yml runs, named
"Claude implementer #<n>" when the tick dispatches them, else the issue
title). A start marker with no run after FIX_LOST_AFTER counts as a start
that ran and failed.
"""
import os
import sys
from datetime import datetime, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_config import RETRY_BUDGET  # noqa: E402
from factory_ready import MARKER, _as_pr, _waiting_on  # noqa: E402
from factory_tick import FIX_LOST_AFTER, Action  # noqa: E402
import tickets  # noqa: E402

ACTIVE = {"queued", "in_progress", "waiting", "requested", "pending"}


def key_of(n):
    return f"<!-- factory-question ticket={n} cause=no-pr -->"


def _time(s):
    return datetime.fromisoformat(s.replace("Z", "+00:00"))


def state_of(snap, t, now=None):
    """(state, detail) of a started ticket with no PR, or None when it isn't
    one. States: running, starting, open-pr, restart, ask, asked."""
    now = now or datetime.now(timezone.utc)
    n, by = t["number"], {i["number"]: i for i in snap.issues}
    prs = [_as_pr(p) for p in snap.prs if p.get("state", "open") == "open"]
    starts = sorted(c["created_at"] for c in t.get("comments", [])
                    if c.get("user") in tickets.TRUSTED and MARKER in (c.get("body") or ""))
    if (not starts or t.get("state") != "open" or "factory" not in t.get("labels", [])
            or tickets.LOCAL in t.get("labels", []) or tickets.has_open_pr(n, prs)
            or _waiting_on(t, by)[0]):
        return None
    runs = [r for r in getattr(snap, "implementer_runs", [])
            if r.get("display_title") in (f"Claude implementer #{n}", t["title"])]
    if any(r.get("status") in ACTIVE for r in runs):
        return "running", "an implementer run is in progress"
    last = _time(starts[-1])
    if not any(_time(r["created_at"]).timestamp() >= last.timestamp() - 120 for r in runs) \
            and (now - last).total_seconds() < FIX_LOST_AFTER:
        return "starting", "started; its run hasn't appeared yet"
    stray = getattr(snap, "stray", {}).get(n, [])
    if stray:
        return "open-pr", stray[0]
    if any(key_of(n) in (i.get("body") or "") and i["state"] == "open" for i in snap.issues):
        return "asked", "a question is open"
    if len(starts) < RETRY_BUDGET:
        return "restart", f"start {len(starts) + 1} of {RETRY_BUDGET}"
    return "ask", f"started {len(starts)} times, no pull request"


def stalled_tickets(snap):
    out = []
    for t in sorted(snap.issues, key=lambda i: i["number"]):
        found = state_of(snap, t)
        if not found:
            continue
        state, detail = found
        n = t["number"]
        if state == "open-pr":
            out.append(Action("open_pr", f"issue#{n}", {
                "branch": detail, "title": t["title"],
                "body": (f"Closes #{n}\n\nOpened by the factory tick from `{detail}`: an "
                         "implementer run pushed this work without opening a pull request. "
                         "The run may not have finished its checks; CI and the reviewer "
                         "check it here.\n")},
                f"`{detail}` has work and no PR; opening it"))
        elif state == "restart":
            out.append(Action("start_ticket", f"issue#{n}", {},
                              f"started before, no PR and no run in progress; {detail}"))
        elif state == "ask" and not any(key_of(n) in (i.get("body") or "") for i in snap.issues):
            out.append(Action("open_question", f"issue#{n}", {
                "label": "needs-you", "key": key_of(n),
                "title": f"Ticket #{n}: no pull request after {RETRY_BUDGET} starts",
                "body": (f"{key_of(n)}\nTicket #{n} was started {RETRY_BUDGET} times and no run "
                         "opened a pull request. Each run's report is on the ticket.\n\n"
                         "- A: comment `@claude <what to do>` on the ticket.\n"
                         "- B: build it yourself (label it `local`).\n"
                         "- C: rewrite or split the ticket, then close this question.\n")},
                f"{detail}; asking"))
    return out
