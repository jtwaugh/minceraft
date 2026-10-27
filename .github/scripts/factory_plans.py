#!/usr/bin/env python3
"""Plan-closing pass: close a plan once all its sub-issues are closed.

Took over from the retired `finish-plan` job of claude-docs.yml. A plan
whose sub-issues all closed is closed as completed, or as not planned (with a comment) when
it only ever had spec-questions. A plan that belongs to a design-doc phase
and closed as completed also gets a `request_phase_done` action; the
backend dispatches the docs updater, which opens the PR (the tick cannot
push to master). Plans already closed, and tickets with no plan, are left
alone: orphans are the invariants pass's concern.
(docs/design/factory-orchestrator.md Phase A, design rule 2)

A plan in the state store closes by design rule 3 instead: every stored
ticket is `done` and no answered decision lacks a ticket. A plan held back
by an unlinked decision is reported once. (Phase B)
"""
import sqlite3

from factory_tick import Action

QUESTIONS_ANSWERED = (
    "All questions answered. Closing this plan; the phase will be planned "
    "again with the answers. To do it now: "
    "`gh workflow run claude-plan.yml -f file=<doc>`.")


def _stored(store, plan):
    """(ticket states, answered decisions with no ticket) for a stored plan
    that has tickets, else None."""
    if store is None:
        return None
    try:
        states = [r[0] for r in store.execute("SELECT state FROM tickets WHERE plan = ?", (plan,))]
        loose = [r[0] for r in store.execute(
            "SELECT issue FROM decisions WHERE plan = ? AND answer IS NOT NULL AND ticket IS NULL "
            "ORDER BY issue", (plan,))]
    except sqlite3.OperationalError:
        return None
    return (states, loose) if states else None


def close_finished_plans(snap):
    actions = []
    for plan in snap.plans:
        if plan["state"] != "open":
            continue
        target = f"issue#{plan['number']}"
        stored = _stored(getattr(snap, "store", None), plan["number"])
        if stored:
            states, loose = stored
            if any(s != "done" for s in states):
                continue
            if loose:
                reason = ("held back: answered decision without a ticket: "
                          + ", ".join(f"#{d}" for d in loose))
                if (target, reason) not in snap.reported:
                    actions.append(Action("report", target, {"kind": "plan-held"}, reason))
                continue
        else:
            children = [i for i in snap.issues if i.get("parent") == plan["number"]]
            if not children or any(c["state"] != "closed" for c in children):
                continue
            if all("spec-question" in c["labels"] for c in children):
                actions.append(Action("close_issue", target, {
                    "reason": "not_planned", "comment": QUESTIONS_ANSWERED},
                    "plan only had questions, now answered"))
                continue
        actions.append(Action("close_issue", target, {"reason": "completed"},
                              "every ticket closed"))
        src = plan.get("plan_source") or {}
        if src.get("file", "").startswith("docs/design/"):
            actions.append(Action("request_phase_done", target, {
                "file": src["file"], "section": src.get("section", "")},
                "plan completed for a design-doc phase"))
    return actions
