"""Live backend of the factory tick: applies each action through `gh`.

    GhBackend(repo, run=gh_run).apply(snapshot, action)

An action is skipped, with no request, when the snapshot already shows it
done, and refused when its payload names an `actor` outside `tickets.TRUSTED`
(human inputs only count from the maintainer's accounts). Whatever is applied
is mirrored into the snapshot with `FakeBackend`, so later passes see it and a
second tick over the same state writes nothing.
`run(args, project=False)` runs `gh` and returns stdout; tests inject one.
(docs/design/factory-orchestrator.md Phase A, design rules 2 and 6)
"""
import json
import os
import subprocess
import sys

os.environ.setdefault("GITHUB_REPOSITORY", "")  # project_board and tickets read it on import
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import project_board  # noqa: E402
import tickets  # noqa: E402
from factory_fix import marker, markers  # noqa: E402
from factory_tick import Backend, FakeBackend  # noqa: E402

START_WORKFLOW = "claude.yml"
START_MARKER = "<!-- factory-auto-start --> Dependencies closed; starting automatically (factory tick)."
DOCS_WORKFLOW = "claude-docs.yml"
PLAN_WORKFLOW = "claude-plan.yml"
FIX_WORKFLOW = "claude-fix.yml"


def gh_run(args, project=False):
    """`gh <args>`; project calls use PROJECT_TOKEN and repo calls WRITE_TOKEN
    (the workflow token) when set."""
    env = dict(os.environ)
    if project and env.get("PROJECT_TOKEN"):
        env["GH_TOKEN"] = env["PROJECT_TOKEN"]
    elif not project and env.get("WRITE_TOKEN"):
        env["GH_TOKEN"] = env["WRITE_TOKEN"]
    p = subprocess.run(["gh", *args], capture_output=True, text=True, env=env)
    if p.returncode:
        raise RuntimeError(f"gh {' '.join(args[:3])}: {p.stderr.strip()}")
    return p.stdout


