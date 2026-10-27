"""Tests for deriving ticket and plan states. python3 -I test_factory_store_states.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
import factory_store_history as history  # noqa: E402
from factory_config import RETRY_BUDGET  # noqa: E402
from factory_fix import marker  # noqa: E402
from factory_ready import MARKER  # noqa: E402
from factory_store_states import sync_states, ticket_state  # noqa: E402
from factory_tick import Snapshot  # noqa: E402


def issue(n, state="open", labels=("factory",), body="", **extra):
    return {"number": n, "title": f"t{n}", "state": state, "labels": list(labels), "kind": "ticket",
            "parent": 1, "body": body, **extra}


def pr(n, ticket, **extra):
    return {"number": n, "head": f"claude/issue-{ticket}-x", "body": "", "state": "open", "sha": "abc1234",
            "comments": [], **extra}


def snap(issues, prs=(), runs=()):
    s = Snapshot.from_dict({"issues": issues, "prs": list(prs)})
    s.implementer_runs = list(runs)
    return s


def started(**extra):
    return issue(10, comments=[{"user": "github-actions[bot]", "body": MARKER, "created_at": "2026-01-01T00:00:00Z"}],
                 **extra)


BLOCKED = {"mergeable_state": "dirty"}
MARKERS = [{"body": marker(7, "abc1234", "conflict"), "created_at": f"2026-01-0{i}T00:00:00Z"}
           for i in range(1, RETRY_BUDGET + 1)]
QUESTION = {"number": 50, "title": "q", "state": "open", "labels": [], "kind": "other",
            "body": "<!-- factory-question ticket=10 cause=stop-needs-you -->"}

CASES = [
    ("proposed", snap([issue(10, labels=())])),
    ("approved", snap([issue(10)])),
    ("waiting", snap([issue(10, body="**Depends on:** #11"), issue(11)])),
    ("approved", snap([issue(10, body="**Depends on:** #11"), issue(11, state="closed")])),
    ("running", snap([started()])),
    ("running", snap([issue(10)], runs=[{"display_title": "Claude implementer #10", "status": "in_progress"}])),
    ("in_review", snap([started()], [pr(7, 10)])),
    ("merged", snap([started()], [pr(7, 10, merged=True, state="closed")])),
    ("done", snap([issue(10, state="closed")], [pr(7, 10, merged=True)])),
    ("blocked_on_question", snap([started(), QUESTION], [pr(7, 10)])),
    ("failed", snap([started()], [pr(7, 10, comments=MARKERS, **BLOCKED)])),
]


class Test(unittest.TestCase):
    def test_table(self):
        for want, s in CASES:
            with self.subTest(want=want, issues=[i["number"] for i in s.issues]):
                self.assertEqual(ticket_state(s, s.issues[0])[0], want)

    def test_all_nine_states_covered(self):
        self.assertEqual({w for w, _ in CASES}, set(history.TICKET_STATES))

    def store(self, s):
        s.store = factory_store.open(":memory:")
        s.store.execute("INSERT INTO plans (id) VALUES (1)")
        for i in s.issues:
            s.store.execute("INSERT INTO tickets (id, plan, title) VALUES (?, 1, ?)", (i["number"], i["title"]))
        return s.store

    def test_one_transition_per_change_and_repeat_adds_none(self):
        s = snap([issue(10, labels=())])
        db = self.store(s)
        self.assertEqual(sync_states(s, 1), 0)
        s.issues[0]["labels"] = ["factory"]
        self.assertEqual(sync_states(s, 2), 1)
        self.assertEqual(sync_states(s, 3), 0)
        s.prs.append(pr(7, 10))
        self.assertEqual(sync_states(s, 4), 1)
        s.prs[0]["merged"] = True
        s.issues[0]["state"] = "closed"
        self.assertEqual(sync_states(s, 5), 2)  # the ticket, then its plan
        rows = history.history(db, 10)
        self.assertEqual([r[1] for r in rows], ["approved", "in_review", "done"])
        self.assertTrue(all(r[2] for r in rows))

    def test_why_stuck_names_open_dependency(self):
        s = snap([issue(10, body="**Depends on:** #11"), issue(11)])
        db = self.store(s)
        sync_states(s, 1)
        state, reason = history.why_stuck(db, 10)
        self.assertEqual(state, "waiting")
        self.assertIn("#11", reason)

    def test_plan_built_when_every_ticket_done(self):
        s = snap([issue(10, state="closed"), issue(11)])
        db = self.store(s)
        sync_states(s, 1)
        self.assertEqual(db.execute("SELECT state FROM plans").fetchone()[0], "open")
        s.issues[1]["state"] = "closed"
        sync_states(s, 2)
        self.assertEqual(db.execute("SELECT state FROM plans").fetchone()[0], "built")
        self.assertEqual(sync_states(s, 3), 0)


if __name__ == "__main__":
    unittest.main()
