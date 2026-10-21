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
import os
import subprocess
import sys

os.environ.setdefault("GITHUB_REPOSITORY", "")  # project_sync and tickets read it on import
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tickets  # noqa: E402
from factory_tick import Backend, FakeBackend  # noqa: E402

START_WORKFLOW = "claude.yml"
START_MARKER = "<!-- factory-auto-start --> Dependencies closed; starting automatically (factory tick)."
DOCS_WORKFLOW = "claude-docs.yml"


def gh_run(args, project=False):
    """`gh <args>`; project calls use PROJECT_TOKEN when it is set."""
    env = dict(os.environ)
    if project and env.get("PROJECT_TOKEN"):
        env["GH_TOKEN"] = env["PROJECT_TOKEN"]
    p = subprocess.run(["gh", *args], capture_output=True, text=True, env=env)
    if p.returncode:
        raise RuntimeError(f"gh {' '.join(args[:3])}: {p.stderr.strip()}")
    return p.stdout


class GhBackend(Backend):
    def __init__(self, repo, run=gh_run):
        self.repo, self.run, self.requests = repo, run, 0
        self.refused = []
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
        self._write(snapshot, action)
        self.mirror.apply(snapshot, action)

    # -- what the snapshot already shows

    def _issue(self, snapshot, action):
        return next((i for i in snapshot.issues if f"issue#{i['number']}" == action.target), None)

    def _satisfied(self, snapshot, action):
        k, p = action.kind, action.payload
        issue = self._issue(snapshot, action)
        if k == "add_label":
            return issue is not None and p["label"] in issue["labels"]
        if k == "remove_label":
            return issue is not None and p["label"] not in issue["labels"]
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
        if k in ("report", "request_phase_done"):
            return (action.target, action.reason if k == "report" else k) in snapshot.reported
        return False

    # -- the requests

    def _id(self, n):
        return self._gh("api", f"repos/{self.repo}/issues/{n}", "--jq", ".id").strip()

    def _write(self, snapshot, action):
        k, p, t = action.kind, action.payload, action.target
        n = t.split("#")[-1]
        r = self.repo
        if k in ("add_label", "remove_label"):
            self._gh("issue", "edit", n, "--repo", r,
                     "--add-label" if k == "add_label" else "--remove-label", p["label"])
        elif k == "set_parent":
            self._gh("api", "-X", "POST", f"repos/{r}/issues/{p['parent']}/sub_issues",
                     "-F", f"sub_issue_id={self._id(n)}")
        elif k == "add_blocked_by":
            self._gh("api", "-X", "POST", f"repos/{r}/issues/{n}/dependencies/blocked_by",
                     "-F", f"issue_id={self._id(p['blocker'])}")
        elif k == "close_issue":
            args = ["issue", "close", n, "--repo", r,
                    "--reason", p.get("reason", "completed").replace("_", " ")]
            self._gh(*args, *(["--comment", p["comment"]] if p.get("comment") else []))
        elif k == "create_issue":
            self._gh("issue", "create", "--repo", r, "--title", p["title"], "--body", p["body"])
        elif k == "update_issue":
            self._gh("issue", "edit", n, "--repo", r, "--body", p["body"])
        elif k == "start_ticket":
            self._gh("issue", "comment", n, "--repo", r, "--body", START_MARKER)
            self._gh("workflow", "run", START_WORKFLOW, "--repo", r, "-f", f"issue={n}")
        elif k == "request_phase_done":
            self._gh("workflow", "run", DOCS_WORKFLOW, "--repo", r, "-f", f"path={p['file']}")
        elif k != "report":  # a report is only remembered; the report pass writes the issue
            raise ValueError(f"unhandled action {k} on {t}")