class GhBackend(Backend):
    def __init__(self, repo, run=gh_run):
        self.repo, self.run, self.requests = repo, run, 0
        self.refused = []
        self.failed = []  # (action, error) for writes that may fail without stopping the tick
        self.applied = 0  # actions that needed a write (not already satisfied)
        self.mirror = FakeBackend()

    def _gh(self, *args, project=False):
        self.requests += 1
        return self.run(list(args), project=project)

    def apply(self, snapshot, action):
        actor = action.payload.get("actor")
        if actor is not None and actor not in tickets.TRUSTED:
            self.refused.append(action)
            return
        if self._satisfied(snapshot, action):
            return
        self.applied += 1
        if self._write(snapshot, action) is None:
            self.mirror.apply(snapshot, action)

    # -- what the snapshot already shows

    def _issue(self, snapshot, action):
        return next((i for i in snapshot.issues if f"issue#{i['number']}" == action.target), None)

    def _card(self, snapshot, action):
        kind, n = ("Issue", action.target[6:]) if action.target.startswith("issue#") \
            else ("PullRequest", action.target[3:])
        return next((c for c in snapshot.board if (c["kind"], c["number"]) == (kind, int(n))), None)

    def _satisfied(self, snapshot, action):
        k, p = action.kind, action.payload
        issue = self._issue(snapshot, action)
        if k in ("add_label", "remove_label"):
            pr = next((x for x in snapshot.prs if f"pr#{x['number']}" == action.target), None)
            labels = issue["labels"] if issue is not None else pr.get("labels", []) if pr else None
            if labels is None:
                return False
            return (p["label"] in labels) == (k == "add_label")
        if k == "set_parent":
            return issue is not None and issue.get("parent") == p["parent"]
        if k == "add_blocked_by":
            return issue is not None and p["blocker"] in issue.get("blocked_by_links", [])
        if k == "close_issue":
            return issue is not None and issue["state"] == "closed"
        if k == "update_issue":
            return issue is not None and issue.get("body") == p["body"]
        if k == "create_issue":
            return any(i["title"] == p["title"] and i["state"] == "open" for i in snapshot.issues)
        if k in ("set_status", "set_waiting", "add_card"):
            card = self._card(snapshot, action)
            if k == "add_card":
                return card is not None
            if card is None:
                return False
            return card.get("status") == p["status"] if k == "set_status" \
                else (card.get("waiting") or "") == p["text"]
        if k == "merge_pr":
            pr = next((x for x in snapshot.prs if f"pr#{x['number']}" == action.target), None)
            return pr is None or pr.get("merged", False)
        if k == "update_branch":
            pr = next((x for x in snapshot.prs if f"pr#{x['number']}" == action.target), None)
            return pr is None or pr["sha"] != p["sha"]
        if k == "supersede_pr":
            pr = next((x for x in snapshot.prs if f"pr#{x['number']}" == action.target), None)
            return pr is None or "superseded" in pr.get("labels", [])
        if k == "start_fix":
            pr = next((x for x in snapshot.prs if f"pr#{x['number']}" == action.target), None)
            lost = pr.get("fix_lost", []) if pr else []
            return pr is None or any(
                (pr["number"], p["head"], p["kind"]) in markers([c])
                for c in pr.get("comments", []) if c.get("created_at") not in lost)
        if k == "open_pr":
            return any(x["head"] == p["branch"] for x in snapshot.prs)
        if k == "open_question":
            found = self._question(snapshot, action)
            return found is not None and found.get("body") == p["body"]
        if k in ("report", "request_replan", "request_phase_done"):
            return (action.target, k if k == "request_phase_done" else action.reason) in snapshot.reported
        return False

    def _question(self, snapshot, action):
        """The open issue already carrying this question's hidden key, if any."""
        key = action.payload["key"]
        return next((i for i in snapshot.issues
                     if i["state"] == "open" and key in (i.get("body") or "")), None)

    # -- the requests

    def _id(self, n):
        return self._gh("api", f"repos/{self.repo}/issues/{n}", "--jq", ".id").strip()

    def _graphql(self, query, **variables):
        args = ["api", "graphql", "-f", f"query={query}"]
        for key, v in variables.items():
            args += ["-F" if isinstance(v, int) else "-f", f"{key}={v}"]
        return json.loads(self._gh(*args, project=True))["data"]

    def _write_board(self, snapshot, action):
        k, p, meta = action.kind, action.payload, snapshot.board_meta
        project, n = meta["project"], action.target.split("#")[-1]
        if k == "add_card":
            node = self._gh("api", f"repos/{self.repo}/issues/{n}", "--jq", ".node_id").strip()
            item = self._graphql(project_board.ADD_ITEM, project=project,
                                 content=node)["addProjectV2ItemById"]["item"]["id"]
            self._graphql(project_board.SET_STATUS, project=project, item=item,
                          field=meta["status_field"], option=meta["option_ids"][p["status"]])
            if p["waiting"] and meta.get("waiting_field"):
                self._graphql(project_board.SET_TEXT, project=project, item=item,
                              field=meta["waiting_field"], text=p["waiting"])
            self.mirror.apply(snapshot, action)
            self._card(snapshot, action)["item"] = item
            return
        card = self._card(snapshot, action)
        if k == "set_status":
            self._graphql(project_board.SET_STATUS, project=project, item=card["item"],
                          field=meta["status_field"], option=meta["option_ids"][p["status"]])
        elif p["text"]:
            self._graphql(project_board.SET_TEXT, project=project, item=card["item"],
                          field=meta["waiting_field"], text=p["text"])
        else:
            self._graphql(project_board.CLEAR, project=project, item=card["item"],
                          field=meta["waiting_field"])
        self.mirror.apply(snapshot, action)

    def _write(self, snapshot, action):
        k, p, t = action.kind, action.payload, action.target
        n = t.split("#")[-1]
        r = self.repo
        if k in ("set_status", "set_waiting", "add_card"):
            self._write_board(snapshot, action)
            return True  # already mirrored
        if k in ("add_label", "remove_label"):
            # REST: works on PRs too, and creates a missing label (gh issue edit does neither).
            if k == "add_label":
                self._gh("api", "-X", "POST", f"repos/{r}/issues/{n}/labels", "-f", f"labels[]={p['label']}")
            else:
                from urllib.parse import quote
                self._gh("api", "-X", "DELETE", f"repos/{r}/issues/{n}/labels/{quote(p['label'], safe='')}")
        elif k == "set_parent":
            # Read the parent live: attaching one doesn't bump updated_at, so
            # the snapshot's can be stale, and a second POST is a 422.
            node, live = self._gh("api", f"repos/{r}/issues/{n}", "--jq",
                                  '"\\(.id) \\(.parent_issue_url // "-")"').split()
            if live != "-":
                if live.rsplit("/", 1)[1] == str(p["parent"]):
                    return None  # already done: mirror it
                self.failed.append((action, f"already a sub-issue of #{live.rsplit('/', 1)[1]}"))
                return True
            try:
                self._gh("api", "-X", "POST", f"repos/{r}/issues/{p['parent']}/sub_issues",
                         "-F", f"sub_issue_id={node}")
            except RuntimeError as e:  # e.g. attached between the read and the POST
                self.failed.append((action, str(e)))
                return True
        elif k == "add_blocked_by":
            self._gh("api", "-X", "POST", f"repos/{r}/issues/{n}/dependencies/blocked_by",
                     "-F", f"issue_id={self._id(p['blocker'])}")
        elif k == "close_issue":
            args = ["issue", "close", n, "--repo", r,
                    "--reason", p.get("reason", "completed").replace("_", " ")]
            self._gh(*args, *(["--comment", p["comment"]] if p.get("comment") else []))
        elif k == "create_issue":
            self._gh("issue", "create", "--repo", r, "--title", p["title"], "--body", p["body"],
                     *(["--assignee", p["assignee"]] if p.get("assignee") else []))
        elif k == "comment":
            self._gh("issue", "comment", n, "--repo", r, "--body", p["body"])
        elif k == "update_issue":
            self._gh("issue", "edit", n, "--repo", r, "--body", p["body"])
        elif k == "start_ticket":
            # Dispatch first: a marker without a run would read as a start in progress.
            self._gh("workflow", "run", START_WORKFLOW, "--repo", r, "-f", f"issue={n}",
                     "-f", f"runner={snapshot.runner}")
            self._gh("issue", "comment", n, "--repo", r, "--body", START_MARKER)
        elif k == "open_pr":
            self._gh("api", "-X", "POST", f"repos/{r}/pulls", "-f", f"title={p['title']}",
                     "-f", f"head={p['branch']}", "-f", "base=master", "-f", f"body={p['body']}")
        elif k == "rerun_jobs":
            self._gh("api", "-X", "POST", f"repos/{r}/actions/runs/{p['run_id']}/rerun-failed-jobs")
        elif k == "merge_pr":
            # As the maintainer (PROJECT_TOKEN), so the merge and the ticket it
            # closes trigger the planner, the conflict resolver and the tick, which
            # events from the workflow token don't. --match-head-commit: only the
            # commit the reviewer called Ready.
            try:
                self._gh("pr", "merge", n, "--repo", r, "--merge",
                         "--match-head-commit", p["sha"], project=True)
            except RuntimeError as e:  # e.g. a push or a new check since the read
                self.failed.append((action, str(e)))
                return True  # not merged: don't mirror it
        elif k == "update_branch":
            # As the maintainer (PROJECT_TOKEN): a merge pushed with the workflow
            # token starts no workflows, so CI and the reviewer would never run.
            try:
                self._gh("api", "-X", "PUT", f"repos/{r}/pulls/{n}/update-branch",
                         "-f", f"expected_head_sha={p['sha']}", project=True)
            except RuntimeError as e:  # a conflict, or a push since the read
                self.failed.append((action, str(e)))
                return True  # not updated: don't mirror it
        elif k == "supersede_pr":
            self._gh("pr", "comment", n, "--repo", r, "--body", p["comment"])
            # REST creates the label if it doesn't exist yet; `gh pr edit` wouldn't.
            self._gh("api", "-X", "POST", f"repos/{r}/issues/{n}/labels", "-f", "labels[]=superseded")
            try:
                self._gh("pr", "ready", n, "--repo", r, "--undo")
            except RuntimeError as e:  # labelled and commented: still parked for the tick
                self.failed.append((action, str(e)))
        elif k == "request_phase_done":
            self._gh("workflow", "run", DOCS_WORKFLOW, "--repo", r, "-f", f"plan={n}")
        elif k == "request_replan":
            # No section: the planner picks the phase and skips it if a plan
            # appeared since (plan_targets.py). A section (a held plan's) is
            # planned as given; planner_apply.py still refuses a second open plan.
            args = ["-f", f"file={p['file']}"]
            args += [x for k in ("section", "notes") if p.get(k) for x in ("-f", f"{k}={p[k]}")]
            self._gh("workflow", "run", PLAN_WORKFLOW, "--repo", r, *args)
        elif k == "start_fix":
            # Dispatch first: a marker without a run would read as a fix in progress.
            self._gh("workflow", "run", FIX_WORKFLOW, "--repo", r, "-f", f"pr={n}", "-f", f"task={p['task']}",
                     "-f", f"runner={snapshot.runner}")
            self._gh("pr", "comment", n, "--repo", r, "--body", marker(int(n), p["head"], p["kind"]))
        elif k == "open_question":
            found = self._question(snapshot, action)
            if found is not None:
                self._gh("issue", "edit", str(found["number"]), "--repo", r, "--body", p["body"])
                found["body"] = p["body"]
                return True  # updated in place: nothing to mirror
            labels = p.get("labels") or [p["label"]]
            self._gh("issue", "create", "--repo", r, "--title", p["title"], "--body", p["body"],
                     *[x for lab in labels for x in ("--label", lab)])
        elif k != "report":
            # A report is only remembered; the report pass writes the issue.
            raise ValueError(f"unhandled action {k} on {t}")
