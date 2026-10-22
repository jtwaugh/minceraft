#!/usr/bin/env python3
"""Tick pass: one nits fix per ticket PR, then at most one question.

For each ticket PR the loader stores `pr["verdict"]` (the reviewer's newest
verdict: kind, reviewed commit, nits) and `pr["comments"]` (comments from
trusted accounts). A head commit at "Ready, with nits" with no fix marker for
that head gets one `start_fix` whose task is the nit list. If the head is
still at nits after a nits fix was started on an earlier head, the pass opens
one `needs-you` question instead and never starts a second fix.
(docs/design/factory-orchestrator.md Phase A, section 3 "Nits"; rule 5)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_fix import markers, nits_task  # noqa: E402
from factory_rerun import CLOSES, is_ticket_pr  # noqa: E402
from factory_tick import Action  # noqa: E402

LABEL = "needs-you"


def question_key(pr_number, head):
    return f"<!-- factory-question pr={pr_number} cause=nits head={head} -->"


def ticket_number(snap, pr):
    m = re.match(r"claude/issue-(\d+)-", pr["head"])
    numbers = [int(m.group(1))] if m else [int(n) for n in CLOSES.findall(pr.get("body") or "")]
    return next(i["number"] for i in snap.issues
                if i["number"] in numbers and i["state"] == "open" and "factory" in i["labels"])


def question_body(pr, nits):
    items = "\n".join(f"{n}. {t}\n   - fix it, leave it, or make it a follow-up ticket?"
                      for n, t in enumerate(nits, 1))
    return (f"{question_key(pr['number'], pr['sha'])}\n"
            f"PR #{pr['number']} was fixed once for the reviewer's nits, and the re-review of "
            f"`{pr['sha'][:7]}` still says \"Ready, with nits\". Decide each remaining nit:\n\n"
            f"{items}\n")


def nits(snap):
    out = []
    for pr in snap.prs:
        v = pr.get("verdict")
        if not (is_ticket_pr(snap, pr) and v and v["kind"] == "nits"
                and v["commit"] and pr["sha"].startswith(v["commit"])):
            continue
        started = [head for kind, head in markers(pr.get("comments", [])) if kind == "nits"]
        target = f"pr#{pr['number']}"
        if pr["sha"] in started:
            continue
        if started:  # a fix on an earlier head, and nits remain
            key = question_key(pr["number"], pr["sha"])
            if not any(key in i["body"] for i in snap.issues if i["state"] == "open"):
                out.append(Action("open_question", f"issue#{ticket_number(snap, pr)}", {
                    "label": LABEL, "key": key,
                    "title": f"PR #{pr['number']}: nits remain after the fix",
                    "body": question_body(pr, v["nits"])},
                    f"nits remain on {pr['sha'][:7]} after a fix; asking"))
        else:
            out.append(Action("start_fix", target, {
                "kind": "nits", "head": pr["sha"], "task": nits_task(v["nits"])},
                f"Ready, with nits on {pr['sha'][:7]}; starting the nits fix"))
    return out
