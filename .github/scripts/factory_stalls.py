#!/usr/bin/env python3
"""Tick pass: find design docs the factory has stopped moving on.

The checks of `stalled.py`, run over a `Snapshot` with no API calls. Each
accepted doc is checked (docs with an open PR are skipped: the PR is the
next step):

- no phases in the header: the planner skips the doc entirely;
- next `todo` phase has no plan;
- next phase's newest plan is closed with every sub-issue closed, but the
  phase is still `todo`.

Leftover `docs/*` branches the docs updater would want to create again (an
open plan's phase-done branch, any `-sync` branch) are reported too.
Findings are `report` actions; nothing is edited.
(docs/design/factory-orchestrator.md Phase A, taking over stalled.py)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_tick import Action  # noqa: E402


def messages(snap):
    """(target, message) for every stall in the snapshot."""
    open_prs = [p for p in snap.prs if p.get("state", "open") == "open"]
    busy = {f for p in open_prs for f in p.get("files", [])}
    heads = {p["head"] for p in open_prs}
    out, open_plans = [], []
    for doc in snap.docs:
        path = doc["file"]
        name = os.path.basename(path)[:-3]
        if name == "README":
            continue
        mine = [p for p in snap.plans
                if (p.get("plan_source") or {}).get("file") == path]
        open_plans += [(name, p["number"]) for p in mine if p["state"] == "open"]
        if doc.get("status") != "accepted" or path in busy:
            continue
        phases = doc.get("phases") or {}
        if not phases:
            out.append((path, f"`{path}` has no phases, so the planner skips it. "
                        f"Run the reconciler on it to add them: "
                        f"`gh workflow run claude-docs.yml -f path={path}`."))
            continue
        todo = [p for p, s in phases.items() if s == "todo"]
        if not todo:
            continue
        section = f"Phase {todo[0]}"
        these = [p for p in mine if p["plan_source"].get("section") == section]
        if not these:
            out.append((path, f"`{path}` {section} has no plan. "
                        f"`gh workflow run claude-plan.yml -f file={path}`."))
            continue
        latest = max(these, key=lambda p: p["number"])
        if latest["state"] != "closed" or latest.get("state_reason") == "not_planned":
            continue
        subs = [i for i in snap.issues if i.get("parent") == latest["number"]]
        if subs and all(s["state"] == "closed" for s in subs):
            n = latest["number"]
            out.append((path, f"`{path}` {section}: plan #{n} is closed but the phase "
                        f"is still `todo`. The doc's Reconciliation notes say what's missing; "
                        f"file a ticket for it as a sub-issue of #{n} and reopen the plan."))
    branches = {b for b in snap.branches if b.startswith("docs/")} - heads
    for name, n in open_plans:
        b = f"docs/{name}-phase-done-{n}"
        if b in branches:
            out.append((b, f"Branch `{b}` is left over; delete it before plan #{n} closes, "
                        f"or the docs updater can't push its phase-done PR."))
    for b in sorted(x for x in branches if x.endswith("-sync")):
        out.append((b, f"Branch `{b}` is left over; delete it, or the next drift run can't push."))
    from factory_rerun import messages as rerun_messages
    return out + rerun_messages(snap)


def stalls(snap):
    """Pass: `report` actions for stalls not yet reported."""
    return [Action("report", target, {"kind": "stalled"}, msg)
            for target, msg in messages(snap)
            if (target, msg) not in snap.reported]
