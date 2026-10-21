"""Board pass of the factory tick: card Status, "Waiting on" and blocked-by links.

    board_pass(snapshot) -> [Action]

Works out what each card should show from the snapshot, using the rules in
`project_sync.py` (imported, not copied), and compares it with the board items
the loader already read. It emits one action per difference and nothing for a
card that matches, so a second pass over the applied state is empty.
(docs/design/factory-orchestrator.md Phase A, design rule 6)

Extra snapshot inputs: board items {kind, number, state, status,
status_updated, waiting}; board_meta {"options", "waiting"} (empty: all
columns and the field exist); prs[] "status" = (status, why), "body", "files";
issues[] "body", "comment_blockers", "idea_unlabelled_at".
"""
import os
import sys

os.environ.setdefault("GITHUB_REPOSITORY", "")  # project_sync and tickets read it on import
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import project_sync  # noqa: E402
import tickets  # noqa: E402
from factory_tick import Action  # noqa: E402

MANAGED = {"planned", "spec-question", "idea", "factory"}


def _target(kind, n):
    return f"{'issue' if kind == 'Issue' else 'pr'}#{n}"


def _wanted(snap):
    """{(kind, number): (status, reason, waiting text)} for each managed card."""
    prs = [{"number": p["number"], "head": {"ref": p["head"]}, "body": p.get("body"),
            "files": p.get("files", []), "status": p.get("status")} for p in snap.prs]
    want, by_ticket = {}, {}
    for pr in prs:
        t = project_sync.ticket_of(pr)
        if t and t not in by_ticket:
            by_ticket[t] = pr
        if pr["status"]:
            want[("PullRequest", pr["number"])] = (*pr["status"], "")
    open_numbers = {i["number"] for i in snap.issues if i["state"].lower() == "open"}
    plans = {i["number"] for i in snap.issues if i["kind"] == "plan"}
    for i in snap.issues:
        labels = set(i["labels"])
        if i["state"].lower() != "open" or not labels & MANAGED or i["kind"] == "plan":
            continue
        key = ("Issue", i["number"])
        if "idea" in labels:
            want[key] = ("Idea", "labelled idea", "")
            continue
        pr = by_ticket.get(i["number"])
        if pr:
            if pr["status"]:
                want[key] = (*pr["status"], "")
            continue
        status, why = project_sync.issue_status(
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
    options = set(meta.get("options") or project_sync.STATUSES + list(project_sync.OPTIONAL))
    waiting_field = meta.get("waiting", True)
    want = _wanted(snap)
    actions, seen = [], set()
    for it in snap.board:
        key = (it["kind"], it["number"])
        target = _target(*key)
        seen.add(key)        if it["state"] in ("CLOSED", "MERGED"):
            status, why, text = "Done", "closed", ""
        elif key in want:
            status, why, text = want[key]
        else:
            continue  # a plan, or something the board pass doesn't manage
        status = status if status in options else project_sync.OPTIONAL.get(status, status)
        if waiting_field and (it.get("waiting") or "") != text:
            actions.append(Action("set_waiting", target, {"text": text},
                                  f"waiting on {text or 'nothing'}"))
        current = it.get("status")
        if current == "Idea" and status != "Idea" and key[0] == "Issue" and it["state"] == "OPEN" \
                and (it.get("status_updated") or "") > next((i for i in snap.issues if i["number"] == it["number"]), {}).get("idea_unlabelled_at", ""):
            actions += [Action("add_label", target, {"label": "idea"}, "moved to Idea on the board"),
                        Action("remove_label", target, {"label": "factory"}, "moved to Idea on the board")]
        elif current != status:
            actions.append(Action("set_status", target, {"status": status}, f"{current} -> {status} ({why})"))
    for key, (status, why, text) in sorted(want.items(), key=lambda kv: kv[0][1]):
        if key not in seen:
            status = status if status in options else project_sync.OPTIONAL.get(status, status)
            actions.append(Action("add_card", _target(*key), {"status": status, "waiting": text},
                                  f"added as {status} ({why})"))
    return actions
