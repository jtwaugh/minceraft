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
import re
import subprocess  # noqa: F401  (gh_cache calls it; the tests patch factory_tick.subprocess.run)
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_fix import marker  # noqa: E402


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
    not_modified: int = 0  # of those, 304s (free against the rate limit)
    reported: set = field(default_factory=set)  # (target, reason) already reported
    codeowners: list = field(default_factory=list)  # CODEOWNERS patterns; None if unreadable
    master_ci: str = ""  # conclusion of the newest finished CI run on master
    master_runs: list = field(default_factory=list)  # finished master CI runs: created_at, conclusion
    unchanged: bool = False  # every read was a 304 after a clean tick: nothing to do

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
    def from_github(cls, repo):
        """Read the repo through gh_cache: conditional requests whose ETags
        outlive the run when $FACTORY_HTTP_CACHE is set (a 304 is free), and
        one network read per path per run. Issues are read incrementally
        (`read_issues`). The board is read when PROJECT_TOKEN and
        PROJECT_NUMBER are set, unless every read came back unchanged after
        a clean tick (`unchanged`). Docs: later."""
        import gh_cache
        snap = cls()
        gh_cache.begin()
        get, paged = gh_cache.get, gh_cache.get_all

        for i in read_issues(repo, gh_cache.state, paged):
            if i.get("pull_request"):
                continue
            labels = [lb["name"] for lb in i["labels"]]
            snap.issues.append({
                "number": i["number"], "title": i["title"], "state": i["state"],
                "labels": labels,
                "kind": ("plan" if i["title"].startswith("Plan:") else
                         "report" if i["title"] == REPORT_TITLE else
                         "decision" if "spec-question" in labels else
                         "idea" if "idea" in labels else "ticket"),
                "body": i.get("body") or "",
                "body_excerpt": (i.get("body") or "")[:500],
                "parent": (int(i["parent_issue_url"].rsplit("/", 1)[1])
                           if i.get("parent_issue_url") else None)})
        snap.plans = [i for i in snap.issues if i["kind"] == "plan"]
        for plan in snap.plans:  # the planner's <!-- plan-source file=... section=... -->
            m = re.search(r"<!-- plan-source file=(\S+) section=(.*?) -->", plan["body"])
            if m:
                plan["plan_source"] = {"file": m.group(1), "section": m.group(2)}
        os.environ.setdefault("GITHUB_REPOSITORY", repo)
        import tickets  # here: tickets reads the environment on import
        for i in snap.issues:
            i["depends_on"] = sorted(tickets.deps(i["body"]) - {i["number"]})
            if i["state"].lower() == "open" and i["kind"] == "ticket" and "factory" in i["labels"]:
                i["comments"] = [{"user": c["user"]["login"], "body": c.get("body") or "",
                                  "created_at": c["created_at"]}
                                 for c in paged(f"repos/{repo}/issues/{i['number']}/comments")
                                 if c["user"]["login"] in tickets.TRUSTED]
        snap.prs = [{"number": p["number"], "head": p["head"]["ref"],
                     "sha": p["head"]["sha"], "body": p.get("body") or "",
                     "labels": [lb["name"] for lb in p.get("labels") or []],
                     "draft": bool(p.get("draft"))}
                    for p in paged(f"repos/{repo}/pulls?state=open")]
        from factory_automerge import codeowner_patterns, verdict_of
        from factory_rerun import is_ticket_pr
        for pr in snap.prs:  # for ticket PRs only: runs, verdict, files, mergeability
            if is_ticket_pr(snap, pr):
                n = pr["number"]
                runs = get(f"repos/{repo}/actions/runs?head_sha={pr['sha']}&per_page=100")
                pr["runs"] = [{"id": r["id"], "name": r["name"], "conclusion": r["conclusion"],
                               "run_attempt": r["run_attempt"], "created_at": r["created_at"]}
                              for r in runs["workflow_runs"] if r["head_sha"] == pr["sha"]]
                # Same paths the board loader reads, so these cost nothing extra.
                pr["verdict"] = verdict_of(paged(f"repos/{repo}/issues/{n}/comments"), pr["sha"])
                pr["files"] = [f["filename"] for f in paged(f"repos/{repo}/pulls/{n}/files")]
                full = get(f"repos/{repo}/pulls/{n}")
                pr["mergeable_state"] = full.get("mergeable_state") if isinstance(full, dict) else None
        try:
            import base64
            owners = get(f"repos/{repo}/contents/.github/CODEOWNERS")
            snap.codeowners = codeowner_patterns(base64.b64decode(owners["content"]).decode())
        except (RuntimeError, KeyError, TypeError, ValueError):
            snap.codeowners = None  # unknown owners: merge nothing automatically
        ci = get(f"repos/{repo}/actions/workflows/ci.yml/runs?branch=master&event=push"
                 "&status=completed&per_page=30")
        ci_runs = ci.get("workflow_runs") if isinstance(ci, dict) else None
        snap.master_ci = (ci_runs or [{}])[0].get("conclusion") or ""
        snap.master_runs = [{"created_at": r["created_at"], "conclusion": r["conclusion"]}
                            for r in ci_runs or []]
        if unchanged(gh_cache.stats, gh_cache.state):
            snap.unchanged = True
            snap.requests, snap.not_modified = gh_cache.stats["requests"], gh_cache.stats["not_modified"]
            return snap  # nothing to do: skip the board (GraphQL, never free)
        # Only docs/ branches matter (the stalls pass); reading just those is
        # one call however many claude/ branches pile up.
        refs = get(f"repos/{repo}/git/matching-refs/heads/docs")  # every match, unpaged
        snap.branches = [r["ref"][len("refs/heads/"):] for r in refs
                         if r["ref"].startswith("refs/heads/docs/")]
        number = os.environ.get("PROJECT_NUMBER", "").strip()
        if os.environ.get("PROJECT_TOKEN") and number:
            import factory_board  # here: factory_board imports this module
            factory_board.load_board(
                snap, os.environ.get("PROJECT_OWNER") or repo.split("/")[0], int(number))
        snap.mark_reported()
        snap.requests = gh_cache.stats["requests"]
        snap.not_modified = gh_cache.stats["not_modified"]
        return snap


