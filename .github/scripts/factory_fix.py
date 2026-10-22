"""Shared pieces of the fix runs the tick starts: the marker comment that says
a fix was started on a head commit, and the task text a fix run is given.

    marker(9, sha, "nits") -> "<!-- factory-fix pr=9 sha=<sha> cause=nits -->"
    markers(comments)      -> [(pr, sha, cause)] from trusted accounts' comments
    nits_task(nits)       -> the task for a nits fix run

The task comes from the tick, never from a comment (design rule 5; #463).
(docs/design/factory-orchestrator.md Phase A, section 3 "Nits")
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

MARKER = re.compile(r"<!-- factory-fix pr=(\d+) sha=([0-9a-f]+) cause=(\w+) -->")


def marker(pr, sha, cause):
    return f"<!-- factory-fix pr={pr} sha={sha} cause={cause} -->"


def markers(comments):
    """(pr, sha, cause) for each fix marker in the given (already trusted) comments."""
    return [(int(m[1]), m[2], m[3]) for c in comments for m in MARKER.finditer(c.get("body") or "")]


def nits_task(nits):
    items = "\n".join(f"{n}. {t}" for n, t in enumerate(nits, 1))
    return ("The reviewer approved this PR with nits. Address each one below, or say "
            "briefly why you did not. Make no other changes.\n\n" + items)
