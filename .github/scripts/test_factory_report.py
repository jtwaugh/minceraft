"""Tests for the report pass. python3 -I test_factory_report.py (no network)."""
import os
import random
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_report import body, report  # noqa: E402
from factory_tick import REPORT_TITLE, FakeBackend, Snapshot, tick  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "history.json")


def load():
    return Snapshot.from_fixture(FIXTURE)


class Test(unittest.TestCase):
    def test_create_then_noop(self):
        s = load()
        a = tick(s, [report], FakeBackend())
        self.assertEqual([x.kind for x in a], ["create_issue"])
        self.assertEqual(tick(s, [report], FakeBackend()), [])

    def test_update_when_changed(self):
        s = load()
        tick(s, [report], FakeBackend())
        s.branches.append("docs/some-sync")
        a = tick(s, [report], FakeBackend())
        self.assertEqual([x.kind for x in a], ["update_issue"])
        self.assertIn("docs/some-sync", a[0].payload["body"])
        self.assertEqual(tick(s, [report], FakeBackend()), [])

    def test_close_when_clean(self):
        s = Snapshot()
        self.assertEqual(report(s), [])
        s.issues.append({"number": 9, "title": REPORT_TITLE, "state": "open",
                         "labels": [], "kind": "report", "body": "old"})
        a = tick(s, [report], FakeBackend())
        self.assertEqual([x.kind for x in a], ["close_issue"])
        self.assertEqual(tick(s, [report], FakeBackend()), [])

    def test_body_is_order_independent(self):
        s = load()
        s.branches += ["docs/a-sync", "docs/b-sync"]
        want = body(s)
        self.assertIn("## ", want)
        self.assertIn("`orphan-ticket:issue#521`", want)
        rng = random.Random(1)
        for _ in range(5):
            for f in (s.issues, s.plans, s.docs, s.branches, s.prs):
                rng.shuffle(f)
            self.assertEqual(body(s), want)


if __name__ == "__main__":
    unittest.main()
