"""Tests for recording decisions, runs and attempts. python3 -I test_factory_store_records.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
import factory_store_history as history  # noqa: E402
from factory_fix import marker  # noqa: E402
from factory_store_records import sync_records  # noqa: E402
from factory_tick import Snapshot  # noqa: E402


def comment(user, body, at="2026-01-01T00:00:00Z"):
    return {"user": user, "body": body, "created_at": at}


def question(*comments):
    return {"number": 50, "title": "q", "state": "closed", "labels": ["spec-question"], "kind": "decision",
            "parent": 1, "body": "", "comments": list(comments)}


TICKET = {"number": 10, "title": "t", "state": "open", "labels": ["factory"], "kind": "ticket", "parent": 1,
          "body": "", "comments": [comment("jtwaugh", "Blocked by #50")]}


def pr(**extra):
    return {"number": 7, "head": "claude/issue-10-x", "body": "", "sha": "abc1234", "comments": [], **extra}


def snap(db, issues, prs=(), runs=()):
    s = Snapshot.from_dict({"issues": issues, "prs": list(prs)})
    s.implementer_runs, s.store = list(runs), db
    return s


class Test(unittest.TestCase):
    def setUp(self):
        self.db = factory_store.open(":memory:")
        self.db.execute("INSERT INTO plans (id) VALUES (1)")
        self.db.execute("INSERT INTO tickets (id, plan) VALUES (10, 1)")

    def decisions(self):
        return self.db.execute("SELECT issue, plan, answer, ticket FROM decisions").fetchall()

    def test_trusted_decide_makes_a_row_linked_to_its_ticket(self):
        s = snap(self.db, [question(comment("jtwaugh", "/decide option A\nbecause")), TICKET])
        sync_records(s, 1)
        self.assertEqual(self.decisions(), [(50, 1, "option A", 10)])

    def test_untrusted_decide_makes_none(self):
        sync_records(snap(self.db, [question(comment("stranger", "/decide option A")), TICKET]), 1)
        self.assertEqual(self.decisions(), [])

    def test_unlinked_decision(self):
        sync_records(snap(self.db, [question(comment("jtwaugh", "/decide B"))]), 1)
        self.assertEqual(self.decisions(), [(50, 1, "B", None)])

    def test_fix_marker_and_rerun_become_transitions(self):
        c = [comment("github-actions[bot]", marker(7, "abc1234", "nits"))]
        runs = [{"id": 99, "name": "CI", "conclusion": "failure", "run_attempt": 2, "created_at": ""}]
        s = snap(self.db, [TICKET], [pr(comments=c, runs=runs)],
                 [{"display_title": "Claude implementer #10", "status": "completed", "created_at": "t0"}])
        sync_records(s, 5)
        rows = history.history(self.db, 10)
        self.assertEqual([r[0] for r in rows], ["fix_attempt", "rerun"])
        self.assertIn("nits", rows[0][2])
        self.assertIn("CI", rows[1][2])
        self.assertEqual(sorted(r[0] for r in self.db.execute("SELECT role FROM runs")), ["fix", "implementer"])

    def test_second_sync_adds_no_rows(self):
        c = [comment("github-actions[bot]", marker(7, "abc1234", "nits"))]
        runs = [{"id": 99, "name": "CI", "conclusion": "failure", "run_attempt": 2, "created_at": ""}]
        s = snap(self.db, [question(comment("jtwaugh", "/decide A")), TICKET], [pr(comments=c, runs=runs)],
                 [{"display_title": "Claude implementer #10", "status": "completed", "created_at": "t0"}])
        self.assertGreater(sync_records(s, 1), 0)
        self.assertEqual(sync_records(s, 2), 0)


if __name__ == "__main__":
    unittest.main()
