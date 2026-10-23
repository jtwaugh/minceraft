#!/usr/bin/env python3
"""Is the factory running? When did it stop, and why?

    python3 .github/scripts/factory_health.py            # status
    python3 .github/scripts/factory_health.py --repro    # also run a dry tick here
    python3 .github/scripts/factory_health.py --stale-hours 8

Reads the factory tick's run history through `gh api` (REST only) and
prints: whether the workflow is enabled, when the last tick succeeded, the
first failure after it and how many followed, the error each failed tick
reported (the tick writes its traceback as an error annotation, which the
API serves without downloading logs), and the longest gap between ticks.

It also lists each blocked factory PR and what Claude is doing about it:
fixing (attempt n of the retry budget, with its run), starts on the next
tick, waiting on you (a question, or the budget is spent), or STUCK
(blocked over 30 minutes with no fix started).

Exit status: 0 healthy, 1 the latest tick failed or a blocked PR is stuck,
2 no tick has run for --stale-hours (default 12) or the workflow is disabled. A watchdog can run
this and alert on anything but 0.
"""
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

WORKFLOW = "factory-tick.yml"


def repo():
    if os.environ.get("GITHUB_REPOSITORY"):
        return os.environ["GITHUB_REPOSITORY"]
    out = subprocess.run(["gh", "repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"],
                         capture_output=True, text=True)
    return out.stdout.strip() or "jtwaugh/minceraft"


def api(path):
    out = subprocess.run(["gh", "api", path], capture_output=True, text=True)
    if out.returncode:
        raise RuntimeError(f"gh api {path}: {out.stderr.strip()}")
    return json.loads(out.stdout)


