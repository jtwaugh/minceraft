"""Ready-ticket pass: which `factory` tickets should be started now.

`ready(snapshot) -> [Action("start_ticket", "issue#n")]`, over a `Snapshot`
and no live calls. The rules are the ones `tickets.py` documents (its pure
helpers are reused; a `local` ticket is never started), with one change: a ticket that depends on a plan, or
whose "Depends on" line can't be read, is never started, and is left for
the invariants report.

Fields read from the snapshot, besides those the skeleton loads: an issue's
`body` (else `body_excerpt`) and `comments` (`user`, `body`, `created_at`);
a PR's `head`, `body`, `files` and `state`.
(docs/design/factory-orchestrator.md Phase A, "One tick")
"""
import os
import re
import sqlite3

os.environ.setdefault("GITHUB_REPOSITORY", "unused/unused")  # tickets.py reads it at import
import tickets  # noqa: E402
from factory_tick import Action  # noqa: E402

MARKER = "<!-- factory-auto-start -->"


def _body(issue):
    return issue.get("body") or issue.get("body_excerpt") or ""


def _trusted(issue):
    return [c for c in issue.get("comments", []) if c.get("user") in tickets.TRUSTED]


def _comment_blockers(issue):
    """(number, created_at) for each "Blocked by #x" in trusted comments."""
    return [(int(x), c.get("created_at", ""))
            for c in _trusted(issue)
            for m in tickets.BLOCKED.finditer(c.get("body") or "")
            for x in re.findall(r"#(\d+)", m.group(1))]


def _stored(store):
    """{ticket: (dependencies, predicted files)} for every stored ticket; empty
    without a store or before ingest has made its tables."""
    if store is None:
        return {}
    try:
        out = {r[0]: (set(), set()) for r in store.execute("SELECT id FROM tickets")}
        for t, d in store.execute("SELECT ticket, depends_on FROM dependencies"):
            out[t][0].add(d)
        for t, f in store.execute("SELECT ticket, path FROM predicted_files"):
            out[t][1].add(f)
    except sqlite3.OperationalError:
        return {}
    return out


def _waiting_on(issue, by_number, stored=None):
    """Dependencies that are not closed (unknown issues count as open), and
    any plan among them. A stored ticket's dependencies come from the store,
    not from its body."""
    n = issue["number"]
    own = (tickets.deps(_body(issue)) | set(issue.get("depends_on", []))) if stored is None else stored
    wanted = (own | {b for b, _ in _comment_blockers(issue)}) - {n}
    open_ = sorted(d for d in wanted if by_number.get(d, {}).get("state") != "closed")
    plans = sorted(d for d in wanted if by_number.get(d, {}).get("kind") == "plan")
    return open_, plans


def _auto_started(issue):
    starts = [c.get("created_at", "") for c in _trusted(issue) if MARKER in (c.get("body") or "")]
    if not starts:
        return False
    return max((t for _, t in _comment_blockers(issue)), default="") < max(starts)


def _as_pr(pr):
    return {"head": {"ref": pr.get("head", "")}, "body": pr.get("body"),
            "number": pr.get("number"), "files": pr.get("files", [])}


def ready(snap):
    by_number = {i["number"]: i for i in snap.issues}
    prs = [_as_pr(p) for p in snap.prs if p.get("state", "open") == "open"]
    candidates = sorted(
        (i for i in snap.issues
         if i.get("state") == "open" and "factory" in i.get("labels", [])
         and i.get("kind") != "plan" and not i["title"].startswith("Plan:")
         and "idea" not in i["labels"]),
        key=lambda i: i["number"])
    actions, claimed = [], {}
    store = _stored(getattr(snap, "store", None))
    for t in candidates:
        n = t["number"]
        row = store.get(n)
        open_, plans = _waiting_on(t, by_number, row and row[0])
        if (open_ or plans or (row is None and tickets.depends_error(_body(t)))
                or tickets.has_open_pr(n, prs) or _auto_started(t)
                or tickets.LOCAL in t.get("labels", [])):
            continue
        preds = row[1] if row else tickets.predicted_files(_body(t))
        if preds and any(not tickets.is_pr_for(n, p) and tickets.overlap(preds, p["files"])
                         for p in prs):
            continue
        if any(tickets.overlap(other, preds) or tickets.overlap(preds, other)
               for other in claimed.values()):
            continue
        claimed[n] = preds
        actions.append(Action("start_ticket", f"issue#{n}", {}, "dependencies closed"))
    return actions
