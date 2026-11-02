#!/usr/bin/env python3
"""Tick pass: flag what waits on the maintainer.

Every open issue and PR that can't move until the maintainer acts gets the
`needs-you` label, and loses it once it can move again. The reasons are
listed, most-blocking first, in one "Factory: needs you" issue, which is
closed when nothing waits. Filter the board, issues or PRs by
`label:needs-you` to see the queue. Level-triggered: the label is a view of
the snapshot, so a label added by hand is removed on the next tick.
(docs/design/factory-orchestrator.md rule 7, Phase E: a first queue)

Waits on the maintainer:
- a ticket PR the reviewer blocked, or whose check failed again after its
  re-run, once Claude's fix attempts can't help: a question is open, the
  retry budget is spent, or the fix pushed nothing (factory_blocked.py
  gets the first try);
- a ticket PR at "Ready, with nits" while the nits pass can't run on it;
- a ticket PR that is Ready but can't merge by itself: it touches a
  CODEOWNERS path, or another open PR is for the same ticket;
- a ticket PR over the size limit;
- a PR whose checks wait on approval (`action_required`): GitHub won't run
  workflows on a PR the tick opened with the workflow token until the
  maintainer approves them, so no verdict ever comes;
- an open PR that isn't a factory ticket (design docs, spec, factory
  changes): only the maintainer merges those;
- an open `spec-question`, and a question the tick opened
  (`<!-- factory-question`);
- an open `planned` ticket nobody has labelled `factory`;
- the "Factory: stalled work" issue while it is open.
A ticket shows its PR's reason. Drafts, `hold`, `local` and `superseded`
items are left alone: the maintainer already chose to park them.
"""
import os
import re
import sys
import time
from datetime import datetime

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_automerge import owned, why_not  # noqa: E402
from factory_config import NEEDS_YOU_ALERT_HOURS  # noqa: E402
from factory_rerun import FAILED, is_ticket_pr, ticket_of  # noqa: E402
from factory_tick import REPORT_TITLE, Action  # noqa: E402

LABEL = "needs-you"
QUEUE_TITLE = "Factory: needs you"
PARKED = {"hold", "local", "superseded"}
MERGE_YOURSELF = {
    "touches a CODEOWNERS path": "Ready, but it touches files only you can approve: review and merge it",
    "CODEOWNERS unreadable": "Ready, but the tick couldn't read CODEOWNERS: merge it yourself",
}


def pr_reason(snap, pr):
    """Why this open PR waits on the maintainer, or None."""
    if pr.get("draft") or pr.get("merged") or PARKED & set(pr.get("labels", [])):
        return None
    if any(r["conclusion"] == "action_required" for r in pr.get("runs", [])):
        return "Its checks wait on your approval: open the PR and approve the workflows"
    if not is_ticket_pr(snap, pr):
        return "Not a factory ticket: review and merge it, or close it"
    fixing = claude_fixing(snap, pr)
    for run in pr.get("runs", []):
        if run["conclusion"] in FAILED and run["name"] == "PR size":
            return "Over 200 reviewable lines: split it, or label it `oversize-ok`"
        if run["conclusion"] in FAILED and run["run_attempt"] > 1 and not fixing:
            return f"{run['name']} failed again after a re-run, and Claude couldn't fix it: look at the log"
    v = pr.get("verdict")
    if not v or not v.get("current"):
        return None  # review still running
    if v["kind"] == "blocked" and not fixing:
        return "Reviewer blocked it and Claude couldn't fix it: see the question, or `/override <reason>`"
    if v["kind"] == "nits" and not v.get("nits"):
        return "Ready, with nits: merge it, or comment `@claude fix the nits`"
    if v["kind"] == "ready":
        # Before the merge state: a PR waiting on a CODEOWNERS review reads "blocked".
        if snap.codeowners and any(owned(f, snap.codeowners) for f in pr.get("files", [])):
            return MERGE_YOURSELF["touches a CODEOWNERS path"]
        why = why_not(snap, pr) or ""
        if why.startswith("another open PR"):
            return f"Two open PRs for one ticket ({why[why.index('#'):-1]} and this): close one"
        return MERGE_YOURSELF.get(why)
    return None


def claude_fixing(snap, pr):
    """True while the Blocked-fix pass has this PR in hand: a fix is running
    or will start this tick. It isn't once the maintainer is working on it,
    a question is open, or the retry budget is spent (then the question
    issue is flagged, and so is the PR)."""
    from factory_blocked import RETRY_BUDGET, attempts, cause_of, maintainer_working
    from factory_questions import has_open_question
    if not pr["head"].startswith("claude/") or cause_of(snap, pr) is None:
        return False
    if maintainer_working(pr):
        return True  # the maintainer pushed or asked @claude: that run is the attempt
    if has_open_question(snap, pr):
        return False
    tried = attempts(pr)
    return len(tried) < RETRY_BUDGET and not any(o == "pushed nothing" for _, _, o in tried)


def issue_reason(i):
    """Why this open issue waits on the maintainer, or None (its PR aside)."""
    labels = set(i["labels"])
    if PARKED & labels:
        return None
    if "<!-- factory-question" in i.get("body", ""):
        return "The factory asked you a question: answer it here"
    if "spec-question" in labels:
        if "research" in labels:
            return "Research question: answer it from your local notes"
        return "Question: comment `/decide <option>`, or merge the spec editor's PR if you already did"
    if i["title"] == REPORT_TITLE:
        return "The tick found stalled work: see the list"
    if i["kind"] == "ticket" and "planned" in labels and "factory" not in labels:
        return "Not approved: label it `factory` to build it, or close it"
    return None


