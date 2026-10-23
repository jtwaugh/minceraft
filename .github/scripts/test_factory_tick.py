"""Tests for the factory tick skeleton. python3 -I test_factory_tick.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_tick import Action, FakeBackend, Snapshot, tick  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURE = os.path.join(HERE, "fixtures", "history.json")


def label_orphans(snap):
    """Dummy pass: tickets with no parent get the `orphan` label."""
    return [Action("add_label", f"issue#{i['number']}", {"label": "orphan"},
                   "no parent plan")
            for i in snap.issues
            if i["kind"] == "ticket" and i["parent"] is None
            and "orphan" not in i["labels"]]


def adopt(snap):
    return [Action("set_parent", f"issue#{i['number']}",
                   {"parent": i["expected_parent"]}, "adopt")
            for i in snap.issues if i.get("parent") is None and i.get("expected_parent")]


class Test(unittest.TestCase):
    def test_loads_fixture(self):
        s = Snapshot.from_fixture(FIXTURE)
        self.assertTrue(s.issues and s.docs and s.branches)
        self.assertEqual({p["kind"] for p in s.plans}, {"plan"})
        self.assertIn(287, {p["number"] for p in s.plans})

    def test_tick_is_idempotent(self):
        s = Snapshot.from_fixture(FIXTURE)
        first = tick(s, [label_orphans, adopt], FakeBackend())
        self.assertTrue(first)
        self.assertEqual(tick(s, [label_orphans, adopt], FakeBackend()), [])

    def test_no_passes_no_actions(self):
        self.assertEqual(tick(Snapshot(), [], FakeBackend()), [])

    def test_duplicate_actions_applied_once(self):
        s = Snapshot.from_fixture(FIXTURE)
        n = sum(1 for i in s.issues if i["kind"] == "ticket" and i["parent"] is None)
        self.assertEqual(len(tick(s, [label_orphans, label_orphans], FakeBackend())), n)

    def test_unhandled_action_fails(self):
        s = Snapshot()
        with self.assertRaises(ValueError):
            tick(s, [lambda _: [Action("nope", "x")]], FakeBackend())

    def test_crashed_pass_is_recorded_and_the_rest_run(self):
        s = Snapshot.from_fixture(FIXTURE)

        def broken(_):
            raise KeyError("commit")

        crashed = []
        applied = tick(s, [broken, label_orphans], FakeBackend(), crashed)
        self.assertTrue(applied)  # label_orphans still ran
        self.assertEqual([n for n, _ in crashed], ["broken"])
        self.assertIn("KeyError", crashed[0][1])

    def test_auto_merge_skipped_after_a_crash(self):
        ran = []

        def broken(_):
            raise RuntimeError("x")

        def auto_merge(_):
            ran.append("merge")
            return []

        crashed = []
        tick(Snapshot(), [broken, auto_merge], FakeBackend(), crashed)
        self.assertEqual(ran, [])
        tick(Snapshot(), [auto_merge], FakeBackend(), [])
        self.assertEqual(ran, ["merge"])

    def test_crash_propagates_without_a_crashed_list(self):
        with self.assertRaises(ZeroDivisionError):
            tick(Snapshot(), [lambda _: [1 / 0]], FakeBackend())

    def test_fix_outcomes_match_markers_to_runs(self):
        from datetime import datetime, timezone
        from factory_fix import marker
        from factory_tick import fix_outcomes
        now = datetime(2026, 10, 23, 20, 0, tzinfo=timezone.utc)

        def pr(t):
            return {"number": 9, "comments": [{"user": "github-actions[bot]",
                                               "body": marker(9, "a" * 40, "review"), "created_at": t}]}

        def run(t, status, conclusion=None, n=9):
            return {"display_title": f"Claude fix #{n}", "created_at": t, "status": status,
                    "conclusion": conclusion, "html_url": "u"}

        p = pr("2026-10-23T19:00:00Z")
        fix_outcomes(p, [run("2026-10-23T18:59:30Z", "completed", "success")], now)
        self.assertEqual((p["fix_finished"], p["fix_lost"]), (["a" * 40], []))
        p = pr("2026-10-23T19:00:00Z")
        fix_outcomes(p, [run("2026-10-23T19:00:05Z", "in_progress")], now)
        self.assertEqual((p["fix_finished"], p["fix_lost"]), ([], []))
        for runs in ([run("2026-10-23T19:00:05Z", "completed", "timed_out")],
                     [run("2026-10-23T19:00:05Z", "completed", "success", n=8)],  # another PR's
                     [run("2026-10-23T18:40:00Z", "completed", "success")]):  # before the marker
            p = pr("2026-10-23T19:00:00Z")
            fix_outcomes(p, runs, now)
            self.assertEqual((p["fix_finished"], p["fix_lost"]), ([], ["2026-10-23T19:00:00Z"]))
        p = pr("2026-10-23T19:55:00Z")  # no run yet, but only five minutes old
        fix_outcomes(p, [], now)
        self.assertEqual(p["fix_runs"][0][1], "starting")


if __name__ == "__main__":
    unittest.main()
