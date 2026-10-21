"""Replay the 2026-10-19/20 stall history through one full tick. python3 -I test_factory_replay.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import GhBackend  # noqa: E402
from factory_tick import Action, FakeBackend, Snapshot, passes, tick  # noqa: E402

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
    """GhBackend with an injected `run` that records requests instead of calling gh."""

    def backend(self):
        calls = []

        def run(args, project=False):
            calls.append(args)
            return "123\n"
        return GhBackend("o/r", run), calls

    def test_untrusted_actor_is_refused_without_a_request(self):
        b, calls = self.backend()
        snap = Snapshot.from_fixture(FIXTURE)
        b.apply(snap, Action("add_label", "issue#521", {"label": "nope", "actor": "mallory"}))
        self.assertEqual((calls, len(b.refused)), ([], 1))
        self.assertNotIn("nope", next(i for i in snap.issues if i["number"] == 521)["labels"])
        b.apply(snap, Action("add_label", "issue#521", {"label": "nope", "actor": "jtwaugh"}))
        self.assertEqual(len(calls), 1)

    def test_satisfied_action_makes_no_request(self):
        b, calls = self.backend()
        snap = Snapshot.from_fixture(FIXTURE)
        snap.issues[0]["labels"] = ["planned"]
        b.apply(snap, Action("add_label", f"issue#{snap.issues[0]['number']}", {"label": "planned"}))
        self.assertEqual(calls, [])

    def test_second_live_tick_writes_nothing(self):
        snap = Snapshot.from_fixture(FIXTURE)
        b, calls = self.backend()
        self.assertTrue(tick(snap, passes(), b))
        self.assertTrue(calls)
        again, calls2 = self.backend()
        self.assertEqual(tick(snap, passes(), again), [])
        self.assertEqual(calls2, [])


if __name__ == "__main__":
    unittest.main()
