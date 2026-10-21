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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


REPORT_TITLE = "Factory: stalled work"  # the one issue the report pass keeps


@dataclass(frozen=True)
class Action:
    kind: str  # "add_label", "set_parent", "start_ticket", "close_issue", "request_phase_done", "report", "create_issue", ...
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
                   d.get("branches", []), d.get("board", []), {"loaded": True})

    def mark_reported(self):
        """Findings already in the open report issue count as reported."""
        from factory_invariants import check
        from factory_stalls import messages
        body = next((i["body"] for i in self.issues
                     if i["title"] == REPORT_TITLE and i["state"] == "open"), "")
        self.reported |= {(v.subject, f"{v.key}: {v.detail}".rstrip(": ")) for v in check(self)
                          if f"`{v.key}`" in body}
        self.reported |= {(t, m) for t, m in messages(self) if f"- {m}" in body}

    @classmethod
    def from_fixture(cls, path):
        return cls.from_dict(json.load(open(path)))

    @classmethod
    def from_github(cls, repo, etags=None):
        """Paginated reads; `etags` (path -> (etag, body)) is sent as
        If-None-Match and a 304 reuses the body. The board is read when
        PROJECT_TOKEN and PROJECT_NUMBER are set. Docs: later."""
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
                "kind": ("plan" if i["title"].startswith("Plan:") else
                         "report" if i["title"] == REPORT_TITLE else "ticket"),
                "body": i.get("body") or "",
                "body_excerpt": (i.get("body") or "")[:500]})
        snap.plans = [i for i in snap.issues if i["kind"] == "plan"]
        snap.prs = [{"number": p["number"], "head": p["head"]["ref"],
                     "sha": p["head"]["sha"], "body": p.get("body") or ""}
                    for p in paged(f"repos/{repo}/pulls?state=open")]
        snap.branches = [b["name"] for b in paged(f"repos/{repo}/branches")]
        number = os.environ.get("PROJECT_NUMBER", "").strip()
        if os.environ.get("PROJECT_TOKEN") and number:
            import factory_board  # here: factory_board imports this module
            factory_board.load_board(
                snap, os.environ.get("PROJECT_OWNER") or repo.split("/")[0], int(number))
        snap.mark_reported()
        return snap


class Backend:
    """Applies actions."""

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
        elif action.kind == "add_blocked_by" and issue is not None:
            if action.payload["blocker"] not in issue.setdefault("blocked_by_links", []):
                issue["blocked_by_links"].append(action.payload["blocker"])
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
            if "reason" in action.payload:
                issue["state_reason"] = action.payload["reason"]
        elif action.kind == "request_phase_done":
            snapshot.reported.add((action.target, action.kind))
        elif action.kind == "report":
            snapshot.reported.add((action.target, action.reason))
        elif action.kind == "create_issue":
            snapshot.issues.append({
                "number": max([i["number"] for i in snapshot.issues], default=0) + 1,
                "title": action.payload["title"], "state": "open", "labels": [],
                "kind": "report", "body": action.payload["body"]})
        elif action.kind == "update_issue" and issue is not None:
            issue["body"] = action.payload["body"]
        else:
            raise ValueError(f"unhandled action {action.kind} on {action.target}")


def tick(snapshot, passes, backend):
    """Run the passes in order, applying each one's actions before the next
    pass reads the snapshot; return every action applied."""
    applied = []
    for p in passes:
        for a in dict.fromkeys(p(snapshot)):
            backend.apply(snapshot, a)
            applied.append(a)
    return applied


def invariants(snap):
    """Pass: report each violated invariant not yet reported, and adopt an
    orphan ticket into the plan its first dependency belongs to."""
    from factory_invariants import check
    by = {i["number"]: i for i in snap.issues}
    out = []
    for v in check(snap):
        reason = f"{v.key}: {v.detail}".rstrip(": ")
        if (v.subject, reason) not in snap.reported:
            out.append(Action("report", v.subject, {"kind": v.kind}, reason))
        if v.kind == "orphan-ticket":
            parents = [by[d]["parent"] for d in by[int(v.subject[6:])].get("depends_on", [])
                       if d in by and by[d].get("parent") is not None]
            if parents:
                out.append(Action("set_parent", v.subject, {"parent": parents[0]},
                                  f"orphan ticket follows #{parents[0]}'s plan"))
    return out


def passes():
    """The fixed order: invariants, stalls, report, plans, ready, board."""
    from factory_board import board_pass as board
    from factory_plans import close_finished_plans
    from factory_ready import ready
    from factory_report import report
    from factory_stalls import stalls
    def board_pass(snap):
        return board(snap) if snap.board_meta.get("loaded") else []

    return [invariants, stalls, report, close_finished_plans, ready, board_pass]


def main(argv):
    args = argv[1:]
    dry = "--dry-run" in args
    if "--fixture" in args:
        snap = Snapshot.from_fixture(args[args.index("--fixture") + 1])
    else:
        snap = Snapshot.from_github(os.environ["GITHUB_REPOSITORY"])
    print(f"{len(snap.issues)} issues, {len(snap.prs)} PRs, "
          f"{len(snap.branches)} branches; {snap.requests} API requests")
    if not (dry or "--fixture" in args):
        sys.exit("writing to GitHub needs the live backend (follow-up to #579); use --dry-run")
    actions = tick(snap, passes(), FakeBackend())
    for a in actions:
        print(f"{a.kind} {a.target}: {a.reason}")
    print(f"{len(actions)} action(s), nothing written; {snap.requests} API requests")


if __name__ == "__main__":
    main(sys.argv)
