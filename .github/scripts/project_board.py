"""The factory board's rules and project API, for the tick's board pass.

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
branch) get the same status. factory_board.py decides what each card should
show and factory_backend.py writes it, with the queries here.

Reads the repo with PROJECT_TOKEN (a classic token with the `project` scope;
the project belongs to the user, so the workflow's own token can't change
it), keeping them off the workflow token's budget; GH_TOKEN is the fallback.
"""
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import review_gate  # noqa: E402  (verdict parsing and the merge-gate decision)
import tickets  # noqa: E402  (dependency and file-overlap rules)

REPO = os.environ["GITHUB_REPOSITORY"]
STATUSES = ["Inbox", "Approved", "Blocked", "Nits", "Ready for Merge", "Done"]
OPTIONAL = {"Submitted": "Approved", "Idea": "Inbox"}  # status: what to use if the board lacks it
# Check runs from agent and housekeeping jobs, not checks on the code.
AGENT_JOBS = {"implement", "start", "edit-spec", "reconcile", "plan-source",
              "phase-done", "drift", "pick", "plan", "find", "resolve"}
NITS = re.compile(r"Verdict:\s*\**\s*ready,? with nits", re.I)


READ_ENV = dict(os.environ, GH_TOKEN=os.environ.get("PROJECT_TOKEN") or os.environ.get("GH_TOKEN", ""))


def rest(path):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path],
                         check=True, capture_output=True, text=True, env=READ_ENV).stdout
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
                         check=True, capture_output=True, text=True, env=READ_ENV).stdout
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


# ------------------------------------------------------------------- project

PROJECT_QUERY = """
query($login: String!, $number: Int!, $after: String) {
  user(login: $login) {
    projectV2(number: $number) {
      id
      field(name: "Status") {
        ... on ProjectV2SingleSelectField { id options { id name } }
      }
      waitingField: field(name: "Waiting on") {
        ... on ProjectV2Field { id dataType }
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
          status: fieldValueByName(name: "Status") {
            ... on ProjectV2ItemFieldSingleSelectValue { name updatedAt }
          }
          waiting: fieldValueByName(name: "Waiting on") {
            ... on ProjectV2ItemFieldTextValue { text }
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

SET_TEXT = """
mutation($project: ID!, $item: ID!, $field: ID!, $text: String!) {
  updateProjectV2ItemFieldValue(input: {projectId: $project, itemId: $item,
      fieldId: $field, value: {text: $text}}) { projectV2Item { id } }
}"""

CLEAR = """
mutation($project: ID!, $item: ID!, $field: ID!) {
  clearProjectV2ItemFieldValue(input: {projectId: $project, itemId: $item,
      fieldId: $field}) { projectV2Item { id } }
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
    wf = project.get("waitingField") or {}
    waiting_field = wf.get("id") if wf.get("dataType") == "TEXT" else None
    if not waiting_field:
        print("No 'Waiting on' text field on the project; add one to see dependencies on cards.")
    return project["id"], field["id"], options, items, waiting_field