FULL_EVENTS = ("schedule", "workflow_dispatch")  # these ticks always read everything
FULL_EVERY = 3600  # seconds between full re-reads of the issue list


def read_issues(repo, state, paged, now=None):
    """Every issue and PR in the repo, as the issues API gives them, read
    incrementally. The list is kept in `state` (gh_cache keeps it between
    ticks); a tick asks only for what was updated since the newest
    `updated_at` it has seen, which is one free 304 when nothing changed. A
    full re-read happens with no saved list, on scheduled or hand-started
    ticks, and at least hourly, so anything `since` can miss (a deleted or
    transferred issue) is corrected within the hour."""
    now = now or datetime.now(timezone.utc)
    saved = state.get("issues")
    full_at = state.get("issues_full_at", "")
    full = (not saved or os.environ.get("GITHUB_EVENT_NAME") in FULL_EVENTS
            or not full_at
            or (now - datetime.fromisoformat(full_at)).total_seconds() > FULL_EVERY)
    if full:
        issues = {}
        fetched = paged(f"repos/{repo}/issues?state=all&sort=created&direction=asc")
        state["issues_full_at"] = now.isoformat()
    else:
        issues = dict(saved)
        fetched = paged(f"repos/{repo}/issues?state=all&sort=updated&direction=asc"
                        f"&since={state['issues_since']}")
    for i in fetched:
        issues[str(i["number"])] = {k: i.get(k) for k in KEEP}
    stamps = [i["updated_at"] for i in issues.values() if i.get("updated_at")]
    state["issues"], state["issues_since"] = issues, max(stamps, default=state.get("issues_since", ""))
    return sorted(issues.values(), key=lambda i: i["number"])


# The issue fields the loader reads, kept between ticks.
KEEP = ("number", "title", "state", "labels", "body", "parent_issue_url", "pull_request", "updated_at")


