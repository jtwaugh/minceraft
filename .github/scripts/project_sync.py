#!/usr/bin/env python3
"""Set each card's Status on the factory's GitHub Project from the repo's state.

    project_sync.py            update the project (needs PROJECT_TOKEN, PROJECT_NUMBER)
    project_sync.py --dry-run  print what each open ticket and PR would get

Statuses, in the order a ticket moves through them:

  Idea             labelled `idea`: an idea, or a ticket sent back to being
                   one (falls back to Inbox if the board has no Idea column)
  Inbox            open ticket or question nobody has approved: no
                   `factory` label and no pull request
  Approved         labelled `factory`, no pull request yet: waiting on
                   dependencies or files, or being built
  Submitted        its PR is open and CI or the reviewer hasn't finished with
                   the head commit yet (falls back to Approved if the board
                   has no Submitted column)
  Blocked          its PR can't merge as is: the reviewer said Blocked (and
                   the maintainer didn't /override), a check failed, or it
                   conflicts with master
  Nits             CI passed and the reviewer said "Ready, with nits"
  Ready for Merge  CI passed, the reviewer said Ready (or the block was
                   overridden, or it's a spec/design-doc PR the reviewer
                   skips), and it merges cleanly
  Done             closed or merged

A ticket and its PR (the PR says "Closes #n" or is on a claude/issue-n-
branch) get the same status, so it doesn't matter which one is on the board.
"Plan:" issues are left alone; their sub-issue progress says more.

Labels are the source of truth, so dragging a card to another column is
undone by the next run, with one exception: a card dragged to Idea is sent
back to being an idea. The run labels the issue `idea` and removes `factory`,
so nothing builds it until it's approved again.

Every run recomputes everything, so a missed event is fixed by the next one.
Uses the REST API (GH_TOKEN) to read the repo and GraphQL (PROJECT_TOKEN, a
classic token with the `project` scope) for the project, which belongs to the
user, not the repo, so the workflow's own token can't change it.
"""
import argparse
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import review_gate  # noqa: E402  (verdict parsing and the merge-gate decision)

REPO = os.environ["GITHUB_REPOSITORY"]
STATUSES = ["Inbox", "Approved", "Blocked", "Nits", "Ready for Merge", "Done"]
OPTIONAL = {"Submitted": "Approved", "Idea": "Inbox"}  # status: what to use if the board lacks it
# Check runs from agent and housekeeping jobs (including this sync, which is
# cancelled whenever a newer run starts), not checks on the code.
AGENT_JOBS = {"implement", "find-ready", "start-ready", "edit-spec", "reconcile",
              "finish-plan", "phase-done", "drift", "pick", "plan", "sync",
              "find", "resolve"}
NITS = re.compile(r"Verdict:\s*\**\s*ready,? with nits", re.I)


def rest(path):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path],
                         check=True, capture_output=True, text=True).stdout
    pages = json.loads(out)
    if pages and isinstance(pages[0], list):
        return [item for page in pages for item in page]
    return pages[0] if pages else []


def graphql(query, **variables):
    args = ["gh", "api", "graphql", "-f", f"query={query}"]
    for k, v in variables.items():
        if v is None:
            continue
        args += ["-F" if isinstance(v, int) else "-f", f"{k}={v}"]
    env = dict(os.environ, GH_TOKEN=os.environ["PROJECT_TOKEN"])
    out = subprocess.run(args, check=True, capture_output=True, text=True, env=env).stdout
    data = json.loads(out)
    if data.get("errors"):
        raise SystemExit(f"GraphQL error: {data['errors']}")
    return data["data"]


# ---------------------------------------------------------------- repo state

def demote_to_idea(n):
    """Label issue #n `idea` and take `factory` off (it was dragged to Idea)."""
    subprocess.run(["gh", "api", "-X", "POST", f"repos/{REPO}/issues/{n}/labels",
                    "-f", "labels[]=idea"], check=True, capture_output=True)
    subprocess.run(["gh", "api", "-X", "DELETE", f"repos/{REPO}/issues/{n}/labels/factory"],
                   capture_output=True)  # fine if it wasn't there


def unlabelled_idea_at(n):
    """When the `idea` label was last removed from #n, or "" if never."""
    events = rest(f"repos/{REPO}/issues/{n}/events?per_page=100")
    times = [e["created_at"] for e in events
             if e.get("event") == "unlabeled" and (e.get("label") or {}).get("name") == "idea"]
    return max(times, default="")


