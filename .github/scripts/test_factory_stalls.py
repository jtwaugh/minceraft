"""Tests for the stall pass. python3 -I test_factory_stalls.py (no network)."""
import copy
import os
import socket
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_stalls import stalls  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "history.json")


def load():
    return Snapshot.from_fixture(FIXTURE)


def reasons(snap):
    return [a.reason for a in stalls(snap)]


class Test(unittest.TestCase):
    def setUp(self):
        def refuse(*a, **k):
            raise AssertionError("network used")
        self._connect, socket.socket.connect = socket.socket.connect, refuse

    def tearDown(self):
        socket.socket.connect = self._connect

    def test_fixture_doc_without_phases(self):
        r = [x for x in reasons(load()) if "no phases" in x]
        self.assertEqual(len(r), 1)
        self.assertIn("raft-scene-feedback.md` has no phases", r[0])

    def test_phase_without_plan(self):
        s = load()
        s.plans = [p for p in s.plans if p["number"] != 169]
        self.assertTrue(any("world-layout.md` Phase D has no plan" in r for r in reasons(s)))

    def test_closed_plan_all_subs_closed(self):
        s = load()
        for i in s.issues:
            if i["number"] == 452:
                i["parent"] = 287
        r = reasons(s)
        self.assertTrue(any("plan #287 is closed but the phase is still `todo`" in x for x in r))
        # a not_planned plan, or one with an open sub-issue, is not stalled
        t = copy.deepcopy(s)
        t.plans[0]["state_reason"] = "not_planned"
        self.assertFalse(any("#287" in x for x in reasons(t)))
        for i in s.issues:
            if i["number"] == 452:
                i["state"] = "open"
        self.assertFalse(any("#287" in x for x in reasons(s)))

    def test_leftover_branches(self):
        s = load()
        s.branches += ["docs/basic-gameplay-sync"]
        r = reasons(s)
        self.assertTrue(any("`docs/world-layout-phase-done-169` is left over" in x for x in r))
        self.assertTrue(any("`docs/ui-art-and-inventory-phase-done-513` is left over" in x for x in r))
        self.assertTrue(any("`docs/basic-gameplay-sync` is left over" in x for x in r))
        self.assertFalse(any("phase-done-287" in x for x in r))  # plan closed
        # a branch an open PR is using is not leftover
        s.prs.append({"number": 1, "state": "open", "head": "docs/basic-gameplay-sync", "files": []})
        self.assertFalse(any("-sync" in x for x in reasons(s)))

    def test_doc_with_open_pr_skipped(self):
        s = load()
        s.prs.append({"number": 2, "state": "open", "head": "x",
                      "files": ["docs/design/raft-scene-feedback.md"]})
        self.assertFalse(any("raft-scene" in r for r in reasons(s)))

    def test_merged_pr_does_not_skip(self):
        s = load()
        s.prs.append({"number": 2, "state": "merged", "head": "x",
                      "files": ["docs/design/raft-scene-feedback.md"]})
        self.assertTrue(any("raft-scene" in r for r in reasons(s)))

    def test_not_accepted_doc_skipped(self):
        s = load()
        s.docs[-1]["status"] = "idea"
        self.assertFalse(any("no phases" in r for r in reasons(s)))

    def test_idempotent(self):
        s = load()
        s.branches.append("docs/x-sync")
        self.assertEqual(len(tick(s, [stalls], FakeBackend())), 4)
        self.assertEqual(tick(s, [stalls], FakeBackend()), [])


if __name__ == "__main__":
    unittest.main()
