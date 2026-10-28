#!/usr/bin/env python3
"""Tick pass: find design docs the factory has stopped moving on.

The checks the retired `stalled.py` made, run over a `Snapshot` with no API
calls. Each accepted doc is checked (docs with an open PR are skipped: the
PR is the next step):

- no phases in the header: the planner skips the doc entirely;
- next `todo` phase has no plan;
- next phase's newest plan is closed with every sub-issue closed (or none),
  but the phase is still `todo`. If the docs updater's note says the phase
  is still incomplete after that plan, the tick also dispatches the planner
  to plan what's left (`request_replan`).

Leftover `docs/*` branches the docs updater would want to create again (an
open plan's phase-done branch, any `-sync` branch) are reported too.
Findings are `report` actions; nothing is edited.
(docs/design/factory-orchestrator.md Phase A)
"""
import os
import sys
from datetime import datetime, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_tick import Action  # noqa: E402
from plan_targets import incomplete_note  # noqa: E402

PLAN_WORKFLOW = "claude-plan.yml"
GRACE = 2 * 3600  # seconds after a plan closes for the docs updater to open its PR


def messages(snap):
    """(target, message) for every stall in the snapshot."""
    return [(t, m) for t, m, _ in findings(snap)]


def findings(snap, now=None):
    """(target, message, replan) for every stall; replan is whether the
    planner should be dispatched for the doc."""
    now = now or datetime.now(timezone.utc)
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
        # An open docs updater PR (phase-done or sync) is the next step too.
        if doc.get("status") != "accepted" or path in busy \
                or any(h.startswith(f"docs/{name}-") for h in heads):
            continue
        phases = doc.get("phases") or {}
        if not phases:
            out.append((path, f"`{path}` has no phases, so the planner skips it. "
                        f"Run the reconciler on it to add them: "
                        f"`gh workflow run claude-docs.yml -f path={path}`.", False))
            continue
        todo = [p for p, s in phases.items() if s == "todo"]
        if not todo:
            continue
        section = f"Phase {todo[0]}"
        these = [p for p in mine if p["plan_source"].get("section") == section]
        if not these:
            out.append((path, f"`{path}` {section} has no plan. "
                        f"`gh workflow run {PLAN_WORKFLOW} -f file={path}`.", False))
            continue
        latest = max(these, key=lambda p: p["number"])
        if latest["state"] != "closed" or latest.get("state_reason") == "not_planned":
            continue
        closed = latest.get("closed_at")
        if closed and (now - datetime.fromisoformat(closed.replace("Z", "+00:00"))).total_seconds() < GRACE:
            continue  # the docs updater is still on it
        subs = [i for i in snap.issues if i.get("parent") == latest["number"]]
        if all(s["state"] == "closed" for s in subs):
            n = latest["number"]
            if incomplete_note(section, n) in doc.get("text", ""):
                out.append((path, f"`{path}` {section}: plan #{n} is closed and the docs updater "
                            f"found the phase still incomplete; dispatched the planner for what's "
                            f"left. If no new plan appears, `gh workflow run {PLAN_WORKFLOW} "
                            f"-f file={path}`.", True))
                continue
            out.append((path, f"`{path}` {section}: plan #{n} is closed but the phase "
                        f"is still `todo`, with no \"{incomplete_note(section, n)}\" note. "
                        f"Check the doc's Reconciliation notes, add that note so the planner "
                        f"plans what's missing, or mark the phase done.", False))
    branches = {b for b in snap.branches if b.startswith("docs/")} - heads
    for name, n in open_plans:
        b = f"docs/{name}-phase-done-{n}"
        if b in branches:
            out.append((b, f"Branch `{b}` is left over; delete it before plan #{n} closes, "
                        f"or the docs updater can't push its phase-done PR.", False))
    for b in sorted(x for x in branches if x.endswith("-sync")):
        out.append((b, f"Branch `{b}` is left over; delete it, or the next drift run can't push.",
                    False))
    from factory_rerun import messages as rerun_messages
    return out + [(t, m, False) for t, m in rerun_messages(snap)]


def stalls(snap):
    """Pass: `report` actions for stalls not yet reported, each re-plan
    dispatched first. Once reported a stall isn't dispatched again, so a
    re-plan that makes no plan stays in the report instead of looping."""
    out = []
    for target, msg, replan in findings(snap):
        if (target, msg) in snap.reported:
            continue
        if replan:
            out.append(Action("request_replan", target, {"file": target}, msg))
        out.append(Action("report", target, {"kind": "stalled"}, msg))
    return out
