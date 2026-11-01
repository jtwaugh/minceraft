#!/usr/bin/env python3
"""Read the factory's state once and print it as JSON.

    python3 collect.py /path/to/minceraft

Run by serve.py in a fresh process for every refresh, so nothing the
factory scripts cache in memory goes stale and the server stays small.
Reuses the repo's own .github/scripts (the tick's loader and rules), and
writes nothing to GitHub: the "next tick" list is a dry tick on a fake
backend. Reads go through gh_cache with ETags kept in $FACTORY_HTTP_CACHE,
so an unchanged repo costs mostly free 304s.
"""
import json
import os
import re
import sys
import time
import traceback
from datetime import datetime, timezone


def main():
    repo_dir = os.path.abspath(sys.argv[1])
    scripts = os.path.join(repo_dir, ".github", "scripts")
    sys.path.insert(0, scripts)
    name = os.environ.setdefault("GITHUB_REPOSITORY", "jtwaugh/minceraft")
    os.environ.pop("GITHUB_EVENT_NAME", None)  # incremental issue reads, like an event tick
    t0 = time.time()

    import gh_cache
    from factory_tick import Snapshot, FakeBackend, passes, tick
    import factory_health as health

    now = datetime.now(timezone.utc)
    snap = Snapshot.from_github(name)  # calls gh_cache.begin()
    get = gh_cache.get

    def r(path):
        return get(f"repos/{name}/{path}")

    titles = {int(k): v.get("title", "") for k, v in (gh_cache.state.get("issues") or {}).items()}
    by = {i["number"]: i for i in snap.issues}
    out = {"repo": name, "generated_at": now.isoformat(), "errors": []}

    def section(key, fn):
        try:
            out[key] = fn()
        except Exception:
            out[key] = None
            out["errors"].append(f"{key}: {traceback.format_exc().strip().splitlines()[-1]}")

    # -- tick health (same reading as factory_health.py)
    def tick_health():
        wf = r(f"actions/workflows/{health.WORKFLOW}")
        runs = []
        for page in (1, 2, 3):
            got = r(f"actions/workflows/{health.WORKFLOW}/runs?per_page=100"
                    + (f"&page={page}" if page > 1 else ""))["workflow_runs"]
            runs += got
            if len(got) < 100:
                break
        ticks = health.finished(runs)

        def brief(t):
            return t and {"conclusion": t["conclusion"], "at": t["created_at"],
                          "url": t["html_url"], "event": t["event"], "id": t["id"]}
        ok = next((t for t in ticks if t["conclusion"] == "success"), None)
        failing = ticks[:ticks.index(ok)] if ok else ticks
        first = failing[-1] if failing else None
        day = [health.when(t["created_at"]) for t in ticks
               if (now - health.when(t["created_at"])).days < 2]
        gaps = [(a - b).total_seconds() / 3600 for a, b in zip(day, day[1:])]
        return {
            "workflow_state": wf["state"],
            "latest": brief(ticks[0]) if ticks else None,
            "last_success": brief(ok),
            "failing": len(failing),
            "first_failure": first and {**brief(first), "why": health.why(r, first)},
            "ticks_48h": len(day),
            "longest_gap_h": round(max(gaps), 1) if gaps else None,
            "running": any(x["status"] != "completed" for x in runs[:5]),
        }
    section("health", tick_health)

    # -- what waits on you (the needs-you pass's own rules)
    def needs_you():
        from factory_needs_you import waiting
        items = []
        for target, (why, blocks) in waiting(snap).items():
            kind, n = target.split("#")
            n = int(n)
            issue = by.get(n, {})
            body = issue.get("body", "")
            m = re.search(r"<!-- factory-question (?:ticket=(\d+) )?(?:pr=(\d+) )?cause=(\S+)", body)
            labels = issue.get("labels", [])
            items.append({
                "kind": kind, "number": n, "title": titles.get(n, issue.get("title", "")),
                "reason": why, "blocks": blocks, "created_at": issue.get("created_at", ""),
                "labels": labels,
                "type": ("factory-question" if m else
                         "spec-question" if "spec-question" in labels else
                         "unapproved" if why.startswith("Not approved") else
                         "pr" if kind == "pr" else "other"),
                "ticket": int(m.group(1)) if m and m.group(1) else None,
                "pr": int(m.group(2)) if m and m.group(2) else None,
                "options": re.findall(r"^\s*[-*]\s*\**([A-Z])\**[:.)]\s*(.+)$", body, re.M)[:8],
                # The question's own prose: the body minus options, file line and the /decide boilerplate.
                "question": "\n".join(
                    ln for ln in re.sub(r"<!--.*?-->", "", body, flags=re.S).splitlines()
                    if ln.strip() and not re.match(r"\s*[-*]\s*\**[A-Z]\**[:.)]\s", ln)
                    and not ln.startswith(("**File:**", "Answer by commenting")))[:1500]
                    if "spec-question" in labels else "",
                "research": "research" in labels,
            })
        items.sort(key=lambda i: (-i["blocks"], i["number"]))
        # A spec question you already answered waits on the spec editor, not on you.
        out["decided"], kept = [], []
        for i in items:
            d = decision_of(i) if i["type"] == "spec-question" else None
            if d:
                out["decided"].append({"number": i["number"], "title": i["title"], **d})
            else:
                kept.append(i)
        for i in kept:
            if i["type"] == "pr":
                i.update(review_state(i["number"]))
        return kept

    def decision_of(i):
        """The first `/decide X` comment from a trusted user, or None."""
        import tickets
        cs = by.get(i["number"], {}).get("comments")
        if cs is None:  # the loader reads comments only for decisions with a parent
            cs = [{"user": c["user"]["login"], "body": c.get("body") or "", "created_at": c["created_at"]}
                  for c in gh_cache.get_all(f"repos/{name}/issues/{i['number']}/comments")]
        for c in cs:
            m = re.match(r"\s*/decide\s+(\S.*)", c["body"])
            if m and c["user"] in tickets.TRUSTED:
                return {"decision": m.group(1).strip()[:80], "at": c["created_at"]}
        return None

    me = []

    def review_state(n):
        """Your approval of the PR's current head, and its CI, for the approve-then-merge buttons."""
        if not me:
            me.append(get("user")["login"])
        pr = next(p for p in snap.prs if p["number"] == n)
        full = r(f"pulls/{n}")
        mine = [v for v in gh_cache.get_all(f"repos/{name}/pulls/{n}/reviews")
                if (v.get("user") or {}).get("login") == me[0] and v["state"] != "COMMENTED"]
        approved = bool(mine) and mine[-1]["state"] == "APPROVED" and mine[-1].get("commit_id") == pr["sha"]
        checks = r(f"commits/{pr['sha']}/check-runs?per_page=100").get("check_runs", [])
        status = r(f"commits/{pr['sha']}/status")
        bad = {"failure", "timed_out", "cancelled", "action_required", "startup_failure"}
        if any(c["conclusion"] in bad for c in checks) or status.get("state") in ("failure", "error"):
            ci = "failed"
        elif any(c["status"] != "completed" for c in checks) or (status.get("statuses") and status["state"] == "pending"):
            ci = "running"
        else:
            ci = "passed" if checks or status.get("statuses") else "none"
        return {"approved": approved, "own": (full.get("user") or {}).get("login") == me[0],
                "ci": ci, "sha": pr["sha"]}
    section("needs_you", needs_you)

    # -- started tickets with no PR (factory_stalled_tickets.state_of)
    def started():
        from factory_ready import MARKER
        from factory_stalled_tickets import state_of
        from factory_config import RETRY_BUDGET
        rows = []
        for t in sorted(snap.issues, key=lambda i: i["number"]):
            found = state_of(snap, t, now)
            if not found:
                continue
            starts = sorted(c["created_at"] for c in t["comments"] if MARKER in (c.get("body") or ""))
            last = health.when(starts[-1])
            stuck = found[0] in ("open-pr", "restart") and (now - last).total_seconds() > 3600
            rows.append({"number": t["number"], "title": t["title"], "state": found[0],
                         "detail": str(found[1]), "starts": len(starts), "budget": RETRY_BUDGET,
                         "last_started": starts[-1], "stuck": stuck})
        return rows
    section("started", started)

    # -- blocked ticket PRs and Claude's fixes (as factory_health.blocked_report)
    def blocked():
        from factory_blocked import RETRY_BUDGET, attempts, cause_of, maintainer_working
        from factory_questions import has_open_question
        from factory_rerun import is_ticket_pr, ticket_of
        rows = []
        for pr in snap.prs:
            if not (pr["head"].startswith("claude/") and is_ticket_pr(snap, pr)):
                continue
            found, tried = cause_of(snap, pr), attempts(pr)
            if not found:
                continue
            stamps = [s for s in (pr.get("head_time"), pr.get("created_at")) if s]
            since = max(health.when(s) for s in stamps) if stamps else now
            if maintainer_working(pr):
                state, what = "you", "you're on it (you pushed, or asked @claude)"
            elif has_open_question(snap, pr):
                state, what = "waiting", "Claude stopped and asked a question"
            elif any(o == "running" and sha == pr["sha"] for _, sha, o in tried):
                state, what = "fixing", f"Claude is fixing it: attempt {len(tried)} of {RETRY_BUDGET}"
            elif len(tried) >= RETRY_BUDGET or any(o == "pushed nothing" for _, _, o in tried):
                state, what = "waiting", f"Claude tried {len(tried)} time(s)"
            elif (now - since).total_seconds() > 1800:
                state, what = "stuck", "blocked over 30 min and no fix has started"
            else:
                state, what = "queued", f"a fix starts on the next tick (attempt {len(tried) + 1})"
            rows.append({"number": pr["number"], "title": titles.get(pr["number"], pr["head"]),
                         "ticket": ticket_of(snap, pr), "cause": found[0], "state": state,
                         "what": what, "since": since.isoformat()})
        return rows
    section("blocked", blocked)

    # -- open PRs at a glance
    def open_prs():
        rows = []
        for pr in snap.prs:
            v = pr.get("verdict") or {}
            runs = pr.get("runs") or []
            failed = sorted({x["name"] for x in runs if x["conclusion"] in ("failure", "timed_out")})
            pending = any(x["conclusion"] is None for x in runs)
            rows.append({"number": pr["number"], "title": titles.get(pr["number"], pr["head"]),
                         "head": pr["head"], "draft": pr.get("draft"), "labels": pr.get("labels", []),
                         "verdict": v.get("kind") if v.get("current") else None,
                         "failed": failed, "pending": pending,
                         "mergeable": pr.get("mergeable_state"), "created_at": pr.get("created_at")})
        return sorted(rows, key=lambda p: p["number"])
    section("open_prs", open_prs)

    # -- recent agent runs, with the error for the newest failures
    def agent_runs():
        rows = []
        failures = 0
        for wf in ("claude.yml", "claude-fix.yml", "claude-review.yml"):
            got = r(f"actions/workflows/{wf}/runs?per_page=15").get("workflow_runs", [])
            for x in got:
                if x["conclusion"] == "skipped":
                    continue
                row = {"workflow": wf.split(".")[0], "title": x["display_title"], "id": x["id"],
                       "status": x["status"], "conclusion": x["conclusion"],
                       "at": x["created_at"], "url": x["html_url"], "error": None, "runner": None}
                if x["conclusion"] in ("failure", "timed_out", "startup_failure") and failures < 8:
                    failures += 1
                    for job in r(f"actions/runs/{x['id']}/jobs").get("jobs", []):
                        if job.get("conclusion") not in ("failure", "timed_out"):
                            continue
                        row["runner"] = job.get("runner_name")
                        notes = [a["message"].strip() for a in r(f"check-runs/{job['id']}/annotations")
                                 if a["annotation_level"] == "failure"
                                 and not a["message"].startswith("Process completed with exit code")]
                        steps = [s["name"] for s in job.get("steps", []) if s["conclusion"] == "failure"]
                        row["error"] = (notes[0].splitlines()[-1][:300] if notes
                                        else f"failed step: {', '.join(steps)}" if steps else None)
                        break
                rows.append(row)
        rows.sort(key=lambda x: x["at"], reverse=True)
        return rows[:25]
    section("agent_runs", agent_runs)

    # -- where agent jobs run: vars.FACTORY_RUNNER, unset means GitHub-hosted ubuntu-latest
    try:
        out["runner"] = {"value": r("actions/variables/FACTORY_RUNNER").get("value")}
    except Exception:  # unset: a 404
        out["runner"] = {"value": None}

    out["requests"], out["not_modified"] = gh_cache.stats["requests"], gh_cache.stats["not_modified"]

    # -- what the next tick would do (dry: FakeBackend changes only this snapshot)
    def next_tick():
        crashed = []
        actions = tick(snap, passes(), FakeBackend(), crashed)
        return {"actions": [{"kind": a.kind, "target": a.target, "reason": a.reason} for a in actions],
                "crashed": [{"pass": n, "error": tb.strip().splitlines()[-1]} for n, tb in crashed]}
    section("next_tick", next_tick)

    try:
        rate = json.loads(os.popen("gh api rate_limit").read())["resources"]["core"]
        out["rate"] = {"remaining": rate["remaining"], "limit": rate["limit"], "reset": rate["reset"]}
    except Exception:
        out["rate"] = None
    gh_cache.save()
    out["seconds"] = round(time.time() - t0, 1)
    return out


if __name__ == "__main__":
    import contextlib
    # The factory scripts print progress; send it to stderr so stdout is only the JSON.
    with contextlib.redirect_stdout(sys.stderr):
        result = main()
    json.dump(result, sys.stdout, default=str)