def waiting(snap):
    """{target: (reason, blocked count)} for everything waiting on the maintainer."""
    out = {}
    for pr in snap.prs:
        why = pr_reason(snap, pr)
        if why:
            out[f"pr#{pr['number']}"] = why
            t = ticket_of(snap, pr)
            if t:
                out.setdefault(f"issue#{t}", f"PR #{pr['number']}: {why}")
    reports = sorted(i["number"] for i in snap.issues
                     if i["title"] == REPORT_TITLE and i["state"] == "open")
    for i in snap.issues:
        if i["state"] != "open" or i["title"] == QUEUE_TITLE:
            continue
        if i["title"] == REPORT_TITLE and i["number"] != reports[0]:
            continue  # one report is enough
        why = issue_reason(i)
        if why:
            out.setdefault(f"issue#{i['number']}", why)
    blocks = {}
    for i in snap.issues:
        if i["state"] == "open":
            for d in i.get("depends_on", []):
                blocks[d] = blocks.get(d, 0) + 1
    ticket = {f"pr#{p['number']}": ticket_of(snap, p) for p in snap.prs}
    return {t: (why, blocks.get(ticket.get(t) or int(t.split("#")[1]), 0))
            for t, why in out.items()}


def clock():
    return time.time()


def since(stamp):
    try:
        return datetime.fromisoformat(stamp.replace("Z", "+00:00")).timestamp()
    except (AttributeError, ValueError):
        return None


def age_text(seconds):
    """Whole hours, then whole days from 48h, so the text changes at most hourly."""
    hours = max(int(seconds // 3600), 0)
    return "waiting <1h" if hours < 1 else f"waiting {hours}h" if hours < 48 else f"waiting {hours // 24}d"


def waits(snap, items, now):
    """{target: seconds} for each waiting item: time since it started waiting
    (store), else since it was created."""
    created = {f"issue#{i['number']}": i.get("created_at") for i in snap.issues}
    created.update({f"pr#{p['number']}": p.get("created_at") for p in snap.prs})
    if snap.store is not None:
        from factory_store_history import record_wait, time_in_state, waiting_subjects
        nums = {t: int(t.split("#")[1]) for t in items}
        for t, n in sorted(nums.items()):
            record_wait(snap.store, n, True, items[t][0], now)
        for n in sorted(waiting_subjects(snap.store) - set(nums.values())):
            record_wait(snap.store, n, False, "no longer waits on you", now)
        return {t: time_in_state(snap.store, n, now, "needs_you") for t, n in nums.items()}
    return {t: now - c for t in items if (c := since(created.get(t))) is not None}


def overdue(seconds):
    return seconds > NEEDS_YOU_ALERT_HOURS * 3600


def queue_body(items, ages=None, late=()):
    if not items:
        return ""
    lines = []
    for t, (why, n) in sorted(items.items(), key=lambda kv: (-kv[1][1], kv[0])):
        ref = "#" + t.split("#")[1]
        lines.append(f"- {ref}: {why}" + (f" (blocks {n} ticket{'s' * (n != 1)})" if n else "")
                     + (f" ({age_text(ages[t])})" if t in (ages or {}) else "")
                     + (" **overdue**" if t in late else ""))
    return ("Everything the factory can't move until you act, most-blocking first. "
            f"The same items carry the `{LABEL}` label. Rewritten by the factory tick "
            "(`.github/scripts/factory_needs_you.py`), closed when nothing waits.\n\n"
            + "\n".join(lines) + "\n")


def alert_marker(target, start):
    return f"<!-- needs-you-alert {target} since={int(start)} -->"


def alerted(queue, target, start):
    """True if the queue issue already carries an alert for this wait: the
    same item, and a start within a minute (a later wait starts later)."""
    for c in queue.get("comments", []):
        for t, at in re.findall(r"<!-- needs-you-alert (\S+) since=(\d+) -->", c.get("body", "")):
            if t == target and abs(int(at) - start) < 60:
                return True
    return False


def needs_you(snap):
    items = waiting(snap)
    out = []
    labelled = {f"issue#{i['number']}" for i in snap.issues
                if i["state"] == "open" and LABEL in i["labels"]}
    labelled |= {f"pr#{p['number']}" for p in snap.prs
                 if LABEL in p.get("labels", []) and not p.get("merged")}
    for t in sorted(items.keys() - labelled):
        out.append(Action("add_label", t, {"label": LABEL}, items[t][0]))
    for t in sorted(labelled - items.keys()):
        out.append(Action("remove_label", t, {"label": LABEL}, "nothing waits on you here now"))
    now = clock()
    secs = waits(snap, items, now)
    late = {t for t, w in secs.items() if overdue(w)}
    text = queue_body(items, secs, late)
    mine = sorted((i for i in snap.issues if i["title"] == QUEUE_TITLE and i["state"] == "open"),
                  key=lambda i: i["number"])
    if not mine:
        if text:
            out.append(Action("create_issue", "new:needs-you",
                              {"title": QUEUE_TITLE, "body": text, "assignee": "jtwaugh"},
                              f"{len(items)} item(s) wait on you"))
    elif not text:
        out.append(Action("close_issue", f"issue#{mine[0]['number']}", {}, "nothing waits on you"))
    elif mine[0].get("body") != text:
        out.append(Action("update_issue", f"issue#{mine[0]['number']}", {"body": text},
                          f"{len(items)} item(s) wait on you"))
    if mine:
        for t in sorted(late):
            start = now - secs[t]
            if not alerted(mine[0], t, start):
                body = (f"{alert_marker(t, start)}\n#{t.split('#')[1]} has waited on you "
                        f"{age_text(secs[t])[8:]} (alert after {NEEDS_YOU_ALERT_HOURS}h): {items[t][0]}")
                out.append(Action("comment", f"issue#{mine[0]['number']}", {"body": body},
                                  f"{t} is overdue"))
    return out
