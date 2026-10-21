"""The live backend of the factory tick: applies actions through `gh`.

Every write first checks the world and skips what is already true, so a
second tick does nothing. Actions that carry a human's input (`actor` in the
payload: a label, `/decide`, `/override`) are refused unless the actor is in
the trusted set shared with `tickets.py`.
(docs/design/factory-orchestrator.md Phase A, "One tick")
"""
import json
import subprocess

from factory_tick import TRUSTED, Backend, FakeBackend

MARKER = "<!-- factory-auto-start -->"
START_WORKFLOW = "claude.yml"  # dispatched with -f issue=N (the workflow ticket adds the input)
PHASE_DONE_WORKFLOW = "claude-docs.yml"  # -f plan=N -f file=... -f section=...


class GhBackend(Backend):
    def __init__(self, repo, snapshot, run=subprocess.run):
        self.repo, self.requests, self.run, self.snap = repo, 0, run, snapshot
        self.mirror = FakeBackend()  # keeps the snapshot current for the passes that follow

    def gh(self, *args, stdin=None):
        self.requests += 1
        p = self.run(["gh", *args], input=stdin, capture_output=True, text=True)
        if p.returncode:
            raise RuntimeError(f"gh {' '.join(args[:3])}: {p.stderr.strip()}")
        return p.stdout

    def api(self, path, *fields, method="POST"):
        return self.gh("api", "-X", method, f"repos/{self.repo}/{path}", *fields)

    def apply(self, snapshot, action):
        self.write(snapshot, action)
        self.mirror.apply(snapshot, action)

    def write(self, snapshot, action):
        actor = action.payload.get("actor")
        if actor is not None and actor not in TRUSTED:
            raise PermissionError(f"{action.kind} {action.target}: {actor} is not a trusted account")
        kind, target, p = action.kind, action.target, action.payload
        n = int(target.split("#")[1]) if "#" in target else None
        issue = next((i for i in snapshot.issues if i["number"] == n), None) if target.startswith("issue#") else None
        labels = issue["labels"] if issue else []
        if kind == "add_label":
            if p["label"] in labels:
                return
            self.api(f"issues/{n}/labels", "-f", f"labels[]={p['label']}")
        elif kind == "remove_label":
            if p["label"] not in labels:
                return
            self.api(f"issues/{n}/labels/{p['label']}", method="DELETE")
        elif kind == "set_parent":
            node = json.loads(self.api(f"issues/{n}", method="GET"))["id"]
            self.api(f"issues/{p['parent']}/sub_issues", "-F", f"sub_issue_id={node}")
        elif kind == "start_ticket":
            self.api(f"issues/{n}/comments", "-f", f"body={MARKER} Dependencies closed; started by the factory tick.")
            self.gh("workflow", "run", START_WORKFLOW, "-R", self.repo, "-f", f"issue={n}")
        elif kind == "close_issue":
            if issue and issue["state"] == "closed":
                return
            if p.get("comment"):
                self.api(f"issues/{n}/comments", "-f", f"body={p['comment']}")
            self.api(f"issues/{n}", "-f", "state=closed", "-f", f"state_reason={p.get('reason', 'completed')}",
                     method="PATCH")
        elif kind == "request_phase_done":
            self.gh("workflow", "run", PHASE_DONE_WORKFLOW, "-R", self.repo, "-f", f"plan={n}",
                    "-f", f"file={p['file']}", "-f", f"section={p['section']}")
        elif kind == "create_issue":
            self.api("issues", "-f", f"title={p['title']}", "-f", f"body={p['body']}")
        elif kind == "update_issue":
            self.api(f"issues/{n}", "-f", f"body={p['body']}", method="PATCH")
        elif kind in ("set_status", "set_waiting", "add_card"):
            self.board(action, issue, n)
        elif kind != "report":  # reports live in the stalled-work issue
            raise ValueError(f"unhandled action {kind} on {target}")

    def board(self, action, issue, n):
        import project_sync
        project, field, options, waiting = self.snap.board_meta["ids"]
        kind = "Issue" if action.target.startswith("issue#") else "PullRequest"
        card = next((c for c in self.snap.board if (c["kind"], c["number"]) == (kind, n)), None)
        if card is None:  # add_card: needs the content's node id
            path = f"{'issues' if kind == 'Issue' else 'pulls'}/{n}"
            node = json.loads(self.api(path, method="GET"))["node_id"]
            item = project_sync.graphql(project_sync.ADD_ITEM, project=project, content=node)[
                "addProjectV2ItemById"]["item"]["id"]
            card = {"kind": kind, "number": n, "item": item, "state": "OPEN"}
            self.snap.board.append(card)
        p = action.payload
        if "status" in p:
            project_sync.graphql(project_sync.SET_STATUS, project=project, item=card["item"],
                                 field=field, option=options[p["status"]])
        text = p.get("text", p.get("waiting"))
        if waiting and text is not None:
            if text:
                project_sync.graphql(project_sync.SET_TEXT, project=project, item=card["item"],
                                     field=waiting, text=text)
            else:
                project_sync.graphql(project_sync.CLEAR, project=project, item=card["item"], field=waiting)
