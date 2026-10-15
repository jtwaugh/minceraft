#!/usr/bin/env python3
"""Decide what the planner should plan in this run.

Prints a JSON list of {"file", "section"} targets for the plan job's matrix.

- workflow_dispatch: FILE and SECTION from the inputs. For a design doc with
  no SECTION, the doc's next phase.
- push to master: SPEC.md changed -> plan the changed SPEC sections (empty
  section; the planner reads the diff). A design doc changed -> its next
  phase, if one is due.

A design doc's next phase is the first phase its header marks `todo`, in
header order. It is due only if no "Plan:" issue exists for it yet: an open
plan means the phase is being built, and a closed plan with the phase still
`todo` waits for the maintainer. So phases are planned one at a time, each
once the previous one is marked done.
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


def plan_bodies():
    """Bodies of every issue labelled `planned`, open or closed."""
    out = subprocess.run(
        ["gh", "api", "--paginate",
         f"repos/{REPO}/issues?state=all&labels=planned&per_page=100",
         "--jq", ".[] | .body // \"\""],
        check=True, capture_output=True, text=True).stdout
    return out


def next_phase(path, bodies):
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
    if marker in bodies:
        log(f"{path}: {section} already has a plan; waiting for it to be marked done.")
        return None
    log(f"{path}: next is {section}.")
    return {"file": path, "section": section}


def is_design_doc(path):
    return (path.startswith("docs/design/") and path.endswith(".md")
            and not path.endswith("README.md"))


def main():
    event = os.environ["EVENT"]
    targets = []
    bodies = None

    def bodies_once():
        nonlocal bodies
        if bodies is None:
            bodies = plan_bodies()
        return bodies

    if event == "workflow_dispatch":
        f = os.environ.get("FILE") or "SPEC.md"
        section = os.environ.get("SECTION", "").strip()
        if f == "SPEC.md":
            if not section:
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
                t = next_phase(f, bodies_once())
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
            changed = subprocess.run(
                ["git", "diff", "--name-only", before, after],
                check=True, capture_output=True, text=True).stdout.split()
            if "SPEC.md" in changed:
                targets.append({"file": "SPEC.md", "section": ""})
            for f in changed:
                if is_design_doc(f):
                    t = next_phase(f, bodies_once())
                    if t:
                        targets.append(t)

    print(json.dumps(targets))


if __name__ == "__main__":
    main()
