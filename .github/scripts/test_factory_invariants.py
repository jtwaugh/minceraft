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
