"""Shared pieces of the fix runs the tick starts: the marker comment that says
a fix was started on a head commit, and the task text a fix run is given.

    marker("nits", sha)   -> "<!-- factory-fix kind=nits head=<sha> -->"
    markers(comments)     -> [(kind, head)] from trusted accounts' comments
    nits_task(nits)       -> the task for a nits fix run

The task comes from the tick, never from a comment (design rule 5; #463).
(docs/design/factory-orchestrator.md Phase A, section 3 "Nits")
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

MARKER = re.compile(r"<!-- factory-fix kind=(\w+) head=([0-9a-f]+) -->")


def marker(kind, head):
    return f"<!-- factory-fix kind={kind} head={head} -->"


def markers(comments):
    """(kind, head) for each fix marker in the given (already trusted) comments."""
    return [m.groups() for c in comments for m in MARKER.finditer(c.get("body") or "")]


def nits_task(nits):
    items = "\n".join(f"{n}. {t}" for n, t in enumerate(nits, 1))
    return ("The reviewer approved this PR with nits. Address each one below, or say "
            "briefly why you did not. Make no other changes.\n\n" + items)
