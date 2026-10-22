#!/usr/bin/env python3
"""List the factory's open pull requests that conflict with master.

    conflicts.py                 print a JSON list of {"number", "branch", "sha"}
    conflicts.py still <n> <sha> print true if PR #n should still be resolved at
                                 <sha> (the resolve job's check before Claude runs)

Only same-repo, non-draft PRs on claude/ branches (the implementer's) are
listed. A PR is skipped if Claude already gave up on its conflicts for the
current head commit: a comment containing <!-- conflict-stop --> written
after that commit. Push a new commit (or merge master yourself) and it's
eligible again. A PR labelled `superseded`, or whose tickets (its
`claude/issue-<n>-` number, else its "Closes #n") are all closed, is skipped
too: another PR already built that ticket, and the factory tick drafts this
one (factory_supersede.py). With ONLY_PR=<n> (the workflow's `pr` input), only that PR
is considered, and an earlier stop doesn't skip it.

GitHub works out mergeability in the background after master moves, so a
PR whose state is still unknown is asked again a few times.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tickets import ticket_numbers  # noqa: E402

REPO = os.environ["GITHUB_REPOSITORY"]
STOP = "<!-- conflict-stop -->"


def gh(path):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path],
                         check=True, capture_output=True, text=True).stdout
    pages = json.loads(out)
    if pages and isinstance(pages[0], list):
        return [item for page in pages for item in page]
    return pages[0] if pages else []


def mergeable_state(n, tries=6, wait=10):
    for i in range(tries):
        pr = gh(f"repos/{REPO}/pulls/{n}")
        if pr.get("mergeable") is not None and pr.get("mergeable_state") != "unknown":
            return pr
        time.sleep(wait)
    return pr


def stopped_on(n, sha):
    """Whether Claude already gave up on PR #n's conflicts at commit `sha`."""
    head_time = gh(f"repos/{REPO}/commits/{sha}")["commit"]["committer"]["date"]
    return any(STOP in (c.get("body") or "") and c["created_at"] > head_time
               for c in gh(f"repos/{REPO}/issues/{n}/comments?per_page=100"))


def superseded(pr):
    """Why PR `pr` is not worth resolving (another PR built its ticket), or None."""
    if "superseded" in {lb["name"] for lb in pr.get("labels") or []}:
        return "labelled superseded"
    numbers = sorted(ticket_numbers(pr["head"]["ref"], pr.get("body")))
    if numbers and all(gh(f"repos/{REPO}/issues/{t}")["state"] == "closed" for t in numbers):
        return f"its ticket {', '.join(f'#{t}' for t in numbers)} is already closed"
    return None


def still(n, sha, only):
    """Re-check one PR just before Claude runs on it. Several merges in a row
    each start a `find`, all before any stop comment exists, and their
    resolve jobs queue behind each other; only the first should run."""
    pr = gh(f"repos/{REPO}/pulls/{n}")
    why = ("closed" if pr["state"] != "open" else "a draft" if pr["draft"] else
           f"moved to {pr['head']['sha'][:7]}" if pr["head"]["sha"] != sha else
           superseded(pr) or
           ("already stopped on this commit" if not only and stopped_on(n, sha) else None))
    if why:
        print(f"#{n}: skipping, {why}", file=sys.stderr)
    return why is None


def main():
    only = os.environ.get("ONLY_PR", "").strip().lstrip("#")
    if sys.argv[1:2] == ["still"]:
        print("true" if still(int(sys.argv[2]), sys.argv[3], only) else "false")
        return
    out = []
    for pr in gh(f"repos/{REPO}/pulls?state=open&per_page=100"):
        n, ref = pr["number"], pr["head"]["ref"]
        if only and str(n) != only:
            continue
        if pr["draft"] or pr["head"]["repo"]["full_name"] != REPO or not ref.startswith("claude/"):
            continue
        full = mergeable_state(n)
        state = full.get("mergeable_state")
        if state != "dirty":
            print(f"#{n}: {state}", file=sys.stderr)
            continue
        sha = full["head"]["sha"]
        why = superseded(full)
        if why:
            print(f"#{n}: conflicts, but {why}; leaving it", file=sys.stderr)
            continue
        if not only and stopped_on(n, sha):
            print(f"#{n}: conflicts, but Claude already stopped on this commit; leaving it", file=sys.stderr)
            continue
        print(f"#{n}: conflicts with master", file=sys.stderr)
        out.append({"number": n, "branch": ref, "sha": sha})
    print(json.dumps(out))


if __name__ == "__main__":
    main()
