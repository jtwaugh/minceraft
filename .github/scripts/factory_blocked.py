#!/usr/bin/env python3
"""Tick pass: one fix run per Blocked ticket PR, head commit and cause.

Step 2 of the Blocked-ticket loop. For each ticket PR on a `claude/` branch the
loader stores `mergeable_state`, `head_time` and `head_author`, the runs (a
check that failed after its re-run carries a log excerpt in `run["log"]`),
`blockers` (the numbered blockers of the reviewer's newest verdict when it is
Blocked about the head commit and no `/override` followed it), `claude_mentions`
(times of the maintainer's `@claude` comments) and `comments` (trusted ones).
The cause is the first of: conflict with `master`; a check that failed again
(`run_attempt > 1`, see factory_rerun.py); a reviewer block. A failed check
still on its first attempt gets only the re-run. A maintainer push, or an
`@claude` comment after the head commit, counts as the attempt: no fix.
Retry budget: each marker is a `fix_attempt` (cause, head, outcome). A later
head means the fix pushed; the same head with `pr["fix_finished"]` listing it
means the run ended and pushed nothing. After `RETRY_BUDGET` attempts that
ended Blocked, or a run that pushed nothing, no fix starts and the pass opens
(then updates) one `needs-you` question per PR and cause. A PR that is no
longer Blocked has no attempts.
(docs/design/factory-orchestrator.md Phase A, section 3 "Blocked tickets")
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_config import RETRY_BUDGET  # noqa: E402
from factory_fix import blockers_task, check_task, conflict_task, markers  # noqa: E402
from factory_questions import has_open_question  # noqa: E402
from factory_rerun import failed_runs, is_ticket_pr, ticket_of  # noqa: E402
from factory_tick import Action  # noqa: E402

MAINTAINER = "jtwaugh"
CAUSES = ("conflict", "check", "review")
SECTION = re.compile(r"\*\*Blockers\*\*:?(.*?)(?=\*\*(?:Nits|Notes)\*\*|\Z)", re.S | re.I)
ITEM = re.compile(r"^\s*\d+[.)]\s+(.*?)(?=^\s*\d+[.)]\s|\Z)", re.S | re.M)


def parse_blockers(body):
    """The reviewer's numbered blockers, each as one line of text."""
    m = SECTION.search(body or "")
    return [" ".join(i.split()) for i in ITEM.findall(m.group(1))] if m else []


def cause_of(snap, pr):
    """(cause, task, what blocks) for a Blocked PR, or None."""
    if pr.get("mergeable_state") == "dirty":
        return "conflict", conflict_task(), "The branch conflicts with `master`."
    again = [r for p, r in failed_runs(snap) if p is pr and r["run_attempt"] > 1]
    if again:
        return ("check", check_task(again[0]["name"], again[0].get("log", "")),
                f"The check `{again[0]['name']}` failed again after a re-run.")
    if pr.get("blockers"):
        return "review", blockers_task(pr["blockers"]), "\n".join(f"> {b}" for b in pr["blockers"])
    return None


def maintainer_working(pr):
    return (pr.get("head_author") == MAINTAINER
            or any(t > pr.get("head_time", "") for t in pr.get("claude_mentions", [])))


def attempts(pr):
    """The `fix_attempt` records of a PR: (cause, head, outcome) per fix marker.
    A marker whose run was cancelled, timed out or never started
    (`pr["fix_lost"]`, by marker time) is "run failed": it counts against the
    budget but, unlike "pushed nothing", doesn't stop the next attempt."""
    heads = [(c.get("created_at"), sha, cause) for c in pr.get("comments", [])
             for n, sha, cause in markers([c]) if n == pr["number"] and cause in CAUSES]
    lost = pr.get("fix_lost", [])
    return [(c, sha, "run failed" if t in lost
             else "pushed a new head" if sha != pr["sha"] or i < len(heads) - 1
             else "pushed nothing" if sha in pr.get("fix_finished", []) else "running")
            for i, (t, sha, c) in enumerate(heads)]


def question_key(pr_number, cause):
    return f"<!-- factory-question pr={pr_number} cause=retry-{cause} -->"


def question(snap, pr, cause, quote, tried):
    items = "\n".join(f"{n}. {c} fix on `{sha[:7]}`: {out}" for n, (c, sha, out) in enumerate(tried, 1))
    key = question_key(pr["number"], cause)
    body = (f"{key}\nThe retry budget is spent on ticket #{ticket_of(snap, pr)}, PR #{pr['number']} "
            f"(head `{pr['sha'][:7]}`, blocked by {cause}).\n\nThe blocker:\n\n{quote}\n\n"
            f"What each attempt tried:\n\n{items}\n\nOptions:\n\n"
            "- A: fix it by hand and push to the branch.\n"
            "- B: if the blocker is wrong, comment `/override <reason>`.\n"
            "- C: close the PR and the ticket restarts from scratch.\n")
    title = f"PR #{pr['number']}: retry budget spent ({cause})"
    found = next((i for i in snap.issues if key in i["body"]), None)
    if found is None:
        return Action("open_question", f"issue#{ticket_of(snap, pr)}", {
            "label": "needs-you", "key": key, "title": title, "body": body},
            f"retry budget spent on {pr['sha'][:7]}; asking")
    if found["state"] == "open" and found["body"] != body:
        return Action("update_issue", f"issue#{found['number']}", {"body": body},
                      f"updating the retry-budget question for PR #{pr['number']}")


def blocked(snap):
    out = []
    for pr in snap.prs:
        if not (pr["head"].startswith("claude/") and is_ticket_pr(snap, pr)) or maintainer_working(pr):
            continue
        found = cause_of(snap, pr)
        if found is None or has_open_question(snap, pr):
            continue
        cause, task, quote = found
        tried = attempts(pr)
        if any(sha == pr["sha"] and o == "running" for _, sha, o in tried):
            continue
        if len(tried) >= RETRY_BUDGET or any(o == "pushed nothing" for _, _, o in tried):
            q = question(snap, pr, cause, quote, tried)
            if q:
                out.append(q)
            continue
        out.append(Action("start_fix", f"pr#{pr['number']}", {
            "kind": cause, "head": pr["sha"], "task": task},
            f"Blocked on {pr['sha'][:7]} ({cause}); starting a fix"))
    return out
