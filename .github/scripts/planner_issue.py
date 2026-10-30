#!/usr/bin/env python3
"""The planner's only way to create or change issues (claude-plan.yml).

  planner_issue.py plan --title T --body-file F       create the Plan issue
  planner_issue.py ticket --title T --body-file F     create a `planned` ticket
  planner_issue.py question --kind decision|research --title T --body-file F
  planner_issue.py edit-plan <n> --body-file F        replace a Plan issue's body

The planner has no `gh issue create` or `gh issue edit`, so a ticket whose
header the factory can't read is refused here, with the reasons, instead of
being filed and then started too early (or never). A ticket needs:

- `**Spec:**`, `**Depends on:**` and `**Files:**` lines, once each;
- a Depends on line that is exactly `none` or `#n, #m, ...`, naming issues
  that exist and aren't pull requests or Plan issues;
- at least one backticked path on the Files line, each of which exists, is
  marked `(new)`, or is named by one of the ticket's dependencies (an
  earlier ticket creates it);
- `### What` and `### Done when` sections.

Anyone else filing a ticket (the implementer's follow-ups) can use it too.
"""
import argparse
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "unused/unused")  # tickets.py reads it at import
import tickets  # noqa: E402

PATH = re.compile(r"`([^`\s]+)`(\s*\(new)?")
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def header(body, label):
    return re.findall(rf"^\*\*{re.escape(label)}:\*\*(.*)$", body, re.M)


def chain(deps, lookup, limit=100):
    """`deps` and everything they depend on, following each issue's Depends
    on line, so a ticket may name a file an earlier ticket in its chain
    creates, not only a direct dependency (#1470, #1493)."""
    out, seen, todo = [], set(), list(deps)
    while todo and len(out) < limit:
        issue = todo.pop(0)
        if issue["number"] in seen:
            continue
        seen.add(issue["number"])
        out.append(issue)
        for line in header(issue.get("body") or "", "Depends on"):
            for n in map(int, re.findall(r"#(\d+)", line)):
                if n not in seen:
                    dep = lookup(n)
                    if dep is not None and "pull_request" not in dep:
                        todo.append(dep)
    return out


def ticket_errors(body, lookup, exists):
    """Reasons `body` isn't a ticket the factory can schedule. `lookup(n)`
    returns issue n as the REST API does, or None; `exists(path)` says
    whether a repo path exists."""
    errors = []
    for label in ("Spec", "Depends on", "Files"):
        n = len(header(body, label))
        if n != 1:
            errors.append(f"needs exactly one `**{label}:**` line, found {n}")
    for section in ("### What", "### Done when"):
        if not re.search(rf"^{section}\s*$", body, re.M):
            errors.append(f"needs a `{section}` section")
    bad = tickets.depends_error(body)
    if bad:
        errors.append(f"{bad}. Put reasons in the body, and name sibling "
                      "tickets by the numbers they were created with")
    deps = []
    for line in [] if bad else header(body, "Depends on"):
        for n in map(int, re.findall(r"#(\d+)", line)):
            issue = lookup(n)
            if issue is None:
                errors.append(f"Depends on #{n}: no such issue")
            elif "pull_request" in issue:
                errors.append(f"Depends on #{n}: that is a pull request")
            elif issue["title"].startswith("Plan:"):
                errors.append(f"Depends on #{n}: that is a Plan issue, which closes only "
                              "after its tickets; name the tickets instead")
            else:
                deps.append(issue)
    from_deps = {p for d in chain(deps, lookup) for p, _ in PATH.findall(d.get("body") or "")}
    for line in header(body, "Files"):
        paths = PATH.findall(line)
        if not paths:
            errors.append("the Files line names no backticked paths")
        for p, new in paths:
            if not (new or exists(p) or p in from_deps):
                errors.append(f"Files: `{p}` doesn't exist; mark it `(new)` if this "
                              "ticket creates it, or depend on the ticket that does")
    return errors


def gh(*args):
    return subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout


def lookup(n):
    p = subprocess.run(["gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/issues/{n}"],
                       capture_output=True, text=True)
    return json.loads(p.stdout) if p.returncode == 0 else None


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["plan", "ticket", "question", "edit-plan"])
    ap.add_argument("number", nargs="?", type=int)
    ap.add_argument("--title")
    ap.add_argument("--kind", choices=["decision", "research"])
    ap.add_argument("--body-file", required=True)
    a = ap.parse_args(argv)
    body = open(a.body_file, encoding="utf-8").read()
    if a.mode == "edit-plan":
        if a.number is None or not lookup(a.number)["title"].startswith("Plan:"):
            sys.exit("edit-plan takes the number of a Plan issue")
        print(gh("issue", "edit", str(a.number), "--body-file", a.body_file), end="")
        return
    if not a.title:
        sys.exit(f"{a.mode} needs --title")
    if a.mode == "plan":
        if not a.title.startswith("Plan:") or not body.startswith("<!-- plan-source file="):
            sys.exit('A plan is titled "Plan: ..." and its body starts with the plan-source line.')
        labels = ["planned"]
    elif a.mode == "ticket":
        if a.title.startswith("Plan:"):
            sys.exit('A ticket title can\'t start with "Plan:".')
        errors = ticket_errors(body, lookup, lambda p: os.path.exists(os.path.join(ROOT, p)))
        if errors:
            sys.exit("Ticket not created:\n" + "\n".join(f"- {e}" for e in errors))
        labels = ["planned"]
    else:
        if not a.kind:
            sys.exit("question needs --kind decision or --kind research")
        if a.kind == "decision" and not body.startswith("**File:**"):
            sys.exit("A decision question's body starts with a `**File:** <file>` line.")
        labels = ["spec-question", a.kind]
    args = ["issue", "create", "--title", a.title, "--body-file", a.body_file]
    for lb in labels:
        args += ["--label", lb]
    print(gh(*args), end="")


if __name__ == "__main__":
    main(sys.argv[1:])
