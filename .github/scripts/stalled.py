#!/usr/bin/env python3
"""Find design docs the factory has stopped moving on, and say so in one issue.

    stalled.py          print what's stalled
    stalled.py --issue  also keep the "Factory: stalled work" issue in step:
                        open or update it while anything is stalled, close
                        it once nothing is

Each accepted doc in docs/design/ is checked (docs with an open PR are
skipped: the PR is the next step):

- no phases in the header: the planner skips the doc entirely;
- next `todo` phase has no plan;
- next phase's newest plan is closed with every sub-issue closed, but the
  phase is still `todo`: the docs updater found something missing (see the
  doc's Reconciliation notes) and nothing will plan it. File a ticket as a
  sub-issue of that plan and reopen the plan.

With --issue it first adopts orphaned follow-up tickets: an open `planned`
ticket with no plan whose "Depends on:" ticket belongs to a plan (the
implementer's split-off second halves) becomes a sub-issue of that plan,
and the plan is reopened, so the phase is marked done when it closes.

It also lists leftover `docs/*` branches the docs updater will want to
create again (an open plan's phase-done branch, any `-sync` branch): the
updater can't force-push, so its run would fail.
"""
import glob
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from plan_targets import header, plan_issues  # noqa: E402

REPO = os.environ["GITHUB_REPOSITORY"]
TITLE = "Factory: stalled work"


def gh(path, *args):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path, *args],
                         check=True, capture_output=True, text=True).stdout
    pages = json.loads(out)
    if pages and isinstance(pages[0], list):
        return [item for page in pages for item in page]
    return pages[0] if pages else []


def open_prs():
    """(files the open PRs change, the open PRs' head branches)."""
    touched, heads = set(), set()
    for pr in gh(f"repos/{REPO}/pulls?state=open&per_page=100"):
        heads.add(pr["head"]["ref"])
        for f in gh(f"repos/{REPO}/pulls/{pr['number']}/files?per_page=100"):
            touched.add(f["filename"])
    return touched, heads


def adopt_orphans(tickets):
    """Attach orphaned follow-ups to their predecessor's plan; one line each."""
    out = []
    for t in tickets:
        if t["state"] != "open" or t["title"].startswith("Plan:") or "pull_request" in t:
            continue
        t = gh(f"repos/{REPO}/issues/{t['number']}")
        if t.get("parent_issue_url"):
            continue
        line = next((x for x in (t.get("body") or "").splitlines() if "Depends on:" in x), "")
        for dep in re.findall(r"#(\d+)", line):
            parent = gh(f"repos/{REPO}/issues/{dep}").get("parent_issue_url")
            plan = gh("repos/" + parent.split("/repos/", 1)[1]) if parent else None
            if not plan or not plan["title"].startswith("Plan:"):
                continue
            n = plan["number"]
            subprocess.run(["gh", "api", "-X", "POST", f"repos/{REPO}/issues/{n}/sub_issues",
                            "-F", f"sub_issue_id={t['id']}"], check=True, capture_output=True)
            if plan["state"] == "closed":
                subprocess.run(["gh", "api", "-X", "PATCH", f"repos/{REPO}/issues/{n}",
                                "-f", "state=open"], check=True, capture_output=True)
            out.append(f"Adopted #{t['number']} into plan #{n} (it follows #{dep}).")
            break
    return out


def findings():
    plans = [p for p in plan_issues() if p["title"].startswith("Plan:")]
    busy, heads = open_prs()
    out, open_plans = [], []
    for path in sorted(glob.glob("docs/design/*.md")):
        if path.endswith("README.md"):
            continue
        status, phases = header(path)
        name = os.path.basename(path)[:-3]
        mine = [p for p in plans if f"plan-source file={path} " in (p.get("body") or "")]
        open_plans += [(name, p["number"]) for p in mine if p["state"] == "open"]
        if status != "accepted" or path in busy:
            continue
        if not phases:
            out.append(f"`{path}` has no phases, so the planner skips it. "
                       f"Run the reconciler on it to add them: "
                       f"`gh workflow run claude-docs.yml -f path={path}`.")
            continue
        todo = [p for p, s in phases if s == "todo"]
        if not todo:
            continue
        section = f"Phase {todo[0]}"
        these = [p for p in mine if f"section={section} -->" in (p.get("body") or "")]
        if not these:
            out.append(f"`{path}` {section} has no plan. "
                       f"`gh workflow run claude-plan.yml -f file={path}`.")
            continue
        latest = max(these, key=lambda p: p["number"])
        if latest["state"] != "closed" or latest.get("state_reason") == "not_planned":
            continue
        subs = gh(f"repos/{REPO}/issues/{latest['number']}/sub_issues?per_page=100")
        if subs and all(s["state"] == "closed" for s in subs):
            out.append(f"`{path}` {section}: plan #{latest['number']} is closed but the phase "
                       f"is still `todo`. The doc's Reconciliation notes say what's missing; "
                       f"file a ticket for it as a sub-issue of #{latest['number']} and reopen the plan.")
    refs = gh(f"repos/{REPO}/git/matching-refs/heads/docs")
    branches = {r["ref"].removeprefix("refs/heads/") for r in refs
                if r["ref"].startswith("refs/heads/docs/")} - heads
    for name, n in open_plans:
        b = f"docs/{name}-phase-done-{n}"
        if b in branches:
            out.append(f"Branch `{b}` is left over; delete it before plan #{n} closes, "
                       f"or the docs updater can't push its phase-done PR.")
    for b in sorted(x for x in branches if x.endswith("-sync")):
        out.append(f"Branch `{b}` is left over; delete it, or the next drift run can't push.")
    return out


def sync_issue(items):
    existing = [i for i in gh(f"repos/{REPO}/issues?state=open&per_page=100")
                if i["title"] == TITLE and "pull_request" not in i]
    if not items:
        for i in existing:
            subprocess.run(["gh", "issue", "close", str(i["number"]), "--repo", REPO,
                            "--comment", "Nothing is stalled now."], check=True)
        return
    body = ("Found by `.github/scripts/stalled.py` (`stalled.yml`). "
            "This issue is rewritten on every run and closed when nothing is stalled.\n\n"
            + "\n".join(f"- [ ] {s}" for s in items) + "\n")
    if existing:
        subprocess.run(["gh", "api", "-X", "PATCH", f"repos/{REPO}/issues/{existing[0]['number']}",
                        "-f", f"body={body}"], check=True, capture_output=True)
    else:
        subprocess.run(["gh", "issue", "create", "--repo", REPO, "--title", TITLE,
                        "--body", body], check=True)


def main():
    adopted = adopt_orphans(plan_issues()) if "--issue" in sys.argv else []
    print("\n".join(adopted))
    items = findings()
    print("\n".join(items) if items else "Nothing stalled.")
    if "--issue" in sys.argv:
        sync_issue(items)


if __name__ == "__main__":
    main()
