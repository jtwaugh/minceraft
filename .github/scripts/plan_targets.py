#!/usr/bin/env python3
"""Decide what the planner should plan in this run.

Prints a JSON list of {"file", "section"} targets for the plan job's matrix.

- workflow_dispatch: FILE and SECTION from the inputs. For a design doc with
  no SECTION, the doc's next phase. Merges reach this as dispatch runs too:
  the Claude action can't run on push, so the push run starts them.
- push to master: SPEC.md changed -> plan the changed SPEC sections (empty
  section; the planner reads the diff). A design doc changed -> its next
  phase, if one is due.

A design doc's next phase is the first phase its header marks `todo`, in
header order. It is due unless a "Plan:" issue for it is open (being
built), or a closed one had tickets or found nothing to build (both wait
for the docs updater to mark the phase done). A closed plan that only
raised questions, all now answered, is planned again. So phases are
planned one at a time, each once the previous one is marked done.
"""
import json
import os
import re
import subprocess
import sys

REPO = os.environ.get("GITHUB_REPOSITORY", "")


def log(msg):
    print(msg, file=sys.stderr)


def header(path):
    """(status, [(phase, state), ...]) from a design doc's front matter."""
    try:
        text = open(path, encoding="utf-8").read()
    except FileNotFoundError:
        return None, []
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if not m:
        return None, []
    head = m.group(1)
    st = re.search(r"^status:\s*([a-z]+)", head, re.M)
    phases = []
    if "phases:" in head:
        block = head.split("phases:", 1)[1]
        for line in block.splitlines()[1:]:
            pm = re.match(r"^\s+([A-Za-z0-9]+):\s*([a-z]+)", line)
            if not pm:
                if line.strip() and not line.startswith((" ", "\t")):
                    break
                continue
            phases.append((pm.group(1), pm.group(2)))
    return (st.group(1) if st else None), phases


def gh_json(path):
    out = subprocess.run(["gh", "api", path], check=True,
                         capture_output=True, text=True).stdout
    return json.loads(out)


def plan_issues():
    """Every issue labelled `planned`, open or closed (plans and tickets)."""
    issues, page = [], 1
    while True:
        batch = gh_json(f"repos/{REPO}/issues?state=all&labels=planned&per_page=100&page={page}")
        issues += batch
        if len(batch) < 100:
            return issues
        page += 1


def plan_verdict(plan):
    """'built' (it has tickets), 'questions' (only spec questions, all
    closed), 'waiting' (only spec questions, some still open) or 'empty'
    (nothing to build in this repo)."""
    subs = gh_json(f"repos/{REPO}/issues/{plan['number']}/sub_issues?per_page=100")
    labels = [{l["name"] for l in s.get("labels", [])} for s in subs]
    if any("spec-question" not in ls for ls in labels):
        return "built"
    if not subs:
        return "empty"
    return "questions" if all(s["state"] == "closed" for s in subs) else "waiting"


def close_answered(plan):
    """Close a plan whose questions are all answered, so it is planned anew."""
    subprocess.run(["gh", "api", f"repos/{REPO}/issues/{plan['number']}", "-X", "PATCH",
                    "-f", "state=closed", "-f", "state_reason=not_planned"],
                   check=True, capture_output=True)
    subprocess.run(["gh", "issue", "comment", str(plan["number"]), "--repo", REPO, "--body",
                    "All questions answered. Closing this plan and planning the phase again."],
                   check=True, capture_output=True)


def next_phase(path, plans):
    status, phases = header(path)
    if status != "accepted":
        log(f"{path}: status is {status!r}, not accepted; skipping.")
        return None
    if not phases:
        log(f"{path}: no phases in the header; give a section to plan it.")
        return None
    todo = [p for p, s in phases if s == "todo"]
    if not todo:
        log(f"{path}: every phase is done.")
        return None
    section = f"Phase {todo[0]}"
    marker = f"plan-source file={path} section={section} -->"
    mine = [p for p in plans
            if p["title"].startswith("Plan:") and marker in (p.get("body") or "")]
    if not mine:
        log(f"{path}: next is {section}.")
        return {"file": path, "section": section}
    # Only the newest plan for this phase decides; older ones are history.
    latest = max(mine, key=lambda p: p["number"])
    v = plan_verdict(latest)
    if v == "built":
        log(f"{path}: {section} has tickets (#{latest['number']}); waiting for the phase to be marked done.")
        return None
    if v == "empty":
        log(f"{path}: {section} had nothing to build here (#{latest['number']}); waiting for it to be marked done.")
        return None
    if v == "waiting":
        log(f"{path}: {section}'s plan #{latest['number']} has open questions; waiting for answers.")
        return None
    # Only questions, all answered.
    if latest["state"] == "open":
        # Often the same merge that answered the last question started this
        # run: retire the plan and plan the phase again.
        close_answered(latest)
        log(f"{path}: closed plan #{latest['number']} (questions answered).")
    log(f"{path}: {section}'s plan only raised questions, now answered; planning again.")
    return {"file": path, "section": section}


def is_design_doc(path):
    return (path.startswith("docs/design/") and path.endswith(".md")
            and not path.endswith("README.md"))


def main():
    event = os.environ["EVENT"]
    targets = []
    plans = None

    def plans_once():
        nonlocal plans
        if plans is None:
            plans = plan_issues()
        return plans

    if event == "workflow_dispatch":
        f = os.environ.get("FILE") or "SPEC.md"
        section = os.environ.get("SECTION", "").strip()
        if f == "SPEC.md":
            # An empty section is allowed when a SPEC.md merge started this
            # run (BEFORE/AFTER set): the planner plans what the diff touched.
            if not section and not (os.environ.get("BEFORE") and os.environ.get("AFTER")):
                print("::error::Give a section to plan from SPEC.md.")
                sys.exit(1)
            targets.append({"file": f, "section": section})
        elif is_design_doc(f):
            status, _ = header(f)
            if status != "accepted":
                print(f"::error::{f} has status {status!r}. Only accepted design docs are planned.")
                sys.exit(1)
            if section:
                targets.append({"file": f, "section": section})
            else:
                t = next_phase(f, plans_once())
                if t:
                    targets.append(t)
        else:
            print("::error::file must be SPEC.md or docs/design/<name>.md")
            sys.exit(1)

    elif event == "push":
        before, after = os.environ["BEFORE"], os.environ["AFTER"]
        if set(before) == {"0"}:
            log("New branch push; nothing to compare.")
        else:
            diff = subprocess.run(
                ["git", "diff", "--name-only", before, after],
                capture_output=True, text=True)
            if diff.returncode != 0:
                # e.g. a force-push whose old head isn't in the checkout.
                log(f"Can't diff {before}..{after}; planning nothing. Run the planner by hand.")
                print("[]")
                return
            changed = diff.stdout.split()
            if "SPEC.md" in changed:
                targets.append({"file": "SPEC.md", "section": ""})
            for f in changed:
                if is_design_doc(f):
                    t = next_phase(f, plans_once())
                    if t:
                        targets.append(t)

    print(json.dumps(targets))


if __name__ == "__main__":
    main()