def ticket_of(pr):
    """The issue number a PR closes, from its branch or body, or None."""
    m = re.match(r"claude/issue-(\d+)-", pr["head"]["ref"])
    if m:
        return int(m.group(1))
    m = re.search(r"(?i)\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?) #(\d+)\b", pr.get("body") or "")
    return int(m.group(1)) if m else None


def checks_state(sha):
    """'failed', 'running' or 'passed' over the code checks on a commit."""
    out = subprocess.run(["gh", "api", "--paginate", "--slurp",
                          f"repos/{REPO}/commits/{sha}/check-runs?per_page=100"],
                         check=True, capture_output=True, text=True).stdout
    runs = [r for page in json.loads(out) for r in page.get("check_runs", [])]
    state = "passed"
    for r in runs:
        if r["name"] in AGENT_JOBS:
            continue
        if r["status"] != "completed":
            state = "running"
        # "cancelled" is left out: a run is cancelled when a newer push
        # supersedes it, and the newer run's checks are the ones that count.
        elif r["conclusion"] in ("failure", "timed_out", "action_required", "startup_failure"):
            return "failed"
    return state


def pr_status(pr):
    """Status for an open PR."""
    n = pr["number"]
    full = rest(f"repos/{REPO}/pulls/{n}")
    if full.get("mergeable_state") == "dirty":
        return "Blocked", "conflicts with master"
    checks = checks_state(pr["head"]["sha"])
    if checks == "failed":
        return "Blocked", "a check failed"
    files = [f["filename"] for f in rest(f"repos/{REPO}/pulls/{n}/files?per_page=100")]
    comments = rest(f"repos/{REPO}/issues/{n}/comments?per_page=100")
    head_time = rest(f"repos/{REPO}/commits/{pr['head']['sha']}")["commit"]["committer"]["date"]
    gate, why, _ = review_gate.decide(full, files, comments, head_time)
    if gate == "failure":
        return "Blocked", "reviewer: blocked"
    if gate == "pending" or checks == "running":
        return "Submitted", "checks or review still running"
    verdict = review_gate.latest_verdict(comments)
    if verdict and verdict[1] and pr["head"]["sha"].startswith(verdict[1]) \
            and NITS.search(verdict[2].get("body") or ""):
        return "Nits", "reviewer: ready, with nits"
    return "Ready for Merge", why


def issue_status(issue, pr_by_ticket):
    """Status for an open issue, or None to leave it alone."""
    if issue["title"].startswith("Plan:"):
        return None, "plan"
    pr = pr_by_ticket.get(issue["number"])
    if pr:
        return pr_status(pr)
    labels = {l["name"] for l in issue.get("labels", [])}
    if "factory" in labels:
        return "Approved", "approved, no PR yet"
    return "Inbox", "not approved yet"


def desired():
    """{(kind, number): (status, reason, node_id)} for every open ticket and PR."""
    pulls = rest(f"repos/{REPO}/pulls?state=open&per_page=100")
    pr_by_ticket, want = {}, {}
    for pr in pulls:
        t = ticket_of(pr)
        if t and t not in pr_by_ticket:
            pr_by_ticket[t] = pr
    pr_cache = {}
    for pr in pulls:
        s, why = pr_status(pr)
        pr_cache[pr["number"]] = (s, why)
        want[("PullRequest", pr["number"])] = (s, why, pr["node_id"])
    issues = [i for i in rest(f"repos/{REPO}/issues?state=open&per_page=100") if "pull_request" not in i]
    for issue in issues:
        labels = {l["name"] for l in issue.get("labels", [])}
        if not labels & {"planned", "spec-question", "idea", "factory"}:
            continue
        if "idea" in labels and not issue["title"].startswith("Plan:"):
            want[("Issue", issue["number"])] = ("Idea", "labelled idea", issue["node_id"])
            continue
        pr = pr_by_ticket.get(issue["number"])
        if pr:
            s, why = pr_cache[pr["number"]]
        else:
            s, why = issue_status(issue, {})
        if s:
            want[("Issue", issue["number"])] = (s, why, issue["node_id"])
    return want


# ------------------------------------------------------------------- project

PROJECT_QUERY = """
query($login: String!, $number: Int!, $after: String) {
  user(login: $login) {
    projectV2(number: $number) {
      id
      field(name: "Status") {
        ... on ProjectV2SingleSelectField { id options { id name } }
      }
      items(first: 100, after: $after) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          content {
            __typename
            ... on Issue { number state repository { nameWithOwner } }
            ... on PullRequest { number state repository { nameWithOwner } }
          }
          fieldValueByName(name: "Status") {
            ... on ProjectV2ItemFieldSingleSelectValue { name updatedAt }
          }
        }
      }
    }
  }
}"""

