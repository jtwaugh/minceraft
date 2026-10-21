#!/usr/bin/env python3
"""Skeleton of the factory tick: a snapshot, passes over it, and a backend.

    factory_tick.py [--fixture FILE]    load the repo (or a fixture), print the counts

A tick loads a `Snapshot`, runs each pass (`pass(snapshot) -> [Action]`) and
applies the actions through a `Backend`. Passes are level-triggered: they
describe what should change given the snapshot, so once the backend has
applied their actions they return nothing and a second tick is a no-op.
The passes themselves live in later tickets.
(docs/design/factory-orchestrator.md Phase A, design rule 2)
"""
import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone


@dataclass(frozen=True)
class Action:
    kind: str  # "add_label", "set_parent", "start_ticket", "close_issue", "request_phase_done", "report"
    target: str  # "issue#12"
    payload: dict = field(default_factory=dict, hash=False, compare=False)
    reason: str = ""


@dataclass
class Snapshot:
    issues: list = field(default_factory=list)
    prs: list = field(default_factory=list)
    plans: list = field(default_factory=list)
    docs: list = field(default_factory=list)
    branches: list = field(default_factory=list)
    board: list = field(default_factory=list)  # project board items
    board_meta: dict = field(default_factory=dict)  # Status options, "Waiting on" field
    requests: int = 0  # API requests the loader made
    reported: set = field(default_factory=set)  # (target, reason) already reported

    @classmethod
    def from_dict(cls, d):
        issues = d.get("issues", [])
        plans = d.get("plans") or [i for i in issues if i.get("kind") == "plan"]
        return cls(issues, d.get("prs", []), plans, d.get("docs", []),
                   d.get("branches", []), d.get("board", []))

    @classmethod
    def from_fixture(cls, path):
        return cls.from_dict(json.load(open(path)))

    @classmethod
    def from_github(cls, repo, etags=None):
        """Paginated reads; `etags` (path -> (etag, body)) is sent as
        If-None-Match and a 304 reuses the body. Docs and board: later."""
        snap = cls()
        etags = {} if etags is None else etags

        def get(path):
            snap.requests += 1
            cached = etags.get(path)
            cmd = ["gh", "api", "-i", "--method", "GET", path]
            if cached:
                cmd += ["-H", f"If-None-Match: {cached[0]}"]
            p = subprocess.run(cmd, capture_output=True, text=True)
            head, _, body = p.stdout.partition("\r\n\r\n")
            if "304" in head.split("\n", 1)[0] and cached:
                return cached[1]
            if p.returncode:
                raise RuntimeError(f"gh api {path}: {p.stderr.strip()}")
            data = json.loads(body)
            for h in head.split("\n"):
                if h.lower().startswith("etag:"):
                    etags[path] = (h.split(":", 1)[1].strip(), data)
            return data

        def paged(path):
            out, page = [], 1
            while len(out) == 100 * (page - 1):
                out += get(f"{path}{'&' if '?' in path else '?'}per_page=100&page={page}")
                page += 1
            return out

        for i in paged(f"repos/{repo}/issues?state=all"):
            if "pull_request" in i:
                continue
            snap.issues.append({
                "number": i["number"], "title": i["title"], "state": i["state"],
                "labels": [lb["name"] for lb in i["labels"]],
                "kind": "plan" if i["title"].startswith("Plan:") else "ticket",
                "body_excerpt": (i.get("body") or "")[:500]})
        snap.plans = [i for i in snap.issues if i["kind"] == "plan"]
        snap.prs = [{"number": p["number"], "head": p["head"]["ref"]}
                    for p in paged(f"repos/{repo}/pulls?state=open")]
        snap.branches = [b["name"] for b in paged(f"repos/{repo}/branches")]
        return snap


class Backend:
    """Applies actions. Live implementations come with the wiring ticket."""

    def apply(self, snapshot, action):
        raise NotImplementedError


class FakeBackend(Backend):
    """Applies actions to the in-memory snapshot, so a second tick sees them."""

    def apply(self, snapshot, action):
        issue = next((i for i in snapshot.issues
                      if f"issue#{i['number']}" == action.target), None)
        if action.kind == "add_label" and issue is not None:
            if action.payload["label"] not in issue["labels"]:
                issue["labels"].append(action.payload["label"])
        elif action.kind == "set_parent" and issue is not None:
            issue["parent"] = action.payload["parent"]
        elif action.kind == "remove_label" and issue is not None:
            if action.payload["label"] in issue["labels"]:
                issue["labels"].remove(action.payload["label"])
        elif action.kind in ("set_status", "set_waiting", "add_card"):
            kind, n = ("Issue", action.target[6:]) if action.target.startswith("issue#") \
                else ("PullRequest", action.target[3:])
            card = next((c for c in snapshot.board if (c["kind"], c["number"]) == (kind, int(n))), None)
            if card is None:
                snapshot.board.append(card := {"kind": kind, "number": int(n), "state": "OPEN"})
            if "status" in action.payload:
                card["status"], card["status_updated"] = action.payload["status"], "9999"
            card["waiting"] = action.payload.get("text", action.payload.get("waiting", card.get("waiting")))
        elif action.kind == "start_ticket" and issue is not None:
            issue.setdefault("comments", []).append({
                "user": "github-actions[bot]", "body": "<!-- factory-auto-start -->",
                "created_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")})
        elif action.kind == "close_issue" and issue is not None:
            issue["state"] = "closed"
            issue["state_reason"] = action.payload["reason"]
        elif action.kind == "request_phase_done":
            snapshot.reported.add((action.target, action.kind))
        elif action.kind == "report":
            snapshot.reported.add((action.target, action.reason))
        else:
            raise ValueError(f"unhandled action {action.kind} on {action.target}")


def tick(snapshot, passes, backend):
    """Run every pass on the snapshot, apply the actions, return them."""
    applied = list(dict.fromkeys(a for p in passes for a in p(snapshot)))
    for a in applied:
        backend.apply(snapshot, a)
    return applied


PASSES = []  # filled in by later tickets

def main(argv):
    if len(argv) == 3 and argv[1] == "--fixture":
        snap = Snapshot.from_fixture(argv[2])
    else:
        snap = Snapshot.from_github(os.environ["GITHUB_REPOSITORY"])
    print(f"{len(snap.issues)} issues, {len(snap.prs)} PRs, "
          f"{len(snap.branches)} branches; {snap.requests} API requests")
    for a in (a for p in PASSES for a in p(snap)):
        print(f"{a.kind} {a.target}: {a.reason}")


if __name__ == "__main__":
    main(sys.argv)
