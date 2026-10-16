#!/usr/bin/env python3
"""List the factory's open pull requests that conflict with master.

    conflicts.py    print a JSON list of {"number", "branch", "sha"}

Only same-repo, non-draft PRs on claude/ branches (the implementer's) are
listed. A PR is skipped if Claude already gave up on its conflicts for the
current head commit: a comment containing <!-- conflict-stop --> written
after that commit. Push a new commit (or merge master yourself) and it's
eligible again. With ONLY_PR=<n> (the workflow's `pr` input), only that PR
is considered, and an earlier stop doesn't skip it.

GitHub works out mergeability in the background after master moves, so a
PR whose state is still unknown is asked again a few times.
"""
import json
import os
import subprocess
import sys
import time

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


def main():
    only = os.environ.get("ONLY_PR", "").strip().lstrip("#")
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
        head_time = gh(f"repos/{REPO}/commits/{sha}")["commit"]["committer"]["date"]
        stopped = any(STOP in (c.get("body") or "") and c["created_at"] > head_time
                      for c in gh(f"repos/{REPO}/issues/{n}/comments?per_page=100"))
        if stopped and not only:
            print(f"#{n}: conflicts, but Claude already stopped on this commit; leaving it", file=sys.stderr)
            continue
        print(f"#{n}: conflicts with master", file=sys.stderr)
        out.append({"number": n, "branch": ref, "sha": sha})
    print(json.dumps(out))


if __name__ == "__main__":
    main()
