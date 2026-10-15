#!/usr/bin/env python3
"""Ticket dependency checks for the implementer workflow.

  tickets.py blocked <n>   Print a JSON list of issue numbers that ticket #n
                           still waits on (open dependencies). [] if ready.
  tickets.py ready         Print a JSON list of open `factory` tickets whose
                           dependencies have all closed and that have no
                           open pull request yet.

A ticket's dependencies are every issue number on its "**Depends on:**"
line, plus any "Blocked by #n" in its body (the planner writes both) or in
a comment from the maintainer or an agent (the implementer writes one when
it stops on a question it raised mid-run). A dependency closed as "not
planned" counts as closed. Plan issues are never tickets.

Each ticket is started automatically once per set of blockers: a
`factory-auto-start` comment marks a start, and it is started again only if
a new "Blocked by" comment came after that start and those issues are now
closed. Otherwise relabel it or comment @claude to retry.
"""
import json
import os
import re
import subprocess
import sys

REPO = os.environ["GITHUB_REPOSITORY"]


def gh(path):
    """GitHub REST call. Lists use per_page=100, enough for this repo."""
    out = subprocess.run(["gh", "api", path],
                         check=True, capture_output=True, text=True).stdout
    return json.loads(out)


TRUSTED = {"jtwaugh", "claude[bot]", "github-actions[bot]"}
BLOCKED = re.compile(r"[Bb]locked by ([^\n]*)")


def comments(n, cache={}):
    if n not in cache:
        cache[n] = [c for c in gh(f"repos/{REPO}/issues/{n}/comments?per_page=100")
                    if c["user"]["login"] in TRUSTED]
    return cache[n]


def comment_blockers(n):
    """(number, created_at) for each "Blocked by #x" in trusted comments."""
    out = []
    for c in comments(n):
        for m in BLOCKED.finditer(c.get("body") or ""):
            out += [(int(x), c["created_at"]) for x in re.findall(r"#(\d+)", m.group(1))]
    return out


def deps(body):
    body = body or ""
    found = set()
    for line in body.splitlines():
        if "Depends on:" in line:
            found.update(int(n) for n in re.findall(r"#(\d+)", line))
    for m in re.finditer(r"[Bb]locked by ([^\n]*)", body):
        found.update(int(n) for n in re.findall(r"#(\d+)", m.group(1)))
    return found


def state(n, cache={}):
    if n not in cache:
        cache[n] = gh(f"repos/{REPO}/issues/{n}")["state"]
    return cache[n]


def open_deps(issue):
    all_deps = deps(issue.get("body")) | {b for b, _ in comment_blockers(issue["number"])}
    return sorted(d for d in all_deps if d != issue["number"] and state(d) != "closed")


def has_open_pr(n, prs):
    pat = re.compile(rf"\b(close[sd]?|fix(e[sd])?|resolve[sd]?) #{n}\b", re.I)
    return any(pr["head"]["ref"].startswith(f"claude/issue-{n}-") or pat.search(pr.get("body") or "")
               for pr in prs)


def auto_started(n):
    """True if the ticket was auto-started and no new blocker was declared
    since: then it shouldn't be started automatically again."""
    starts = [c["created_at"] for c in comments(n)
              if "<!-- factory-auto-start -->" in (c.get("body") or "")]
    if not starts:
        return False
    newest_blocker = max((t for _, t in comment_blockers(n)), default="")
    return newest_blocker < max(starts)


def main():
    mode = sys.argv[1]
    if mode == "blocked":
        issue = gh(f"repos/{REPO}/issues/{sys.argv[2]}")
        print(json.dumps(open_deps(issue)))
    elif mode == "ready":
        tickets = [i for i in gh(f"repos/{REPO}/issues?state=open&labels=factory&per_page=100")
                   if "pull_request" not in i and not i["title"].startswith("Plan:")]
        prs = gh(f"repos/{REPO}/pulls?state=open&per_page=100")
        ready = []
        for t in tickets:
            waiting = open_deps(t)
            if waiting:
                print(f"#{t['number']}: waiting on {waiting}", file=sys.stderr)
            elif has_open_pr(t["number"], prs):
                print(f"#{t['number']}: already has an open PR", file=sys.stderr)
            elif auto_started(t["number"]):
                print(f"#{t['number']}: already started automatically once; relabel or @claude to retry",
                      file=sys.stderr)
            else:
                print(f"#{t['number']}: ready", file=sys.stderr)
                ready.append(t["number"])
        print(json.dumps(sorted(ready)))
    else:
        sys.exit(f"unknown mode {mode}")


if __name__ == "__main__":
    main()
