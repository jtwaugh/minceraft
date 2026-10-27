"""Board pass of the factory tick: card Status, "Waiting on" and blocked-by links.

    board_pass(snapshot) -> [Action]

Works out what each card should show from the snapshot, using the rules in
`project_board.py` (imported, not copied), and compares it with the board items
the loader already read. It emits one action per difference and nothing for a
card that matches, so a second pass over the applied state is empty.
(docs/design/factory-orchestrator.md Phase A, design rule 6)

A ticket in the snapshot's state store takes its Status and "Waiting on" from
`factory_views.render` instead of the body and PR rules; a ticket not in the
store keeps those rules. (Phase B, reconciler notes 3, 4 and 8)

Extra snapshot inputs: board items {kind, number, state, status,
status_updated, waiting}; board_meta {"options", "waiting"} (empty: all
columns and the field exist); prs[] "status" = (status, why), "body", "files";
issues[] "body", "comment_blockers", "idea_unlabelled_at", "blocked_by_links"
(issue numbers already linked as blockers; absent means unknown).
"""
import os
import sqlite3
import sys

os.environ.setdefault("GITHUB_REPOSITORY", "")  # project_board and tickets read it on import
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_views  # noqa: E402
import project_board  # noqa: E402
import tickets  # noqa: E402
from factory_tick import Action  # noqa: E402

MANAGED = {"planned", "spec-question", "idea", "factory"}


def _target(kind, n):
    return f"{'issue' if kind == 'Issue' else 'pr'}#{n}"


def _stored_views(store):
    """factory_views tickets for every stored ticket; empty without a store or
    before ingest has made its tables."""
    if store is None:
        return {}
    try:
        return factory_views.render(store)["tickets"]
    except sqlite3.OperationalError:
        return {}


def _wanted(snap):
    """{(kind, number): (status, reason, waiting text)} for each managed card."""
    prs = [{"number": p["number"], "head": {"ref": p["head"]}, "body": p.get("body"),
            "files": p.get("files", []), "status": p.get("status")} for p in snap.prs]
    want, by_ticket = {}, {}
    for pr in prs:
        t = project_board.ticket_of(pr)
        if t and t not in by_ticket:
            by_ticket[t] = pr
        if pr["status"]:
            want[("PullRequest", pr["number"])] = (*pr["status"], "")
    open_numbers = {i["number"] for i in snap.issues if i["state"].lower() == "open"}
    plans = {i["number"] for i in snap.issues if i["kind"] == "plan"}
    stored = _stored_views(getattr(snap, "store", None))
    for i in snap.issues:
        labels = set(i["labels"])
        if i["state"].lower() != "open" or not labels & MANAGED or i["kind"] == "plan":
            continue
        key = ("Issue", i["number"])
        if "idea" in labels:
            want[key] = ("Idea", "labelled idea", "")
            continue
        view = stored.get(str(i["number"]))
        if view:
            want[key] = (view["status"], f"store: {view['state']}", view["waiting"])
            continue
        pr = by_ticket.get(i["number"])
        if pr:
            if pr["status"]:
                want[key] = (*pr["status"], "")
            continue
        status, why = project_board.issue_status(
            {"number": i["number"], "title": i["title"], "labels": [{"name": n} for n in labels]}, {})
        waits = []
        if "spec-question" not in labels:
            body = i.get("body") or i.get("body_excerpt") or ""
            deps = tickets.deps(body) | set(i.get("comment_blockers", []))
            waits = [f"#{d}" for d in sorted(deps - {i["number"]} - plans) if d in open_numbers]
            preds = tickets.predicted_files(body)
            waits += [f"PR #{p['number']}" for p in prs
                      if preds and not tickets.is_pr_for(i["number"], p)
                      and tickets.overlap(preds, p["files"])]
        if waits:
            why += f"; waiting on {', '.join(waits)}"
        want[key] = (status, why, ", ".join(waits))
    return want


