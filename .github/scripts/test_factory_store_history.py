"""Tests for the transition log. python3 -I test_factory_store_history.py"""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
import factory_store_history as h  # noqa: E402


class Test(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.db = factory_store.open(os.path.join(self.dir.name, "store.db"))
        self.addCleanup(self.db.close)
        self.db.execute("INSERT INTO plans (id) VALUES (1)")
        self.db.execute("INSERT INTO tickets (id, plan) VALUES (2, 1)")
        self.db.commit()

    def state(self, table, i):
        return self.db.execute(f"SELECT state FROM {table} WHERE id = ?", (i,)).fetchone()[0]

    def test_unknown_state_refused(self):
        with self.assertRaises(ValueError):
            h.record_transition(self.db, 2, "bogus", "x", 1)
        with self.assertRaises(ValueError):
            h.record_transition(self.db, 2, "built", "plan state on a ticket", 1)
        with self.assertRaises(ValueError):
            h.record_transition(self.db, 1, "running", "ticket state on a plan", 1)
        with self.assertRaises(ValueError):
            h.record_transition(self.db, 99, "done", "no such subject", 1)
        self.assertEqual(h.history(self.db, 2), [])

    def test_same_transition_twice_one_row(self):
        self.assertTrue(h.record_transition(self.db, 2, "approved", "maintainer", 10))
        self.assertFalse(h.record_transition(self.db, 2, "approved", "again", 20))
        self.assertEqual(len(h.history(self.db, 2)), 1)

    def test_queries_over_several_states(self):
        for to, why, at in (("approved", "labelled", 10), ("waiting", "dep #1 open", 20),
                            ("running", "started", 50)):
            h.record_transition(self.db, 2, to, why, at)
        self.assertEqual([r[1] for r in h.history(self.db, 2)], ["approved", "waiting", "running"])
        self.assertEqual(h.why_stuck(self.db, 2), ("running", "started"))
        self.assertEqual(h.time_in_state(self.db, 2, 80), 30)
        self.assertEqual(self.state("tickets", 2), "running")

    def test_never_moved(self):
        self.assertIsNone(h.why_stuck(self.db, 2))
        self.assertIsNone(h.time_in_state(self.db, 2, 5))

    def test_plan_states(self):
        h.record_transition(self.db, 1, "built", "last ticket closed", 5)
        self.assertEqual(self.state("plans", 1), "built")
        self.assertEqual(h.why_stuck(self.db, 1), ("built", "last ticket closed"))

    def test_events_keep_order_and_state(self):
        h.record_transition(self.db, 2, "in_review", "PR opened", 10)
        h.record_event(self.db, 2, "rerun", "clippy", 20)
        h.record_event(self.db, 2, "fix_attempt", "conflict", 30)
        h.record_event(self.db, 2, "rerun", "tests", 40)
        self.assertEqual([(r[0], r[2]) for r in h.history(self.db, 2)],
                         [("transition", "PR opened"), ("rerun", "clippy"),
                          ("fix_attempt", "conflict"), ("rerun", "tests")])
        self.assertEqual(self.state("tickets", 2), "in_review")
        self.assertEqual(h.why_stuck(self.db, 2), ("in_review", "PR opened"))
        self.assertEqual(h.time_in_state(self.db, 2, 100), 90)
        with self.assertRaises(ValueError):
            h.record_event(self.db, 2, "merged", "x", 50)


if __name__ == "__main__":
    unittest.main()
