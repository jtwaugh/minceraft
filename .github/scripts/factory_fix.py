"""Shared pieces of the fix runs the tick starts: the marker comment that says
a fix was started on a head commit, and the task text a fix run is given.

    marker("nits", sha)   -> "<!-- factory-fix kind=nits head=<sha> -->"
    markers(comments)     -> [(kind, head)] from trusted accounts' comments
    nits_task(nits)       -> the task for a nits fix run
    verdict(comments)     -> the reviewer's newest verdict, as the loader stores it

The task comes from the tick, never from a comment (design rule 5; #463).
(docs/design/factory-orchestrator.md Phase A, section 3 "Nits")
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

MARKER = re.compile(r"<!-- factory-fix kind=(\w+) head=([0-9a-f]+) -->")
NITS_VERDICT = re.compile(r"Verdict:\s*\**\s*ready,? with nits", re.I)
NITS_HEADING = re.compile(r"^\W*nits\b\W*$", re.I | re.M)
ITEM = re.compile(r"^\s*(?:[-*]|\d+[.)])\s+(.*\S)")


def marker(kind, head):
    return f"<!-- factory-fix kind={kind} head={head} -->"


def markers(comments):
    """(kind, head) for each fix marker in the given (already trusted) comments."""
    return [m.groups() for c in comments for m in MARKER.finditer(c.get("body") or "")]


def parse_nits(body):
    """The bullet or numbered items under the reviewer's **Nits** heading."""
    m = NITS_HEADING.search(body)
    if not m:
        return []
    out = []
    for line in body[m.end():].splitlines():
        item = ITEM.match(line)
        if item:
            out.append(item.group(1))
        elif out and line.startswith((" ", "\t")) and line.strip():
            out[-1] += " " + line.strip()  # a wrapped item
    return out


def verdict(raw_comments):
    """The reviewer's newest verdict comment as {kind, commit, nits}, or None.
    kind is "nits", "ready" or "blocked"; commit is the reviewed sha as written."""
    import review_gate
    found = review_gate.latest_verdict(raw_comments)
    if found is None:
        return None
    kind, commit, comment = found
    body = comment.get("body") or ""
    if kind == "ready" and NITS_VERDICT.search(body):
        return {"kind": "nits", "commit": commit, "nits": parse_nits(body)}
    return {"kind": kind, "commit": commit, "nits": []}


def nits_task(nits):
    items = "\n".join(f"{n}. {t}" for n, t in enumerate(nits, 1))
    return ("The reviewer approved this PR with nits. Address each one below, or say "
            "briefly why you did not. Make no other changes.\n\n" + items)
