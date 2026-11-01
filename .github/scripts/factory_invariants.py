#!/usr/bin/env python3
"""Check the design rule 3 invariants over a `Snapshot`.

    factory_invariants.py [--fixture FILE]    print the violations

`check(snapshot)` returns `Violation`s and changes nothing; reporting them is
a later pass. Each has a stable key (kind plus issue or doc), the same for
the same input in any order. (docs/design/factory-orchestrator.md Phase A,
design rule 3)
"""
import os
import re
import sys
from dataclasses import dataclass

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_tick import Snapshot  # noqa: E402
os.environ.setdefault("GITHUB_REPOSITORY", "unused/unused")  # tickets.py reads it at import
from tickets import CLOSES, depends_error, ticket_numbers  # noqa: E402


@dataclass(frozen=True)
class Violation:
    kind: str  # "orphan-ticket", "plan-as-dependency", ...
    subject: str  # "issue#12" or a design-doc path
    detail: str = ""

    @property
    def key(self):
        return f"{self.kind}:{self.subject}"


def decision_phase(issue):
    """(doc file, phase) a decision issue is about, from its body excerpt."""
    body = issue.get("body_excerpt", "")
    f = re.search(r"\*\*File:\*\*\s*(\S+)", body)
    p = re.search(r"^Phase ([A-Za-z0-9]+)", body, re.M)
    return (f.group(1), p.group(1)) if f and p else None


def prs_by_ticket(snap):
    """Ticket number -> sorted numbers of the open PRs for it."""
    out = {}
    for pr in snap.prs:
        if pr.get("state", "open") == "open" and not pr.get("merged"):
            for n in ticket_numbers(pr["head"], pr.get("body")):
                out.setdefault(n, []).append(pr["number"])
    return {n: sorted(v) for n, v in out.items()}


def check(snap):
    by = {i["number"]: i for i in snap.issues}
    plans = {p["number"] for p in snap.plans}
    out = set()
    for i in snap.issues:
        if i.get("kind") != "ticket":
            continue
        n = f"issue#{i['number']}"
        # Only open work tickets: closed ones can't be adopted usefully, and
        # alerts, questions and findings issues get kind "ticket" by default.
        is_work = {"factory", "planned"} & set(i.get("labels", []))
        if i.get("state") == "open" and is_work and i.get("parent") not in plans:
            out.add(Violation("orphan-ticket", n, "belongs to no plan"))
        for d in i.get("depends_on", []):
            if d in plans:
                out.add(Violation("plan-as-dependency", n, f"depends on plan #{d}"))
        bad = i.get("state") == "open" and depends_error(i.get("body") or i.get("body_excerpt"))
        if bad:
            out.add(Violation("unreadable-depends", n, f"{bad}; not started until it is"))
    for n, prs in sorted(prs_by_ticket(snap).items()):
        if len(prs) > 1 and by.get(n, {}).get("state") == "open":
            out.add(Violation("duplicate-prs", f"issue#{n}",
                              f"open PRs {', '.join(f'#{p}' for p in prs)}; close all but one"))
    for pr in snap.merged_prs:
        if pr.get("base") == "master":
            continue
        for n in sorted({int(m) for m in CLOSES.findall(pr.get("body") or "")}):
            if by.get(n, {}).get("state") == "open":
                out.add(Violation("closes-merged-to-non-master", f"issue#{n}",
                                  f"PR #{pr['number']} (base {pr['base']}) says Closes #{n} and merged; "
                                  "close the ticket once the base has reached master"))
    for doc in snap.docs:
        if doc.get("status") == "accepted" and not doc.get("phases"):
            out.add(Violation("accepted-doc-without-phases", doc["file"]))
        for phase, state in (doc.get("phases") or {}).items():
            if state != "done":
                continue
            for d in snap.issues:
                if (d.get("kind") != "decision" or not d.get("answered")
                        or decision_phase(d) != (doc["file"], str(phase))):
                    continue
                ts = [t for t in snap.issues if t.get("decision") == d["number"]]
                if not ts:
                    out.add(Violation("answered-decision-without-ticket",
                                      f"issue#{d['number']}",
                                      f"{doc['file']} phase {phase} is done"))
                for t in ts:
                    if t["state"] != "closed":
                        out.add(Violation("done-phase-open-ticket", f"issue#{t['number']}",
                                          f"{doc['file']} phase {phase} is done"))
    return sorted(out, key=lambda v: v.key)


def main(argv):
    snap = Snapshot.from_fixture(argv[2]) if argv[1:2] == ["--fixture"] else \
        Snapshot.from_github(os.environ["GITHUB_REPOSITORY"])
    for v in check(snap):
        print(f"{v.key}: {v.detail}")


if __name__ == "__main__":
    main(sys.argv)
