#!/usr/bin/env python3
"""Create the Plan issue, tickets and spec questions from a planner result.

    python3 -I planner_apply.py <result.json>

The result is checked against the planner schema first; an invalid one creates
nothing. Then, through planner_issue.py (so every ticket still passes its
header checks): the Plan issue, each ticket in order with its `**Depends on:**`
line rewritten to the numbers of the tickets it waits on, each question, and
finally the Plan body listing them in build order, questions last. When an
open Plan with the same plan-source line exists, nothing is created.
(docs/design/factory-orchestrator.md Phase C)
"""
import json
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import agent_results  # noqa: E402

DEPENDS = re.compile(r"^\*\*Depends on:\*\*.*$", re.M)


def source_line(plan):
    return f"<!-- plan-source file={plan['file']} section={plan['section']} -->"


def open_plan_exists(plan, run=agent_results.gh):
    code, out = run("issue", "list", "--state", "open", "--label", "planned",
                    "--limit", "500", "--json", "title,body")
    if code != 0:
        raise RuntimeError("could not list open plans")
    return any(i["title"].startswith("Plan:") and source_line(plan) in (i["body"] or "")
               for i in json.loads(out))


def with_depends(body, numbers):
    line = "**Depends on:** " + (", ".join(f"#{n}" for n in numbers) or "none")
    return DEPENDS.sub(lambda _: line, body, count=1) if DEPENDS.search(body) else body


def plan_body(plan, tickets, questions):
    lines = [source_line(plan), "", plan["notes"].strip(), "", "Build order:", ""]
    lines += [f"{i}. #{n} {t['title']}" for i, (n, t) in enumerate(tickets, 1)]
    if not tickets:
        lines.append("Nothing to build here.")
    if questions:
        lines += ["", "Questions:", ""] + [f"- #{n} {q['title']}" for n, q in questions]
    return "\n".join(lines) + "\n"


def create(mode, title, body, extra=(), script=None):
    """Run planner_issue.py; returns the new issue's number."""
    with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8") as f:
        f.write(body)
    try:
        args = [sys.executable, "-I", script or os.path.join(HERE, "planner_issue.py"), mode,
                *([] if mode == "edit-plan" else ["--title", title]), *extra,
                "--body-file", f.name]
        out = subprocess.run(args, check=True, capture_output=True, text=True).stdout
    finally:
        os.unlink(f.name)
    return int(re.search(r"/issues/(\d+)\s*$", out.strip()).group(1)) if mode != "edit-plan" else 0


def apply(text, run=agent_results.gh, script=None):
    """Create everything a valid planner result asks for. Returns the Plan
    number, or None when nothing was created."""
    errors = agent_results.validate("planner", text)
    if errors:
        raise ValueError("invalid planner result:\n" + "\n".join(errors))
    result = json.loads(text)
    if "needs_maintainer" in result:
        return None
    plan = result["plan"]
    if open_plan_exists(plan, run):
        return None
    plan_no = create("plan", plan["title"], source_line(plan) + "\n\nCreating tickets.\n",
                     script=script)
    try:
        made = []
        for t in result["tickets"]:
            body = with_depends(t["body"], [made[d - 1][0] for d in t["depends_on"]])
            made.append((create("ticket", t["title"], body, script=script), t))
        asked = [(create("question", q["title"], q["body"], ["--kind", q["kind"]], script=script), q)
                 for q in result["questions"]]
        create("edit-plan", "", plan_body(plan, made, asked), [str(plan_no)], script=script)
    except subprocess.CalledProcessError as e:
        raise ValueError(f"{(e.stderr or '').strip()}\nPlan #{plan_no} was left open with the "
                         "\"Creating tickets.\" body; close it before rerunning.") from e
    return plan_no


def main(argv):
    if len(argv) != 2:
        print("usage: planner_apply.py <result.json>", file=sys.stderr)
        return 2
    try:
        n = apply(open(argv[1], encoding="utf-8").read())
    except ValueError as e:
        print(e, file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as e:
        print((e.stderr or "").strip(), file=sys.stderr)
        return 1
    print(f"created Plan #{n}" if n else "created nothing", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
