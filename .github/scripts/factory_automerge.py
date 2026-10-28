#!/usr/bin/env python3
"""Tick pass: merge a ticket PR the reviewer called "Ready", without a click.

    auto_merge(snapshot) -> [Action("merge_pr", "pr#n", {"sha": head})]

A ticket PR (a `claude/` branch for an open `factory` ticket) is merged when
all of these hold:

- the reviewer's newest verdict is plain "Ready" (not "Ready, with nits",
  not Blocked) and names the PR's head commit;
- GitHub says the PR is `clean`: no conflicts and every required check
  passed (CI, PR size, review-verdict);
- it touches no path in CODEOWNERS (SPEC.md, CLAUDE.md, .github/, design
  docs: those are the maintainer's to accept);
- it has no `hold` label, and isn't a draft;
- no other open PR is for the same ticket (two builds of one ticket wait for
  the maintainer to pick one; the invariants pass reports them);
- the newest CI run on master didn't fail (merging onto a broken master
  hides which change broke it).

The backend merges with `--match-head-commit`, so a commit pushed after the
verdict is never merged on it. Everything else (nits, overrides, the
maintainer's own PRs, spec and design PRs) still waits for the maintainer.
(FACTORY.md decision 1; docs/design/factory-orchestrator.md rule 7)
"""
import fnmatch
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_rerun import is_ticket_pr  # noqa: E402
from factory_tick import Action  # noqa: E402
os.environ.setdefault("GITHUB_REPOSITORY", "unused/unused")  # tickets.py reads it at import
from tickets import ticket_numbers  # noqa: E402

HOLD = "hold"
BOT = "claude[bot]"


def verdict_of(comments, head):
    """The reviewer's newest verdict as {"kind": ready|nits|blocked, "current":
    whether it names `head`}, or None. `comments` are REST issue comments."""
    import review_gate
    found = review_gate.latest_verdict(comments)
    if found is None:
        return None
    kind, commit, c = found
    if kind == "ready" and review_gate.has_nits(c.get("body") or ""):
        kind = "nits"
    return {"kind": kind, "current": head.startswith(commit)}


def codeowner_patterns(text):
    """The path patterns of a CODEOWNERS file, in order."""
    out = []
    for line in text.splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.append(line.split()[0])
    return out


def owned(path, patterns):
    """Whether a repo path matches any CODEOWNERS pattern (the subset this
    repo uses: /file, /dir/, and *.ext anywhere)."""
    for pat in patterns:
        anchored, p = pat.startswith("/"), pat.lstrip("/")
        if p.endswith("/"):
            if path.startswith(p) if anchored else f"/{p}" in f"/{path}":
                return True
        elif anchored or "/" in p:
            if fnmatch.fnmatchcase(path, p) or path.startswith(p + "/"):
                return True
        elif fnmatch.fnmatchcase(path.rsplit("/", 1)[-1], p):
            return True
    return False


def why_not(snap, pr):
    """The reason this PR isn't merged automatically, or None."""
    v = pr.get("verdict")
    if pr.get("merged"):
        return "merged"
    if HOLD in pr.get("labels", []):
        return "held"
    if pr.get("draft"):
        return "draft"
    if not v or not v["current"]:
        return "no verdict on the head commit"
    if v["kind"] != "ready":
        return f"verdict is {v['kind']}"
    mine = ticket_numbers(pr["head"], pr.get("body"))
    twins = [p["number"] for p in snap.prs if p["number"] != pr["number"] and not p.get("merged")
             and mine & ticket_numbers(p["head"], p.get("body"))]
    if twins:
        return f"another open PR is for the same ticket (#{twins[0]})"
    if pr.get("mergeable_state") != "clean":
        return f"not clean ({pr.get('mergeable_state') or 'unknown'})"
    if snap.codeowners is None:
        return "CODEOWNERS unreadable"
    if any(owned(f, snap.codeowners) for f in pr.get("files", [])):
        return "touches a CODEOWNERS path"
    if snap.master_ci == "failure":
        return "master CI is red"
    return None


def auto_merge(snap):
    return [Action("merge_pr", f"pr#{pr['number']}", {"sha": pr["sha"]},
                   f"reviewer said Ready on {pr['sha'][:7]} and every required check passed")
            for pr in snap.prs if is_ticket_pr(snap, pr) and why_not(snap, pr) is None]
