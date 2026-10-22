#!/usr/bin/env python3
"""Tick pass: park an open PR whose ticket is already closed.

    superseded(snapshot) -> [Action("supersede_pr", "pr#n", {"tickets": [..]})]

Two builds of one ticket can both get opened (a local session and the
factory, say). When one merges, its ticket closes and the other is left
open, conflicting with master and repeating work master already has. Any
open, non-draft PR on a `claude/` branch whose tickets (the branch's
`claude/issue-<n>-` number, else every "Closes #n") are all closed tickets
gets one comment, the `superseded` label and is turned into a draft, so
the conflict resolver and auto-merge leave it alone. The label is the
marker: marking the PR ready again keeps the label and the pass leaves it be.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_tick import Action  # noqa: E402
from tickets import ticket_numbers  # noqa: E402

LABEL = "superseded"


def comment(numbers):
    refs = ", ".join(f"#{n}" for n in numbers)
    return (f"{refs} {'is' if len(numbers) == 1 else 'are'} already closed, so this pull "
            "request is most likely superseded by the change that closed it. I've turned "
            f"it into a draft and labelled it `{LABEL}`. Close it if master already has "
            "the change; if part of it should still land, mark it ready for review "
            "(the label keeps the factory tick from drafting it again).")


def superseded(snap):
    by = {i["number"]: i for i in snap.issues}
    out = []
    for pr in snap.prs:
        if (not pr["head"].startswith("claude/") or pr.get("draft") or pr.get("merged")
                or LABEL in pr.get("labels", [])):
            continue
        numbers = sorted(ticket_numbers(pr["head"], pr.get("body")))
        if numbers and all(by.get(n, {}).get("kind") == "ticket"
                           and by[n].get("state") == "closed" for n in numbers):
            out.append(Action("supersede_pr", f"pr#{pr['number']}",
                              {"tickets": numbers, "comment": comment(numbers)},
                              f"its ticket{'s' if len(numbers) > 1 else ''} "
                              f"{', '.join(f'#{n}' for n in numbers)} closed"))
    return out
