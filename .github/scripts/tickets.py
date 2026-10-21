#!/usr/bin/env python3
"""Ticket dependency checks for the implementer workflow.

  tickets.py blocked <n>   Print a JSON list of issue numbers that ticket #n
                           still waits on (open dependencies). [] if ready.
  tickets.py ready         Print a JSON list of open `factory` tickets whose
                           dependencies have all closed, that have no open
                           pull request yet, and whose files no open pull
                           request (or other ticket in the list) is editing.
  tickets.py check <n>     Print true if ticket #n would be in `ready` now,
                           else false.
  tickets.py has-pr <n>    Print true if ticket #n has an open pull request
                           (claude.yml's check on a run the tick dispatched).

The factory tick decides what to start (factory_ready.py, which reuses the
pure helpers here); `ready` and `check` print the same rules from the API.

A ticket's dependencies are every issue number on its "**Depends on:**"
line, plus any "Blocked by #n" in its body (the planner writes both) or in
a comment from the maintainer or an agent (the implementer writes one when
it stops on a question it raised mid-run). A dependency closed as "not
planned" counts as closed. Plan issues are never tickets.

Each ticket is started automatically once per set of blockers: a
`factory-auto-start` comment marks a start, and it is started again only if
a new "Blocked by" comment came after that start and those issues are now
closed. Otherwise relabel it or comment @claude to retry.

To keep branches from conflicting, a ticket also waits while an open pull
request edits a file the ticket expects to change: the backticked file names
in its body (the planner's **Files:** line, or "Likely files: `walk.rs`").
Files every feature touches (lib.rs, mod.rs, Cargo.toml, Cargo.lock) don't
count; conflicts there are trivial and resolved after each merge
(resolve-conflicts.yml). When that pull request merges, the next factory
tick starts the ticket.
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


def fetch(n, cache={}):
    if n not in cache:
        cache[n] = gh(f"repos/{REPO}/issues/{n}")
    return cache[n]


def state(n):
    return fetch(n)["state"]


def open_deps(issue):
    """Open issues this ticket waits on. A "Plan:" issue named as a
    dependency is ignored: the plan closes only after its tickets do, so
    waiting on it would never end."""
    all_deps = deps(issue.get("body")) | {b for b, _ in comment_blockers(issue["number"])}
    return sorted(d for d in all_deps if d != issue["number"] and state(d) != "closed"
                  and not fetch(d)["title"].startswith("Plan:"))


FILE = re.compile(r"`([\w./-]+\.(?:rs|js|mjs|toml|py|html|css|json|sh|yml|input))`")
SHARED = re.compile(r"(^|/)(lib\.rs|mod\.rs|main\.rs|Cargo\.toml|Cargo\.lock)$")


def predicted_files(body):
    """File names or paths a ticket says it will change."""
    return {f.lstrip("./") for f in FILE.findall(body or "") if not SHARED.search(f)}


def touches(pred, path):
    if "/" in pred:
        return path == pred or path.endswith("/" + pred)
    return path.rsplit("/", 1)[-1] == pred


def overlap(preds, paths):
    return sorted({p for p in paths for q in preds if touches(q, p)})


def pr_files(pr, cache={}):
    n = pr["number"]
    if n not in cache:
        cache[n] = [f["filename"] for f in gh(f"repos/{REPO}/pulls/{n}/files?per_page=100")]
    return cache[n]


def is_pr_for(n, pr):
    pat = re.compile(rf"\b(close[sd]?|fix(e[sd])?|resolve[sd]?) #{n}\b", re.I)
    return pr["head"]["ref"].startswith(f"claude/issue-{n}-") or bool(pat.search(pr.get("body") or ""))


def busy_prs(ticket, prs):
    """Open PRs (not this ticket's) editing a file this ticket expects to change."""
    preds = predicted_files(ticket.get("body"))
    if not preds:
        return []
    out = []
    for pr in prs:
        if is_pr_for(ticket["number"], pr):
            continue
        files = overlap(preds, pr_files(pr))
        if files:
            print(f"#{ticket['number']}: PR #{pr['number']} edits {', '.join(files)}", file=sys.stderr)
            out.append(pr["number"])
    return out


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


def open_prs():
    return gh(f"repos/{REPO}/pulls?state=open&per_page=100")


def ready_tickets(prs):
    tickets = [i for i in gh(f"repos/{REPO}/issues?state=open&labels=factory&per_page=100")
               if "pull_request" not in i and not i["title"].startswith("Plan:")
               and "idea" not in {l["name"] for l in i.get("labels", [])}]
    ready, claimed = [], {}
    for t in sorted(tickets, key=lambda t: t["number"]):
        n = t["number"]
        waiting = open_deps(t)
        if waiting:
            print(f"#{n}: waiting on {waiting}", file=sys.stderr)
            continue
        if has_open_pr(n, prs):
            print(f"#{n}: already has an open PR", file=sys.stderr)
            continue
        if auto_started(n):
            print(f"#{n}: already started automatically once; relabel or @claude to retry",
                  file=sys.stderr)
            continue
        busy = busy_prs(t, prs)
        if busy:
            print(f"#{n}: waiting for PR(s) {busy} to merge (same files)", file=sys.stderr)
            continue
        preds = predicted_files(t.get("body"))
        clash = [m for m, other in claimed.items() if overlap(other, preds) or overlap(preds, other)]
        if clash:
            print(f"#{n}: shares files with #{clash[0]}, which starts first", file=sys.stderr)
            continue
        print(f"#{n}: ready", file=sys.stderr)
        ready.append(n)
        claimed[n] = preds
    return ready


def main():
    mode = sys.argv[1]
    if mode == "blocked":
        issue = gh(f"repos/{REPO}/issues/{sys.argv[2]}")
        print(json.dumps(open_deps(issue) + busy_prs(issue, open_prs())))
    elif mode == "ready":
        print(json.dumps(ready_tickets(open_prs())))
    elif mode == "has-pr":
        print("true" if has_open_pr(int(sys.argv[2]), open_prs()) else "false")
    elif mode == "check":
        n = int(sys.argv[2])
        print("true" if n in ready_tickets(open_prs()) else "false")
    else:
        sys.exit(f"unknown mode {mode}")


if __name__ == "__main__":
    main()
