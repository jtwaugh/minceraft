"""Derive ticket and plan states from a tick snapshot into the store.

`sync_states(snap, now)` works out each stored ticket's state and records a
transition, with a reason, when it changed; a repeat tick adds none. A stored
plan moves `open` to `built` when all its tickets are done. Only human inputs
are read (the `factory` label, a merge, a closed issue, a question) plus the
markers the existing passes leave; nothing is written to GitHub.

A ticket's state is the first that holds: closed is `done`; a merged PR is
`merged`; no `factory` label is `proposed`; a spent retry budget is `failed`;
an open question linked to it is `blocked_on_question`; an open PR is
`in_review`; a start marker or an implementer run in progress is `running`;
open dependencies are `waiting`; otherwise `approved`.
(docs/design/factory-orchestrator.md section 3, Phase B)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_blocked import attempts, cause_of  # noqa: E402
from factory_config import RETRY_BUDGET  # noqa: E402
from factory_questions import ASKED  # noqa: E402
from factory_ready import MARKER, _trusted, _waiting_on  # noqa: E402
from factory_stalled_tickets import ACTIVE  # noqa: E402
from factory_store_history import record_transition  # noqa: E402
import tickets  # noqa: E402


def _prs(snap, n):
    return [p for p in snap.prs if n in tickets.ticket_numbers(p.get("head", ""), p.get("body"))]


def _asks(q):
    """The ticket number a question names, or its PR's number as ('pr', n)."""
    m = ASKED.search(q.get("body") or "")
    return (int(m.group(1)), None) if m and m.group(1) else (None, int(m.group(2))) if m else (None, None)


def _open_question(snap, n, prs):
    numbers = {p["number"] for p in prs}
    for q in snap.issues:
        if q["state"] != "open":
            continue
        ticket, pr = _asks(q)
        if ticket == n or (pr is not None and pr in numbers):
            return q["number"]
    return None


def _budget_spent(snap, pr):
    if cause_of(snap, pr) is None:
        return False
    tried = attempts(pr)
    return len(tried) >= RETRY_BUDGET or any(o == "pushed nothing" for _, _, o in tried)


def ticket_state(snap, t):
    """(state, reason) of a ticket issue, from the snapshot alone."""
    n = t["number"]
    by = {i["number"]: i for i in snap.issues}
    if t.get("state") == "closed":
        return "done", "the issue is closed"
    prs = _prs(snap, n)
    merged = [p for p in prs if p.get("merged") or p.get("merged_at")]
    if merged:
        return "merged", f"PR #{merged[0]['number']} is merged"
    if "factory" not in t.get("labels", []):
        return "proposed", "no `factory` label"
    open_prs = [p for p in prs if p.get("state", "open") == "open"]
    spent = next((p for p in open_prs if _budget_spent(snap, p)), None)
    if spent:
        return "failed", f"retry budget spent on PR #{spent['number']}"
    q = _open_question(snap, n, open_prs)
    if q:
        return "blocked_on_question", f"question #{q} is open"
    if open_prs:
        return "in_review", f"PR #{open_prs[0]['number']} is open"
    if any(MARKER in (c.get("body") or "") for c in _trusted(t)):
        return "running", "started; no PR yet"
    runs = [r for r in snap.implementer_runs
            if r.get("display_title") in (f"Claude implementer #{n}", t["title"]) and r.get("status") in ACTIVE]
    if runs:
        return "running", "an implementer run is in progress"
    waiting, _ = _waiting_on(t, by)
    if waiting:
        return "waiting", "waiting on " + ", ".join(f"#{d}" for d in waiting)
    return "approved", "labelled `factory`"


def sync_states(snap, now):
    """Record a transition for each stored ticket whose state changed, then
    move plans whose tickets are all done to `built`. Returns the number of
    transitions recorded."""
    store, by, moved = snap.store, {i["number"]: i for i in snap.issues}, 0
    for (n,) in store.execute("SELECT id FROM tickets ORDER BY id").fetchall():
        if n in by:
            state, reason = ticket_state(snap, by[n])
            moved += record_transition(store, n, state, reason, now)
    plans = store.execute(
        "SELECT id FROM plans WHERE state = 'open' AND EXISTS (SELECT 1 FROM tickets WHERE plan = plans.id) "
        "AND NOT EXISTS (SELECT 1 FROM tickets WHERE plan = plans.id AND state <> 'done')").fetchall()
    for (p,) in plans:
        moved += record_transition(store, p, "built", "every ticket is done", now)
    return moved