def board_pass(snap):
    meta = snap.board_meta
    options = set(meta.get("options") or project_board.STATUSES + list(project_board.OPTIONAL))
    waiting_field = meta.get("waiting", True)
    want = _wanted(snap)
    unlabelled = {i["number"]: i.get("idea_unlabelled_at", "") for i in snap.issues}
    actions, seen = [], set()
    for it in snap.board:
        key = (it["kind"], it["number"])
        target = _target(*key)
        seen.add(key)
        if it["state"] in ("CLOSED", "MERGED"):
            status, why, text = "Done", "closed", ""
        elif key in want:
            status, why, text = want[key]
        else:
            continue  # a plan, or something the board pass doesn't manage
        status = status if status in options else project_board.OPTIONAL.get(status, status)
        if waiting_field and (it.get("waiting") or "") != text:
            actions.append(Action("set_waiting", target, {"text": text},
                                  f"waiting on {text or 'nothing'}"))
        current = it.get("status")
        if current == "Idea" and status != "Idea" and key[0] == "Issue" and it["state"] == "OPEN" \
                and (it.get("status_updated") or "") > unlabelled.get(it["number"], ""):
            actions += [Action("add_label", target, {"label": "idea"}, "moved to Idea on the board"),
                        Action("remove_label", target, {"label": "factory"}, "moved to Idea on the board")]
        elif current != status:
            actions.append(Action("set_status", target, {"status": status}, f"{current} -> {status} ({why})"))
    for key, (status, why, text) in sorted(want.items(), key=lambda kv: kv[0][1]):
        if key not in seen:
            status = status if status in options else project_board.OPTIONAL.get(status, status)
            actions.append(Action("add_card", _target(*key), {"status": status, "waiting": text},
                                  f"added as {status} ({why})"))
    return actions


def load_board(snap, login, number):
    """Read the project once into snap.board and snap.board_meta, and fill what
    the passes need from GitHub: PR status and files, issue comment blockers and
    the time `idea` was last taken off a card now showing Idea."""
    project, status_field, options, items, waiting_field = project_board.load_project(login, number)
    # load_project aliases a missing OPTIONAL column to its fallback's id; those aren't real columns
    real = [n for n, oid in options.items()
            if not (n in project_board.OPTIONAL and oid == options.get(project_board.OPTIONAL[n]))]
    snap.board_meta = {"loaded": True, "number": number, "project": project,
                       "status_field": status_field, "waiting_field": waiting_field,
                       "options": real, "option_ids": {n: options[n] for n in real},
                       "waiting": bool(waiting_field)}
    repo = project_board.REPO
    snap.board = []
    for node in items:
        c = node.get("content") or {}
        if not c or (repo and c["repository"]["nameWithOwner"] != repo):
            continue
        status, waiting = node.get("status") or {}, node.get("waiting") or {}
        snap.board.append({"item": node["id"], "kind": c["__typename"], "number": c["number"], "state": c["state"],
                           "status": status.get("name"), "status_updated": status.get("updatedAt"),
                           "waiting": waiting.get("text")})
    for p in snap.prs:
        pr = {"number": p["number"], "head": {"ref": p["head"], "sha": p.get("sha", "")}}
        p["status"] = project_board.pr_status(pr)
        p["files"] = tickets.pr_files(pr)
    shown_idea = {c["number"] for c in snap.board if c["kind"] == "Issue" and c["status"] == "Idea"}
    for i in snap.issues:
        if i["state"].lower() != "open" or i["kind"] != "ticket":
            continue
        i["comment_blockers"] = sorted({b for b, _ in tickets.comment_blockers(i["number"])})
        if i["number"] in shown_idea:
            i["idea_unlabelled_at"] = project_board.unlabelled_idea_at(i["number"])


def blocked_by_pass(snap):
    """One add_blocked_by per declared dependency (body plus comment blockers)
    that isn't linked yet. Issues without `blocked_by_links` are skipped."""
    known = {i["number"] for i in snap.issues}
    actions = []
    for i in snap.issues:
        if i["state"].lower() != "open" or i["kind"] != "ticket" or "blocked_by_links" not in i:
            continue
        deps = tickets.deps(i.get("body") or i.get("body_excerpt") or "") | set(i.get("comment_blockers", []))
        for d in sorted((deps - {i["number"]}) & known - set(i["blocked_by_links"])):
            actions.append(Action("add_blocked_by", f"issue#{i['number']}", {"blocker": d},
                                  f"declares a dependency on #{d}"))
    return actions
