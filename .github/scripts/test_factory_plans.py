"""Tests for the plan-closing pass. python3 -I test_factory_plans.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_plans import close_finished_plans  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

SRC = {"file": "docs/design/x.md", "section": "Phase 1"}


def plan(n, state="open", reason=None, src=SRC):
    return {"number": n, "title": f"Plan: {n}", "state": state, "labels": [],
            "kind": "plan", "parent": None, "plan_source": src,
            "state_reason": reason}


def ticket(n, parent, state="closed", labels=()):
    return {"number": n, "title": "t", "state": state, "labels": list(labels),
            "kind": "ticket", "parent": parent}


def run(issues):
    snap = Snapshot.from_dict({"issues": issues})
    first = tick(snap, [close_finished_plans], FakeBackend())
    return snap, first, tick(snap, [close_finished_plans], FakeBackend())


class Test(unittest.TestCase):
    def test_all_closed(self):
        snap, first, second = run([plan(1), ticket(2, 1), ticket(3, 1, labels=["spec-question"])])
        self.assertEqual([a.kind for a in first], ["close_issue", "request_phase_done"])
        self.assertEqual(first[0].payload["reason"], "completed")
        self.assertEqual(first[1].payload, SRC)
        self.assertEqual(snap.plans[0]["state"], "closed")
        self.assertEqual(second, [])

    def test_open_ticket(self):
        _, first, _ = run([plan(1), ticket(2, 1), ticket(3, 1, "open")])
        self.assertEqual(first, [])

    def test_only_questions(self):
        snap, first, second = run([plan(1), ticket(2, 1, labels=["spec-question"])])
        self.assertEqual([a.kind for a in first], ["close_issue"])
        self.assertEqual(first[0].payload["reason"], "not_planned")
        self.assertIn("All questions answered", first[0].payload["comment"])
        self.assertEqual(second, [])

    def test_closed_not_planned_left_alone(self):
        _, first, _ = run([plan(1, "closed", "not_planned"), ticket(2, 1)])
        self.assertEqual(first, [])

    def test_plan_without_design_doc_has_no_request(self):
        _, first, _ = run([plan(1, src=None), ticket(2, 1)])
        self.assertEqual([a.kind for a in first], ["close_issue"])

    def test_orphans_untouched(self):
        _, first, _ = run([ticket(2, None), plan(1), ticket(3, 1, "open")])
        self.assertEqual(first, [])

    def test_empty_plan_left_open(self):
        _, first, _ = run([plan(1)])
        self.assertEqual(first, [])


if __name__ == "__main__":
    unittest.main()