def unchanged(stats, state):
    """True when this tick can stop after reading: every read came back 304,
    the last tick applied nothing and failed nothing, and this isn't a
    scheduled or hand-started tick (those always run in full, as a safety
    net). The passes are level-triggered and none reads the clock, so the
    same input gives the same (empty) result."""
    return (os.environ.get("GITHUB_EVENT_NAME") not in FULL_EVENTS
            and stats["requests"] > 0 and stats["requests"] == stats["not_modified"]
            and state.get("last_tick_clean") is True)


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
        elif action.kind == "merge_pr":
            pr = next(p for p in snapshot.prs if f"pr#{p['number']}" == action.target)
            pr["merged"] = True
        elif action.kind == "update_branch":
            pr = next(p for p in snapshot.prs if f"pr#{p['number']}" == action.target)
            # A new head is on its way: nothing about the old one counts any more.
            pr["sha"], pr["runs"], pr["verdict"] = "", [], None
        elif action.kind == "supersede_pr":
            pr = next(p for p in snapshot.prs if f"pr#{p['number']}" == action.target)
            pr["labels"] = [*pr.get("labels", []), "superseded"]
            pr["draft"] = True
        elif action.kind == "close_issue" and issue is not None:
            issue["state"] = "closed"
            if "reason" in action.payload:
                issue["state_reason"] = action.payload["reason"]
        elif action.kind == "request_phase_done":
            snapshot.reported.add((action.target, action.kind))
        elif action.kind == "rerun_jobs":
            pr = next(p for p in snapshot.prs if f"pr#{p['number']}" == action.target)
            run = next(r for r in pr["runs"] if r["id"] == action.payload["run_id"])
            run["run_attempt"], run["conclusion"] = run["run_attempt"] + 1, None
        elif action.kind == "start_fix":
            pr = next(p for p in snapshot.prs if f"pr#{p['number']}" == action.target)
            pr.setdefault("comments", []).append({
                "user": "github-actions[bot]",
                "body": marker(pr["number"], action.payload["head"], action.payload["kind"]),
                "created_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")})
        elif action.kind == "open_question":
            if not any(action.payload["key"] in i["body"] for i in snapshot.issues):
                snapshot.issues.append({
                    "number": max([i["number"] for i in snapshot.issues], default=0) + 1,
                    "title": action.payload["title"], "state": "open",
                    "labels": [action.payload["label"]], "kind": "question",
                    "body": action.payload["body"]})
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
    """The fixed order: invariants, supersede, refresh, stalls, rerun, nits, report, plans,
    ready, merge, board."""
    from factory_automerge import auto_merge
    from factory_board import board_pass as board
    from factory_plans import close_finished_plans
    from factory_nits import nits
    from factory_refresh import refresh_after_red_master
    from factory_ready import ready
    from factory_report import report
    from factory_rerun import rerun_failed
    from factory_stalls import stalls
    from factory_supersede import superseded

    def board_pass(snap):
        return board(snap) if snap.board_meta.get("loaded") else []

    return [invariants, superseded, refresh_after_red_master, stalls, rerun_failed, nits, report, close_finished_plans, ready, auto_merge, board_pass]


def main(argv):
    try:
        _main(argv)
    finally:
        import gh_cache
        gh_cache.save()  # even after a failed read: what was read still saves calls next time


def _main(argv):
    args = argv[1:]
    dry = "--dry-run" in args
    if "--fixture" in args:
        snap = Snapshot.from_fixture(args[args.index("--fixture") + 1])
    else:
        snap = Snapshot.from_github(os.environ["GITHUB_REPOSITORY"])
    if snap.unchanged:
        print(f"Nothing changed since the last clean tick: {snap.requests} reads, "
              f"all 304 (free). Skipped the board and the passes.")
        return
    print(f"{len(snap.issues)} issues, {len(snap.prs)} PRs, "
          f"{len(snap.branches)} docs branches; {snap.requests} API requests, "
          f"{snap.not_modified} of them 304 (free)")
    backend = FakeBackend()
    live = not (dry or "--fixture" in args)
    if live:
        import gh_cache
        from factory_backend import GhBackend
        backend = GhBackend(os.environ["GITHUB_REPOSITORY"])
        gh_cache.state["last_tick_clean"] = False  # until this tick finishes with nothing to do
    actions = tick(snap, passes(), backend)
    if live:
        gh_cache.state["last_tick_clean"] = not (backend.applied or backend.failed or backend.refused)
    for a in actions:
        print(f"{a.kind} {a.target}: {a.reason}")
    for a in getattr(backend, "refused", []):
        print(f"refused {a.kind} {a.target}: actor {a.payload['actor']} is not trusted")
    for a, err in getattr(backend, "failed", []):
        print(f"failed {a.kind} {a.target}: {err}")
    written = isinstance(backend, FakeBackend) and "nothing written" or "applied"
    print(f"{len(actions)} action(s), {written}; "
          f"{snap.requests + getattr(backend, 'requests', 0)} API requests")


if __name__ == "__main__":
    main(sys.argv)
