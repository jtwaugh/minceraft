"""Shared pieces of the fix runs the tick starts: the marker comment that says
a fix was started on a head commit, and the task text a fix run is given.

    marker(9, sha, "nits") -> "<!-- factory-fix pr=9 sha=<sha> cause=nits -->"
    markers(comments)      -> [(pr, sha, cause)] from trusted accounts' comments
    nits_task(nits)       -> the task for a nits fix run
    parse_nits(body)      -> the items under a review's **Nits** heading
    verdict(comments, head) -> the reviewer's newest verdict, with its nits
    conflict_task(), check_task(name, log), blockers_task(blockers) -> the Blocked-fix tasks

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


HEADING = re.compile(r"^\s*(?:#+\s*\**|\*\*)\s*([A-Za-z][A-Za-z ]*?)\s*:?\s*\**:?\s*$")
PLAIN_HEADING = re.compile(r"^\s*(nits|notes|blockers|since last review)\s*:?\s*$", re.I)
ITEM = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s+(.*)")


def parse_nits(body):
    """The bullet or numbered items under the **Nits** heading, wrapped lines joined."""
    items, inside, gap = [], False, False
    for line in (body or "").splitlines():
        h = HEADING.match(line) or PLAIN_HEADING.match(line)
        if h and not ITEM.match(line):
            if inside:
                break
            inside = h.group(1).lower() == "nits"
            continue
        if not inside:
            continue
        m = ITEM.match(line)
        if m:
            items.append(m.group(1).strip())
            gap = False
        elif not line.strip():
            gap = True
        elif items and not gap:
            items[-1] += " " + line.strip()
    return items


def verdict(comments, head):
    """{kind: nits|ready|blocked, commit, nits, current} from the reviewer's
    newest verdict comment (`review_gate.latest_verdict`), or None. `commit` is
    the reviewed commit as written; `current` says whether it names `head`."""
    import review_gate
    found = review_gate.latest_verdict(comments)
    if found is None:
        return None
    kind, commit, c = found
    body = c.get("body") or ""
    if kind == "ready" and "nits" in review_gate.VERDICT.search(body).group(1).lower():
        kind = "nits"
    return {"kind": kind, "commit": commit, "nits": parse_nits(body) if kind == "nits" else [],
            "current": bool(commit and head.startswith(commit))}


def nits_task(nits):
    items = "\n".join(f"{n}. {t}" for n, t in enumerate(nits, 1))
    return ("The reviewer approved this PR with nits. Address each one below, or say "
            "briefly why you did not. Make no other changes.\n\n" + items)


def conflict_task():
    return ("This PR conflicts with master. Merge origin/master into the branch "
            "(git fetch, git merge; never rebase or force-push) and resolve the "
            "conflicts. Make no other changes.")


def check_task(name, excerpt):
    return (f"The check `{name}` failed on this PR, and failed again after a re-run. "
            "Fix the cause, run the checks in CLAUDE.md before pushing, and make no "
            f"other changes.\n\nLog excerpt:\n\n{excerpt}")


def blockers_task(blockers):
    items = "\n".join(f"{n}. {t}" for n, t in enumerate(blockers, 1))
    return ("The reviewer blocked this PR. Fix each blocker below, or reply saying "
            "why you think it is wrong and leave the code. Make no other changes.\n\n" + items)