SET_STATUS = """
mutation($project: ID!, $item: ID!, $field: ID!, $option: String!) {
  updateProjectV2ItemFieldValue(input: {projectId: $project, itemId: $item,
      fieldId: $field, value: {singleSelectOptionId: $option}}) { projectV2Item { id } }
}"""

ADD_ITEM = """
mutation($project: ID!, $content: ID!) {
  addProjectV2ItemById(input: {projectId: $project, contentId: $content}) { item { id } }
}"""


def load_project(login, number):
    items, after, project = [], None, None
    while True:
        p = graphql(PROJECT_QUERY, login=login, number=number, after=after)["user"]["projectV2"]
        if p is None:
            raise SystemExit(f"No project {number} for user {login}, or the token can't see it.")
        project = project or p
        items += p["items"]["nodes"]
        if not p["items"]["pageInfo"]["hasNextPage"]:
            break
        after = p["items"]["pageInfo"]["endCursor"]
    field = project["field"]
    if not field or "options" not in field:
        raise SystemExit("The project has no single-select Status field.")
    options = {o["name"]: o["id"] for o in field["options"]}
    for name, fallback in OPTIONAL.items():
        if name not in options:
            print(f"No '{name}' column on the board; using '{fallback}' instead.")
            options[name] = options.get(fallback)
    missing = [s for s in STATUSES if s not in options]
    if missing:
        raise SystemExit("Add these options to the project's Status field (exact names): "
                         + ", ".join(missing))
    return project["id"], field["id"], options, items


def sync(login, number, want):
    project, field, options, items = load_project(login, number)
    seen = set()
    changed = 0
    for it in items:
        c = it["content"]
        if not c or c.get("__typename") not in ("Issue", "PullRequest") \
                or c["repository"]["nameWithOwner"] != REPO:
            continue
        key = (c["__typename"], c["number"])
        seen.add(key)
        if c["state"] in ("CLOSED", "MERGED"):
            status, why = "Done", "closed"
        elif key in want:
            status, why = want[key][:2]
        else:
            continue  # a plan, or something this script doesn't manage
        value = it.get("fieldValueByName") or {}
        current = value.get("name")
        # In the Idea column without the label: either dragged there (make it
        # an idea) or the label was just removed to approve it again (move it
        # out). Whichever happened last wins.
        if current == "Idea" and status != "Idea" and key[0] == "Issue" and c["state"] == "OPEN" \
                and (value.get("updatedAt") or "") > unlabelled_idea_at(key[1]):
            # Dragged to Idea on the board: make it an idea for real.
            demote_to_idea(key[1])
            print(f"Issue #{key[1]}: moved to Idea on the board; labelled idea, factory removed")
            changed += 1
            continue
        if current != status:
            graphql(SET_STATUS, project=project, item=it["id"], field=field, option=options[status])
            print(f"{key[0]} #{key[1]}: {current} -> {status} ({why})")
            changed += 1
    # Open tickets and PRs not on the board yet (covers a missed auto-add).
    for key, (status, why, node) in sorted(want.items(), key=lambda kv: kv[0][1]):
        if key in seen:
            continue
        item = graphql(ADD_ITEM, project=project, content=node)["addProjectV2ItemById"]["item"]["id"]
        graphql(SET_STATUS, project=project, item=item, field=field, option=options[status])
        print(f"{key[0]} #{key[1]}: added as {status} ({why})")
        changed += 1
    print(f"{changed} card(s) updated, {len(items)} on the board.")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    want = desired()
    if args.dry_run or not os.environ.get("PROJECT_TOKEN"):
        for (kind, n), (status, why, _) in sorted(want.items(), key=lambda kv: kv[0][1]):
            print(f"{kind:12} #{n:<5} {status:16} {why}")
        if not args.dry_run:
            print("No PROJECT_TOKEN; printed what would be set.")
        return
    number = os.environ.get("PROJECT_NUMBER", "").strip()
    if not number.isdigit():
        raise SystemExit("Set the PROJECT_NUMBER repository variable to the project's number.")
    login = os.environ.get("PROJECT_OWNER") or REPO.split("/")[0]
    sync(login, int(number), want)


if __name__ == "__main__":
    main()
