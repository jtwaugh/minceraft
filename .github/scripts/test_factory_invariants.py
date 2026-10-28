"""Tests for the rule 3 invariant checks. python3 -I test_factory_invariants.py"""
import copy
import os
import random
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_invariants import check  # noqa: E402
from factory_tick import Snapshot  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "history.json")


def keys(snap):
    return [v.key for v in check(snap)]


def load():
    return Snapshot.from_fixture(FIXTURE)


def issue(snap, n):
    return next(i for i in snap.issues if i["number"] == n)


def phase_done(snap, f, phase):
    next(d for d in snap.docs if d["file"].endswith(f))["phases"][phase] = "done"


class Test(unittest.TestCase):
    def test_duplicate_prs(self):
        s = Snapshot(issues=[{"number": 900, "title": "t", "state": "open", "kind": "ticket",
                              "labels": [], "body": "", "parent": None}],
                     prs=[{"number": 903, "head": "claude/issue-900-sea-ice", "body": "Closes #900"},
                          {"number": 905, "head": "claude/issue-900-20261022-0243", "body": ""},
                          {"number": 906, "head": "claude/issue-901-x", "body": ""}])
        dup = [v for v in check(s) if v.kind == "duplicate-prs"]
        self.assertEqual([(v.subject, v.detail) for v in dup],
                         [("issue#900", "open PRs #903, #905; close all but one")])
        s.issues[0]["state"] = "closed"  # then it's the supersede pass's
        self.assertEqual([v for v in check(s) if v.kind == "duplicate-prs"], [])

    def test_closes_merged_to_non_master(self):
        t = {"number": 900, "title": "t", "state": "open", "kind": "ticket",
             "labels": [], "body": "", "parent": None}
        pr = {"number": 905, "base": "factory-c/x", "body": "Closes #900", "merged_at": "2026-10-20T00:00:00Z"}
        s = Snapshot(issues=[t], merged_prs=[pr])
        want = [("closes-merged-to-non-master", "issue#900")]
        got = lambda: [(v.kind, v.subject) for v in check(s) if v.kind == "closes-merged-to-non-master"]
        self.assertEqual(got(), want)
        detail = [v for v in check(s) if v.kind == "closes-merged-to-non-master"][0].detail
        self.assertIn("#905", detail)
        self.assertIn("factory-c/x", detail)
        self.assertEqual(check(s), check(s))  # stable
        pr["base"] = "master"
        self.assertEqual(got(), [])
        pr["base"] = "factory-c/x"
        t["state"] = "closed"
        self.assertEqual(got(), [])
        t["state"] = "open"
        s.merged_prs = []  # an unmerged closed PR is never loaded
        self.assertEqual(got(), [])

    def test_orphan_tickets(self):
        k = keys(load())
        for n in (452, 500, 521):
            self.assertIn(f"orphan-ticket:issue#{n}", k)
        self.assertNotIn("orphan-ticket:issue#507", k)
        self.assertNotIn("orphan-ticket:issue#470", k)  # a decision, not a ticket

    def test_plan_as_dependency(self):
        k = keys(load())
        self.assertIn("plan-as-dependency:issue#519", k)
        self.assertEqual(sum(x.startswith("plan-as-dependency") for x in k), 1)

    def test_unreadable_depends(self):
        s = load()
        self.assertFalse([x for x in keys(s) if x.startswith("unreadable-depends")])
        i = issue(s, 500)
        i["body"] = "**Depends on:** the swimming tickets in this plan\n"
        self.assertNotIn("unreadable-depends:issue#500", keys(s))  # closed: history
        i["state"] = "open"
        self.assertIn("unreadable-depends:issue#500", keys(s))

    def test_accepted_doc_without_phases(self):
        self.assertEqual([x for x in keys(load()) if x.startswith("accepted-doc")],
                         ["accepted-doc-without-phases:docs/design/raft-scene-feedback.md"])

    def test_done_phase_with_decision_and_no_ticket(self):
        s = load()
        self.assertFalse([x for x in keys(s) if "decision" in x or "done-phase" in x])
        phase_done(s, "basic-gameplay.md", "8")
        self.assertFalse([x for x in keys(s) if "decision" in x or "done-phase" in x])
        del issue(s, 500)["decision"]
        self.assertIn("answered-decision-without-ticket:issue#470", keys(s))

    def test_done_phase_with_open_ticket(self):
        s = load()
        phase_done(s, "basic-gameplay.md", "8")
        issue(s, 500)["state"] = "open"
        self.assertIn("done-phase-open-ticket:issue#500", keys(s))

    def test_clean_snapshot_has_no_violations(self):
        s = load()
        s.issues = [i for i in s.issues if i["number"] in (169, 507)]
        s.plans = [i for i in s.issues if i["kind"] == "plan"]
        s.docs = [d for d in s.docs if d["phases"]]
        self.assertEqual(check(s), [])

    def test_keys_stable_in_any_order(self):
        s = load()
        want = keys(s)
        t = copy.deepcopy(s)
        random.Random(1).shuffle(t.issues)
        random.Random(2).shuffle(t.docs)
        self.assertEqual(keys(t), want)
        self.assertEqual(len(want), len(set(want)))


if __name__ == "__main__":
    unittest.main()
