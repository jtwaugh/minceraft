#!/usr/bin/env python3
"""Attach each open plan's tickets and questions as GitHub sub-issues.

Sweeps every open "Plan:" issue labelled `planned`. For each `#<n>` its body
mentions, in order, it attaches issue n as a sub-issue if n is a ticket or
spec question (labelled `planned` or `spec-question`, not a plan, not a pull
request) that is open and has no parent yet. Closed issues are skipped, so a
plan that only mentions old answered questions isn't mistaken for one that
raised them. Safe to run any number of times.
"""
import json
import os
import re
import subprocess

REPO = os.environ["REPO"]


def gh(*args):
    out = subprocess.run(["gh", "api", *args], check=True,
                         capture_output=True, text=True).stdout
    return json.loads(out) if out.strip() else None


def main():
    plans = [p for p in gh(f"repos/{REPO}/issues?state=open&labels=planned&per_page=100")
             if p["title"].startswith("Plan:") and "pull_request" not in p]
    for plan in plans:
        attached = {s["number"] for s in gh(f"repos/{REPO}/issues/{plan['number']}/sub_issues?per_page=100")}
        seen = []
        for n in re.findall(r"#(\d+)", plan.get("body") or ""):
            n = int(n)
            if n == plan["number"] or n in seen or n in attached:
                continue
            seen.append(n)
            issue = gh(f"repos/{REPO}/issues/{n}")
            labels = {l["name"] for l in issue.get("labels", [])}
            if ("pull_request" in issue or issue["title"].startswith("Plan:")
                    or issue["state"] != "open"
                    or not labels & {"planned", "spec-question"}
                    or issue.get("parent_issue_url")):
                continue
            gh(f"repos/{REPO}/issues/{plan['number']}/sub_issues", "-X", "POST",
               "-F", f"sub_issue_id={issue['id']}")
            print(f"Plan #{plan['number']}: attached #{n}")


if __name__ == "__main__":
    main()
