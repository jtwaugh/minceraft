#!/usr/bin/env python3
"""Check that a Claude run left behind what it was asked to make.

A run of claude-code-action counts as a success whenever Claude exits
cleanly, even if it gave up halfway (for example after its commit or
`gh pr create` command was refused). These checks look for the result
itself, so a run that made nothing fails visibly instead.

    expect.py ticket N STARTED      an open PR for ticket N, or a comment
                                    from Claude on #N since STARTED
    expect.py phase-done PLAN STARTED
                                    a PR from docs/...-phase-done-PLAN
                                    opened since STARTED
    expect.py plan FILE STARTED     a "Plan:" issue for FILE created since
                                    STARTED
    expect.py review PR STARTED     a comment from Claude on PR #PR with a
                                    "Verdict:" and a "Reviewed commit:" line,
                                    written since STARTED

STARTED is an ISO 8601 UTC time (2026-10-16T03:17:05Z). Writes ok=true or
ok=false, and expected=<what was looked for>, to $GITHUB_OUTPUT (or stdout),
and always exits 0: the workflow decides what to do with a miss.
"""
import json
import os
import re
import subprocess
import sys

REPO = os.environ["GITHUB_REPOSITORY"]
BOT = "claude[bot]"


def gh(path):
    """GET a REST path, following pagination, as one flat list or object."""
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path],
                         check=True, capture_output=True, text=True).stdout
    pages = json.loads(out)
    if pages and isinstance(pages[0], list):
        return [item for page in pages for item in page]
    return pages[0] if pages else []


def ticket(n, started):
    closes = re.compile(rf"(?i)\b(close[sd]?|fix(e[sd])?|resolve[sd]?) #{n}\b")
    for pr in gh(f"repos/{REPO}/pulls?state=open&per_page=100"):
        if pr["head"]["ref"].startswith(f"claude/issue-{n}-") or closes.search(pr.get("body") or ""):
            return True
    comments = gh(f"repos/{REPO}/issues/{n}/comments?per_page=100&since={started}")
    return any(c["user"]["login"] == BOT and c["created_at"] >= started for c in comments)


def phase_done(plan, started):
    branch = re.compile(rf"^docs/.*phase-done-{plan}$")
    pulls = gh(f"repos/{REPO}/pulls?state=all&sort=created&direction=desc&per_page=50")
    return any(branch.match(p["head"]["ref"]) and p["created_at"] >= started for p in pulls)


def plan(path, started):
    marker = f"plan-source file={path} "
    issues = gh(f"repos/{REPO}/issues?state=all&labels=planned&since={started}&per_page=100")
    return any(i["title"].startswith("Plan:") and i["created_at"] >= started
               and marker in (i.get("body") or "") for i in issues)


def review(pr, started):
    comments = gh(f"repos/{REPO}/issues/{pr}/comments?per_page=100&since={started}")
    return any(c["user"]["login"] == BOT and "Verdict:" in (c.get("body") or "")
               and "Reviewed commit:" in (c.get("body") or "") for c in comments)


CHECKS = {
    "ticket": (ticket, "a pull request for #{0}, or a comment saying why it stopped"),
    "phase-done": (phase_done, "a pull request marking plan #{0}'s phase done"),
    "plan": (plan, "a Plan issue for {0}"),
    "review": (review, "a review comment with a verdict and the reviewed commit on #{0}"),
}


def main():
    kind, arg, started = sys.argv[1:4]
    fn, what = CHECKS[kind]
    ok = fn(arg, started)
    expected = what.format(arg)
    lines = [f"ok={'true' if ok else 'false'}", f"expected={expected}"]
    print(("Found " if ok else "Missing: ") + expected, file=sys.stderr)
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as f:
            f.write("\n".join(lines) + "\n")
    else:
        print("\n".join(lines))


if __name__ == "__main__":
    main()
