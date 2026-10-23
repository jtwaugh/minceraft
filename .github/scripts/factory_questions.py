#!/usr/bin/env python3
"""Tick pass: a fix run's stop marker becomes one question issue.

A fix run that cannot go on (ambiguous spec, disagreement with a reviewer
blocker, a conflict needing a real decision, a change outside the
implementation lane, a PR that cannot fit 200 lines) posts a comment
starting `<!-- factory-fix-stop pr=N sha=<head> kind=<decision|research|needs-you> -->`
followed by the question and its options. For each ticket PR the pass reads
the trusted comments for a marker naming the current head and emits one
`open_question`: `spec-question` plus `decision` or `research` when the spec
or a design doc is the problem, else `needs-you`. The issue is keyed by
ticket, PR and kind, so a later tick updates it (and never opens a second).
While it is open `has_open_question` is true and the Blocked-fix and nits
passes start nothing; `link_questions` links the ticket as blocked by it. A decision question starts with `**File:**` (the file named in the
run's text, else SPEC.md) as `planner_issue.py question` requires.
(docs/design/factory-orchestrator.md Phase A, section 3 "Blocked tickets" step 4)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_fix import stops  # noqa: E402
from factory_rerun import is_ticket_pr, ticket_of  # noqa: E402
from factory_tick import Action  # noqa: E402

ASKED = re.compile(r"<!-- factory-question (?:ticket=(\d+) )?(?:pr=(\d+) )?cause=")
FILE = re.compile(r"^\*\*File:\*\*\s*(\S.*)$", re.M)
CLOSING = {
    "decision": ("Answer by commenting `/decide <option>` with any reasoning; the spec editor "
                 "will draft the change to {file}."),
    "research": "Needs research from the maintainer's local spec lane.",
    "needs-you": "Answer in a comment; the ticket restarts once this is closed.",
}


def key_of(ticket, pr_number, kind):
    return f"<!-- factory-question ticket={ticket} pr={pr_number} cause=stop-{kind} -->"


def current_stop(pr):
    """(kind, text) of the newest stop marker naming this PR's head, or None."""
    found = [(k, t) for n, sha, k, t in stops(pr.get("comments", []))
             if n == pr["number"] and sha == pr["sha"]]
    return found[-1] if found else None


def has_open_question(snap, pr, cause=""):
    """True when a question (stop, retry budget or nits) for this PR is still open;
    `cause` narrows it to keys starting with that cause."""
    pattern = re.compile(rf"<!-- factory-question (?:ticket=\d+ )?pr={pr['number']} cause={cause}")
    return any(i["state"] == "open" and pattern.search(i.get("body") or "") for i in snap.issues)


def build(snap, pr, kind, text):
    ticket = ticket_of(snap, pr)
    key = key_of(ticket, pr["number"], kind)
    named = FILE.search(text)
    file = named.group(1).strip() if named else "SPEC.md"
    text = FILE.sub("", text).strip()
    top = f"**File:** {file}\n" if kind == "decision" else ""
    body = (f"{top}{key}\nThe fix run on ticket #{ticket}, PR #{pr['number']} (head "
            f"`{pr['sha'][:7]}`) stopped and needs an answer.\n\n{text}\n\n"
            f"{CLOSING[kind].format(file=file)}\n")
    labels = ["spec-question", kind] if kind != "needs-you" else ["needs-you"]
    first = next((ln.strip(" #>*") for ln in text.splitlines() if ln.strip()), "stopped")
    return key, {"label": labels[0], "labels": labels, "key": key, "body": body,
                 "title": f"Ticket #{ticket}, PR #{pr['number']}: {first[:80]}"}


def questions(snap):
    out = []
    for pr in snap.prs:
        if not (pr["head"].startswith("claude/") and is_ticket_pr(snap, pr)):
            continue
        stop = current_stop(pr)
        if stop is None:
            continue
        key, payload = build(snap, pr, *stop)
        found = next((i for i in snap.issues if key in (i.get("body") or "")), None)
        if found is None:
            out.append(Action("open_question", f"issue#{ticket_of(snap, pr)}", payload,
                              f"fix run stopped on {pr['sha'][:7]} ({stop[0]}); asking"))
        elif found["state"] == "open" and found["body"] != payload["body"]:
            out.append(Action("update_issue", f"issue#{found['number']}", {"body": payload["body"]},
                              f"updating the stop question for PR #{pr['number']}"))
    return out


def link_questions(snap):
    """One add_blocked_by per open question whose ticket isn't linked to it yet.

    The question's number exists only after it is opened, so the link follows on
    a later tick. A ticket without `blocked_by_links` (not loaded) is skipped."""
    out = []
    for q in snap.issues:
        m = ASKED.search(q.get("body") or "")
        if q["state"] != "open" or not m:
            continue
        if m.group(1):
            number = int(m.group(1))
        else:
            pr = next((p for p in snap.prs if p["number"] == int(m.group(2))), None)
            number = ticket_of(snap, pr) if pr else None
        ticket = next((i for i in snap.issues if i["number"] == number), None)
        if ticket is None or q["number"] in ticket.get("blocked_by_links", [q["number"]]):
            continue
        out.append(Action("add_blocked_by", f"issue#{number}", {"blocker": q["number"]},
                          f"linking ticket #{number} as blocked by question #{q['number']}"))
    return out
