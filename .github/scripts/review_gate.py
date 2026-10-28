#!/usr/bin/env python3
"""Turn the reviewer's verdict into a commit status that can block a merge.

    review_gate.py --pr N        evaluate one pull request
    review_gate.py --sha SHA     evaluate the open pull requests whose head is SHA

Sets the `review-verdict` status on the PR's head commit:

- success  the reviewer said "Ready" or "Ready, with nits" about the head
           commit; or the PR only touches SPEC.md and docs/design/ (the
           reviewer skips those, you review them yourself); or the
           maintainer commented `/override <reason>` after the verdict.
- failure  the reviewer said "Blocked" about the head commit and nobody
           has overridden it.
- pending  no verdict yet for the head commit. Every push resets it.

A verdict counts only for the commit named in its result's `reviewed_commit`,
so a fix pushed after a block is never merged unreviewed. Only the
maintainer's /override counts; an agent can't override a block.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time

REPO = os.environ["GITHUB_REPOSITORY"]
MAINTAINER = "jtwaugh"
BOT = "claude[bot]"
# The reviewer workflow posts the structured result with its own token.
ACTIONS = "github-actions[bot]"
CONTEXT = "review-verdict"
# Same paths the reviewer workflow ignores (claude-review.yml, paths-ignore).
NO_REVIEW = re.compile(r"^(SPEC\.md|docs/design/.*)$")

# The reviewer's validated result: a marker comment holding one JSON block
# (docs/design/factory-orchestrator.md Phase C, decided in #1218).
RESULT = re.compile(r"<!-- factory-result role=reviewer\b[^>]*-->\s*```json\s*(.*?)```", re.S)
OVERRIDE = re.compile(r"^\s*/override\b[ \t]*(.*)", re.S)


def run_gh(args, tries=3):
    """`gh <args>`, retried with backoff so a brief API failure doesn't leave a
    PR without a status; the last failure exits with gh's own error."""
    for attempt in range(tries):
        p = subprocess.run(["gh", *args], capture_output=True, text=True)
        if p.returncode == 0:
            return p.stdout
        print(f"gh {' '.join(args)}: {p.stderr.strip()}", file=sys.stderr)
        if attempt + 1 < tries:
            time.sleep(5 * 2 ** attempt)
    sys.exit(f"gh {' '.join(args)} failed {tries} times")


def gh(path):
    out = run_gh(["api", "--paginate", "--slurp", path])
    pages = json.loads(out)
    if pages and isinstance(pages[0], list):
        return [item for page in pages for item in page]
    return pages[0] if pages else []


def set_status(sha, state, description, url):
    description = description if len(description) <= 140 else description[:137] + "..."
    run_gh(["api", "-X", "POST", f"repos/{REPO}/statuses/{sha}",
            "-f", f"state={state}", "-f", f"context={CONTEXT}",
            "-f", f"description={description}", "-f", f"target_url={url}"])
    print(f"{sha[:7]}: {state}: {description}")


def result_value(body):
    """The reviewer's validated result (a dict) in the comment, or None."""
    import agent_results
    for m in RESULT.finditer(body):
        if agent_results.validate("reviewer", m[1]):
            continue
        value = json.loads(m[1])
        if "verdict" in value:
            return value
    return None


def structured_verdict(body):
    """(kind, reviewed_sha) from a valid reviewer result in the comment, or None."""
    value = result_value(body)
    if value is None:
        return None
    kind = "blocked" if value["verdict"] in ("blocked", "changes needed") else "ready"
    return kind, value["reviewed_commit"].lower()


def has_nits(body):
    """True when the comment's reviewer result is "ready with nits"."""
    value = result_value(body)
    return bool(value) and value["verdict"] == "ready with nits"


def latest_verdict(comments):
    """The reviewer's newest verdict comment as (kind, reviewed_sha, comment), or None.
    Only a valid structured result from the bot or the workflow counts."""
    found = None
    for c in comments:
        if c["user"]["login"] not in (BOT, ACTIONS):
            continue
        parsed = structured_verdict(c.get("body") or "")
        if parsed is None:
            continue
        if found is None or c["updated_at"] > found[2]["updated_at"]:
            found = (*parsed, c)
    return found


def overrides(comments):
    """The maintainer's /override comments that give a reason, oldest first."""
    out = []
    for c in comments:
        if c["user"]["login"] != MAINTAINER:
            continue
        m = OVERRIDE.match(c.get("body") or "")
        if m and m.group(1).strip():
            out.append((c, " ".join(m.group(1).split())))
    return sorted(out, key=lambda x: x[0]["created_at"])


def decide(pr, files, comments, head_time):
    """Return (state, description, url) for a PR. Pure, so it can be tested."""
    head = pr["head"]["sha"]
    url = pr["html_url"]
    if not files:
        return "success", "No file changes: nothing to review", url
    if all(NO_REVIEW.match(f) for f in files):
        return "success", "Spec and design-doc changes: reviewed by the maintainer, not the agent", url

    verdict = latest_verdict(comments)
    current = verdict if verdict and verdict[1] and head.startswith(verdict[1]) else None
    ovs = overrides(comments)

    if current and current[0] == "ready":
        return "success", f"Reviewer: ready ({head[:7]})", current[2]["html_url"]

    if current:  # blocked on the head commit
        after = [o for o in ovs if o[0]["created_at"] > current[2]["updated_at"]]
        if after:
            c, reason = after[-1]
            return "success", f"Block overridden by @{MAINTAINER}: {reason}", c["html_url"]
        return ("failure", "Blocked by the reviewer. Fix it, or comment /override <reason>",
                current[2]["html_url"])

    # No verdict for this commit yet.
    after = [o for o in ovs if o[0]["created_at"] > head_time]
    if after:
        c, reason = after[-1]
        return "success", f"Review skipped by @{MAINTAINER}: {reason}", c["html_url"]
    return "pending", f"Waiting for the reviewer on {head[:7]}", url


def evaluate(n):
    pr = gh(f"repos/{REPO}/pulls/{n}")
    if pr["state"] != "open":
        print(f"#{n} is {pr['state']}; nothing to do")
        return
    files = [f["filename"] for f in gh(f"repos/{REPO}/pulls/{n}/files?per_page=100")]
    comments = gh(f"repos/{REPO}/issues/{n}/comments?per_page=100")
    head_time = gh(f"repos/{REPO}/commits/{pr['head']['sha']}")["commit"]["committer"]["date"]
    state, description, url = decide(pr, files, comments, head_time)
    set_status(pr["head"]["sha"], state, description, url)


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--pr", type=int)
    g.add_argument("--sha")
    args = ap.parse_args()
    if args.pr:
        evaluate(args.pr)
        return
    pulls = gh(f"repos/{REPO}/pulls?state=open&per_page=100")
    matched = [p["number"] for p in pulls if p["head"]["sha"] == args.sha]
    if not matched:
        print(f"No open pull request has head {args.sha[:7]}")
    for n in matched:
        evaluate(n)


if __name__ == "__main__":
    main()
