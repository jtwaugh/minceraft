#!/usr/bin/env python3
"""Timing maths for implementer runs: wall time and time to first commit.

Pure functions, no network. `runs` are `Claude implementer #N` run records,
dicts with `ticket`, `start`, `end` (ISO 8601, `Z` or offset) and
`conclusion`; `first_commits` maps a ticket number to the time of its first
commit on its `claude/issue-N-*` branch. `summarise` splits tickets into
those started before `cutoff` and those started on or after it and gives
each group's median wall time and median start-to-first-commit, in seconds.
Cancelled runs are ignored. A ticket's wall time is the sum of its runs; its
start is its earliest run's start. A ticket with no commit counts in
`no_commit` and is left out of the second median.
(docs/design/factory-orchestrator.md Phase D, Reconciliation note 8)

`report --cutoff <ISO time>` fetches the data with `gh api` and prints the
before/after table. Fetching lives in `fetch_runs` and `fetch_first_commits`,
which tests replace.
"""
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from statistics import median


def parse(s):
    t = datetime.fromisoformat(s.replace("Z", "+00:00"))
    return t if t.tzinfo else t.replace(tzinfo=timezone.utc)


def _group(tickets, first_commits):
    walls = [t["wall"] for t in tickets]
    to_commit = []
    for t in tickets:
        c = first_commits.get(t["ticket"])
        if c is not None:
            to_commit.append((parse(c) - t["start"]).total_seconds())
    return {
        "tickets": len(tickets),
        "median_wall": median(walls) if walls else None,
        "median_to_first_commit": median(to_commit) if to_commit else None,
        "no_commit": len(tickets) - len(to_commit),
    }


def summarise(runs, first_commits, cutoff):
    """{"before": {...}, "after": {...}} for tickets started before / on or after cutoff."""
    cut = parse(cutoff) if isinstance(cutoff, str) else cutoff
    per = {}
    for r in runs:
        if r.get("conclusion") == "cancelled":
            continue
        start, end = parse(r["start"]), parse(r["end"])
        t = per.setdefault(r["ticket"], {"ticket": r["ticket"], "start": start, "wall": 0.0})
        t["start"] = min(t["start"], start)
        t["wall"] += (end - start).total_seconds()
    tickets = list(per.values())
    return {
        "before": _group([t for t in tickets if t["start"] < cut], first_commits),
        "after": _group([t for t in tickets if t["start"] >= cut], first_commits),
    }


RUN_NAME = re.compile(r"^Claude implementer #(\d+)$")


def parse_runs(raw):
    """Run records from claude.yml workflow runs; runs not named for a ticket
    (or not finished) are ignored."""
    out = []
    for r in raw:
        m = RUN_NAME.match(r.get("display_title") or "")
        if not m or not r.get("run_started_at") or not r.get("updated_at"):
            continue
        out.append({"ticket": int(m.group(1)), "start": r["run_started_at"],
                    "end": r["updated_at"], "conclusion": r.get("conclusion")})
    return out


def _gh(*args):
    p = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
    if p.returncode:
        sys.exit(f"gh api {' '.join(args)} failed: {p.stderr.strip()}")
    return p.stdout


def fetch_runs():
    out = _gh("--paginate", "repos/{owner}/{repo}/actions/workflows/claude.yml/runs?per_page=100",
              "--jq", ".workflow_runs[]")
    return parse_runs([json.loads(line) for line in out.splitlines() if line.strip()])


def fetch_first_commits(tickets):
    """{ticket: time of the earliest commit on any claude/issue-N-* branch}."""
    found = {}
    for n in sorted(tickets):
        refs = json.loads(_gh(f"repos/{{owner}}/{{repo}}/git/matching-refs/heads/claude/issue-{n}-") or "[]")
        for ref in refs:
            name = ref["ref"].removeprefix("refs/heads/")
            cmp = json.loads(_gh(f"repos/{{owner}}/{{repo}}/compare/master...{name}"))
            dates = [c["commit"]["committer"]["date"] for c in cmp.get("commits", [])]
            if dates:
                found[n] = min([found[n], *dates], key=parse) if n in found else min(dates, key=parse)
    return found


def _fmt(sec):
    if sec is None:
        return "-"
    m = round(sec / 60)
    return f"{m // 60}h{m % 60:02d}m" if m >= 60 else f"{m}m"


def format_report(summary, cutoff):
    rows = [("", "tickets", "median wall", "median to first commit", "no commit")]
    for k in ("before", "after"):
        g = summary[k]
        rows.append((k, str(g["tickets"]), _fmt(g["median_wall"]),
                     _fmt(g["median_to_first_commit"]), str(g["no_commit"])))
    w = [max(len(r[i]) for r in rows) for i in range(5)]
    lines = [f"Implementer timing, split at {cutoff}"]
    lines += ["  ".join(c.ljust(w[i]) for i, c in enumerate(r)).rstrip() for r in rows]
    return "\n".join(lines)


def report(cutoff, runs_fn=None, commits_fn=None):
    runs = (runs_fn or fetch_runs)()
    commits = (commits_fn or fetch_first_commits)({r["ticket"] for r in runs})
    return format_report(summarise(runs, commits, cutoff), cutoff)


def main(argv):
    if len(argv) != 3 or argv[0] != "report" or argv[1] != "--cutoff":
        sys.exit("usage: factory_timing.py report --cutoff <ISO time>")
    try:
        parse(argv[2])
    except ValueError:
        sys.exit(f"bad --cutoff: {argv[2]}")
    print(report(argv[2]))


if __name__ == "__main__":
    main(sys.argv[1:])
