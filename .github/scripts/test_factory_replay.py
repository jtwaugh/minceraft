"""Replay the 2026-10-19/20 stall history through one full tick. python3 -I test_factory_replay.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import GhBackend  # noqa: E402
from factory_tick import Action, FakeBackend, Snapshot, passes, tick, trusted_comments  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "history.json")


def keys(actions):
    return {(a.kind, a.target) for a in actions}


class Replay(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snap = Snapshot.from_fixture(FIXTURE)
        cls.first = tick(cls.snap, passes(), FakeBackend())
        cls.second = tick(cls.snap, passes(), FakeBackend())

    def reasons(self):
        return "\n".join(a.reason for a in self.first if a.kind == "report")

    def test_each_case_is_reported(self):
        text = self.reasons()
        for expected in ["orphan-ticket:issue#452", "orphan-ticket:issue#500", "orphan-ticket:issue#521",
                         "plan-as-dependency:issue#519",
                         "accepted-doc-without-phases:docs/design/raft-scene-feedback.md"]:
            self.assertIn(expected, text)

    def test_stalled_phases_are_reported(self):
        stalls = [a for a in self.first if a.kind == "report" and a.payload.get("kind") == "stalled"]
        self.assertTrue(any(a.target.endswith("raft-scene-feedback.md") for a in stalls))

    def test_one_report_issue_and_ready_ticket(self):
        self.assertIn(("create_issue", "new:stalled-work"), keys(self.first))
        self.assertIn(("start_ticket", "issue#521"), keys(self.first))
        self.assertNotIn(("start_ticket", "issue#519"), keys(self.first))  # depends on its plan

    def test_split_off_half_is_adopted(self):
        self.assertIn(("set_parent", "issue#521"), keys(self.first))
        self.assertEqual(next(i for i in self.snap.issues if i["number"] == 521)["parent"], 169)

    def test_second_tick_does_nothing(self):
        self.assertEqual(self.second, [])


class Live(unittest.TestCase):
    def test_untrusted_actor_refused(self):
        calls = []
        b = GhBackend("o/r", Snapshot(), run=lambda *a, **k: calls.append(a))
        with self.assertRaises(PermissionError):
            b.apply(Snapshot(), Action("add_label", "issue#1", {"label": "factory", "actor": "mallory"}))
        self.assertEqual(calls, [])

    def test_satisfied_action_makes_no_request(self):
        snap = Snapshot(issues=[{"number": 1, "labels": ["factory"], "state": "open"}])
        calls = []
        GhBackend("o/r", snap, run=lambda *a, **k: calls.append(a)).apply(
            snap, Action("add_label", "issue#1", {"label": "factory"}))
        self.assertEqual(calls, [])

    def test_only_trusted_comments_are_read(self):
        raw = [{"user": {"login": u}, "body": "Blocked by #3", "created_at": "t"}
               for u in ("jtwaugh", "mallory")]
        self.assertEqual([c["user"] for c in trusted_comments(raw)], ["jtwaugh"])


if __name__ == "__main__":
    unittest.main()
