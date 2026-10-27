"""Record decisions, runs and attempts from a tick snapshot into the store.

`sync_records(snap, now)` adds, for stored tickets only and alongside the
markers the other passes still read:

- a `decisions` row for each `spec-question` issue in a stored plan that a
  trusted account answered with `/decide <option>` (the latest wins), linked
  to the stored ticket that names it ("Blocked by #q", or the question's own
  `ticket=` key), or to none; the same text from anyone else makes none;
- a `runs` row (role `implementer` or `fix`) with an `attempts` row each;
- a `fix_attempt` transition for each fix marker on the ticket's PR, with its
  cause and run, and a `rerun` transition for each check run with
  `run_attempt > 1`.

A second sync adds nothing. Returns the number of rows added.
(docs/design/factory-orchestrator.md Phase B)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_blocked import CAUSES, attempts  # noqa: E402
from factory_fix import markers  # noqa: E402
from factory_questions import ASKED  # noqa: E402
from factory_rerun import CHECKS  # noqa: E402
from factory_store_history import _TABLE, record_event  # noqa: E402
import tickets  # noqa: E402

DECIDE = re.compile(r"^\s*/decide\b[ \t]*(.*)$", re.M)
IMPLEMENTER = re.compile(r"Claude implementer #(\d+)")


def _count(store):
    return sum(store.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]
               for t in ("decisions", "runs", "attempts", "transitions"))


def _answer(issue):
    """The option in the latest trusted `/decide` comment, or None."""
    found = [m.group(1).strip() for c in issue.get("comments", [])
             if c.get("user") in tickets.TRUSTED for m in DECIDE.finditer(c.get("body") or "")]
    return found[-1] if found else None


def _implementing(snap, stored, q):
    """The stored ticket that waits on question q, else the one it names."""
    for t in snap.issues:
        if t["number"] not in stored or t.get("kind") != "ticket":
            continue
        texts = [t.get("body") or ""] + [c.get("body") or "" for c in t.get("comments", [])
                                           if c.get("user") in tickets.TRUSTED]
        for text in texts:
            for m in tickets.BLOCKED.finditer(text):
                if q["number"] in {int(x) for x in re.findall(r"#(\d+)", m.group(1))}:
                    return t["number"]
    named = ASKED.search(q.get("body") or "")
    return int(named.group(1)) if named and named.group(1) and int(named.group(1)) in stored else None


def _record_decisions(snap, plans, tix):
    for q in snap.issues:
        if q.get("kind") != "decision" or q.get("parent") not in plans:
            continue
        answer = _answer(q)
        if answer is None:
            continue
        snap.store.execute(
            "INSERT INTO decisions (issue, plan, answer, ticket) VALUES (?, ?, ?, ?) "
            "ON CONFLICT(issue) DO UPDATE SET answer = excluded.answer, ticket = excluded.ticket",
            (q["number"], q["parent"], answer, _implementing(snap, tix, q)))


def _run(store, run, ticket, role, outcome, n=1):
    store.execute("INSERT OR IGNORE INTO runs (id, ticket, role) VALUES (?, ?, ?)", (run, ticket, role))
    store.execute("INSERT INTO attempts (run, n, outcome) VALUES (?, ?, ?) "
                  "ON CONFLICT(run, n) DO UPDATE SET outcome = excluded.outcome", (run, n, outcome))


def _event(store, ticket, kind, cause, now):
    """record_event unless this exact event is already logged."""
    if not store.execute("SELECT 1 FROM transitions WHERE subject = ? AND kind = ? AND reason = ?",
                         (ticket, kind, cause)).fetchone():
        record_event(store, ticket, kind, cause, now)


def _record_pr(snap, tix, pr, ticket, now):
    store = snap.store
    comments = [c for c in pr.get("comments", []) if c.get("user") in tickets.TRUSTED]
    own = dict(pr, comments=comments)
    stamped = [(c.get("created_at"), m) for c in comments for m in markers([c]) if m[0] == pr["number"]]
    outcomes = iter(attempts(own))  # the retry budget counts only CAUSES markers
    for n, (when, (_, sha, cause)) in enumerate(stamped, 1):
        run = f"fix-pr{pr['number']}-{when}"
        _run(store, run, ticket, "fix", next(outcomes)[2] if cause in CAUSES else "recorded", n)
        _event(store, ticket, "fix_attempt", f"{cause} (run {run}, head {sha[:7]})", now)
    for r in pr.get("runs", []):
        if r["name"] in CHECKS and r.get("run_attempt", 1) > 1:
            _event(store, ticket, "rerun", f"{r['name']} (run {r['id']}, attempt {r['run_attempt']})", now)


def sync_records(snap, now):
    store = snap.store
    store.execute(_TABLE)
    before = _count(store)
    plans = {r[0] for r in store.execute("SELECT id FROM plans")}
    tix = {r[0] for r in store.execute("SELECT id FROM tickets")}
    with store:
        _record_decisions(snap, plans, tix)
        for r in snap.implementer_runs:
            m = IMPLEMENTER.search(r.get("display_title") or "")
            if m and int(m.group(1)) in tix:
                _run(store, f"implementer-{m.group(1)}-{r.get('created_at')}", int(m.group(1)),
                     "implementer", r.get("status") or "unknown")
        for pr in snap.prs:
            for n in sorted(tickets.ticket_numbers(pr.get("head", ""), pr.get("body")) & tix):
                _record_pr(snap, tix, pr, n, now)
    return _count(store) - before
