"""Tests for the plan-closing pass reading the store. python3 -I test_factory_plans_store.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
from factory_plans import close_finished_plans  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

SRC = {"file": "docs/design/x.md", "section": "Phase 1"}
PLAN = {"number": 1, "title": "Plan: 1", "state": "open", "labels": [], "kind": "plan",
        "parent": None, "plan_source": SRC}


def snap_with(tickets, decisions=(), children_closed=False):
    """Plan 1 in the store with tickets {id: state} and decisions
    [(issue, answer, ticket)]. GitHub's sub-issues stay open unless asked."""
    conn = factory_store.open(":memory:")
    conn.execute("INSERT INTO plans (id) VALUES (1)")
    for n, state in tickets.items():
        conn.execute("INSERT INTO tickets (id, plan, state) VALUES (?, 1, ?)", (n, state))
    for d in decisions:
        conn.execute("INSERT INTO decisions (issue, plan, answer, ticket) VALUES (?, 1, ?, ?)", d)
    state = "closed" if children_closed else "open"
    issues = [dict(PLAN)] +[{"number": n, "title": "t", "state": state, "labels": [],
                        "kind": "ticket", "parent": 1} for n in tickets]
    snap = Snapshot.from_dict({"issues": issues})
    snap.store = conn
    return snap


def run(snap):
    first = tick(snap, [close_finished_plans], FakeBackend())
    return first, tick(snap, [close_finished_plans], FakeBackend())


class Test(unittest.TestCase):
    def test_all_done_and_linked_closes(self):
        snap = snap_with({2: "done", 3: "done"}, [(9, "a", 2)])
        first, second = run(snap)
        self.assertEqual([a.kind for a in first], ["close_issue", "request_phase_done"])
        self.assertEqual(first[1].payload, SRC)
        self.assertEqual(second, [])

    def test_store_decides_not_github(self):
        snap = snap_with({2: "done"})
        first, _ = run(snap)
        self.assertEqual(first[0].kind, "close_issue")

    def test_ticket_not_done_holds(self):
        snap = snap_with({2: "done", 3: "merged"}, children_closed=True)
        first, _ = run(snap)
        self.assertEqual(first, [])

    def test_unlinked_decision_reported_once(self):
        snap = snap_with({2: "done"}, [(9, "a", None), (8, None, None)])
        first, second = run(snap)
        self.assertEqual([a.kind for a in first], ["report"])
        self.assertIn("#9", first[0].reason)
        self.assertNotIn("#8", first[0].reason)
        self.assertEqual(second, [])
        self.assertEqual(snap.plans[0]["state"], "open")

    def test_unstored_plan_uses_sub_issues(self):
        snap = snap_with({2: "proposed"}, children_closed=True)
        snap.store.execute("DELETE FROM tickets")
        first, _ = run(snap)
        self.assertEqual([a.kind for a in first], ["close_issue", "request_phase_done"])


if __name__ == "__main__":
    unittest.main()
