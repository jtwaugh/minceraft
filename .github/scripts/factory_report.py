#!/usr/bin/env python3
"""Tick pass: keep the one "Factory: stalled work" issue in step with the findings.

    factory_report.py [--fixture FILE]    print the actions

Invariant violations and stalls become one issue body, grouped by kind and
sorted, so the same findings give the same text in any input order. While
anything is found the issue is created or updated (only if the body
changed); once nothing is found it is closed.
(docs/design/factory-orchestrator.md Phase A, design rule 2)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_invariants import check  # noqa: E402
from factory_stalls import messages  # noqa: E402
from factory_tick import REPORT_TITLE, Action, Snapshot  # noqa: E402

INTRO = ("Found by the factory tick (`.github/scripts/factory_report.py`). "
         "This issue is rewritten when the findings change and closed when "
         "nothing is stalled.")


def body(snap):
    """The report text, or "" when there are no findings."""
    groups = {}
    for v in check(snap):
        groups.setdefault(v.kind, []).append(
            f"- `{v.key}`" + (f": {v.detail}" if v.detail else ""))
    stalls = sorted(f"- {m}" for _, m in messages(snap))
    if stalls:
        groups["stalled"] = stalls
    if not groups:
        return ""
    parts = [INTRO]
    for kind in sorted(groups):
        parts.append(f"## {kind}\n" + "\n".join(sorted(groups[kind])))
    return "\n\n".join(parts) + "\n"


def report(snap):
    """Pass: create, update or close the stalled-work issue."""
    text = body(snap)
    mine = sorted((i for i in snap.issues
                   if i["title"] == REPORT_TITLE and i["state"] == "open"),
                  key=lambda i: i["number"])
    if not mine:
        return [Action("create_issue", "new:stalled-work",
                       {"title": REPORT_TITLE, "body": text},
                       "findings, no open report")] if text else []
    n = f"issue#{mine[0]['number']}"
    if not text:
        return [Action("close_issue", n, {}, "nothing is stalled now")]
    if mine[0].get("body") != text:
        return [Action("update_issue", n, {"body": text}, "findings changed")]
    return []


def main(argv):
    snap = Snapshot.from_fixture(argv[2]) if argv[1:2] == ["--fixture"] else \
        Snapshot.from_github(os.environ["GITHUB_REPOSITORY"])
    for a in report(snap):
        print(f"{a.kind} {a.target}: {a.reason}")


if __name__ == "__main__":
    main(sys.argv)