def when(s):
    return datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def ago(t, now):
    m = int((now - t).total_seconds() // 60)
    return f"{m} min ago" if m < 120 else f"{m // 60} h {m % 60} min ago" if m < 2880 \
        else f"{m // 1440} days ago"


def local(t):
    return t.astimezone().strftime("%Y-%m-%d %H:%M %Z")


def finished(runs):
    """Completed runs that ticked: cancelled ones were superseded in the
    concurrency group and skipped ones were filtered comments."""
    return [r for r in runs if r["status"] == "completed"
            and r["conclusion"] in ("success", "failure", "timed_out", "startup_failure")]


def why(r, run):
    """The failed step and the error annotations of one failed run."""
    lines = []
    for job in r(f"actions/runs/{run['id']}/jobs")["jobs"]:
        steps = [s["name"] for s in job.get("steps", []) if s["conclusion"] == "failure"]
        if steps:
            lines.append(f"failed step: {', '.join(steps)}")
        for a in r(f"check-runs/{job['id']}/annotations"):
            if a["annotation_level"] != "failure":
                continue
            msg = a["message"].strip()
            if msg.startswith("Process completed with exit code"):
                continue
            title = a.get("title") or "error"
            lines.append(f"{title}\n      " + "\n      ".join(msg.splitlines()[-12:]))
    if not any("\n" in ln for ln in lines):
        lines.append(f"no error annotation (ticks before the health check didn't write one); "
                     f"see `gh run view {run['id']} --log-failed`, or rerun this with --repro")
    return lines


STUCK_AFTER = 30 * 60  # seconds a blocked PR may wait for its first fix attempt


def blocked_report(name, now):
    """Print each blocked factory PR and what Claude is doing about it; 1 when
    one has waited over STUCK_AFTER with no fix attempt, else 0. Reads the
    repo the way the tick does, and judges it with the tick's own rules."""
    here = os.path.dirname(os.path.abspath(__file__))
    sys.path.insert(0, here)
    os.environ["GITHUB_REPOSITORY"] = name
    from factory_blocked import RETRY_BUDGET, attempts, cause_of, maintainer_working
    from factory_questions import has_open_question
    from factory_rerun import is_ticket_pr
    from factory_tick import Snapshot
    snap = Snapshot.from_github(name)
    lines, stuck = [], 0
    for pr in snap.prs:
        if not (pr["head"].startswith("claude/") and is_ticket_pr(snap, pr)):
            continue
        found, tried = cause_of(snap, pr), attempts(pr)
        if not found:
            continue
        runs = pr.get("fix_runs", [])
        last = f" ({runs[-1][1]}{', ' + runs[-1][2] if runs[-1][2] else ''})" if runs else ""
        since = when(pr["head_time"]) if pr.get("head_time") else now
        if maintainer_working(pr):
            what = "you're on it (you pushed, or asked @claude)"
        elif has_open_question(snap, pr):
            what = "WAITING ON YOU: Claude stopped and asked a question"
        elif any(o == "running" and sha == pr["sha"] for _, sha, o in tried):
            what = f"Claude is fixing it: attempt {len(tried)} of {RETRY_BUDGET}{last}"
        elif len(tried) >= RETRY_BUDGET or any(o == "pushed nothing" for _, _, o in tried):
            what = f"WAITING ON YOU: Claude tried {len(tried)} time(s){last}"
        elif (now - since).total_seconds() > STUCK_AFTER:
            what, stuck = f"STUCK: blocked for {ago(since, now)[:-4]} and no fix has started", 1
        else:
            what = f"Claude starts a fix on the next tick (attempt {len(tried) + 1} of {RETRY_BUDGET})"
        lines.append(f"    #{pr['number']} {found[0]}: {what}")
    print("\n  Blocked PRs and Claude's fixes:")
    print("\n".join(lines) if lines else "    none blocked")
    return stuck


def main(argv):
    stale_h = float(argv[argv.index("--stale-hours") + 1]) if "--stale-hours" in argv else 12
    name = repo()

    def r(path):
        return api(f"repos/{name}/{path}")

    now = datetime.now(timezone.utc)
    wf = r(f"actions/workflows/{WORKFLOW}")
    runs = []
    for page in (1, 2, 3):
        got = r(f"actions/workflows/{WORKFLOW}/runs?per_page=100&page={page}")["workflow_runs"]
        runs += got
        if len(got) < 100:
            break
    ticks = finished(runs)  # newest first
    status = 0

    print(f"Factory tick on {name}: workflow {wf['state']}")
    if wf["state"] != "active":
        print(f"  The workflow is {wf['state']}: nothing ticks until it is enabled again "
              f"(gh workflow enable {WORKFLOW}).")
        status = 2
    if not ticks:
        print("  No finished ticks in the last 300 runs.")
        return 2

    latest = ticks[0]
    lt = when(latest["created_at"])
    print(f"  latest tick:  {latest['conclusion']:8} {local(lt)} ({ago(lt, now)})  "
          f"{latest['event']}  {latest['html_url']}")
    if (now - lt).total_seconds() > stale_h * 3600:
        print(f"  STALE: no tick for over {stale_h:g} h. Scheduled runs may have been dropped "
              f"or disabled; run one by hand with `gh workflow run {WORKFLOW}`.")
        status = 2

    ok = next((t for t in ticks if t["conclusion"] == "success"), None)
    if ok:
        ot = when(ok["created_at"])
        print(f"  last success: {local(ot)} ({ago(ot, now)})  {ok['html_url']}")
    else:
        print("  last success: none in the runs read")

    failing = [t for t in ticks[:ticks.index(ok) if ok else len(ticks)]]
    if failing:
        status = max(status, 1)
        first = failing[-1]
        ft = when(first["created_at"])
        print(f"\n  BROKEN since {local(ft)} ({ago(ft, now)}): "
              f"{len(failing)} failed tick(s) in a row, none succeeded since.")
        print(f"  first failure: {first['html_url']}  ({first['event']}"
              f"{', ' + first['display_title'] if first['event'] != 'schedule' else ''})")
        for line in why(r, first):
            print(f"    {line}")
        if failing[0] is not first:
            print(f"  latest failure: {failing[0]['html_url']}")
            for line in why(r, failing[0]):
                print(f"    {line}")
    else:
        print("\n  Healthy: the latest tick succeeded.")

    day = [when(t["created_at"]) for t in ticks if (now - when(t["created_at"])).days < 2]
    gaps = [(a - b, b) for a, b in zip(day, day[1:])]
    if gaps:
        g, start = max(gaps)
        print(f"\n  {len(day)} ticks in the last 48 h; longest gap {g.total_seconds() / 3600:.1f} h "
              f"(from {local(start)}). The */5 schedule is best effort: GitHub delays "
              f"and drops scheduled runs, so events do most of the ticking.")

    status = max(status, blocked_report(name, now))

    if "--repro" in argv:
        print("\n  Dry tick here (reads GitHub, writes nothing):\n")
        here = os.path.dirname(os.path.abspath(__file__))
        env = {**os.environ, "GITHUB_REPOSITORY": name}
        rc = subprocess.run([sys.executable, "-B", os.path.join(here, "factory_tick.py"),
                             "--dry-run"], env=env).returncode
        print(f"\n  dry tick exited {rc}")
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
