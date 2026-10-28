"""Tests for the stall pass. python3 -I test_factory_stalls.py (no network)."""
import copy
import os
import socket
import sys
import unittest
from datetime import datetime, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_stalls import stalls  # noqa: E402
from factory_tick import DESIGN_DIR, FakeBackend, Snapshot, load_docs, tick  # noqa: E402

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

    def test_closed_plan_without_tickets(self):
        # A plan that found nothing to build waits for the docs updater too.
        r = [x for x in reasons(load()) if "#287" in x]
        self.assertEqual(len(r), 1)
        self.assertIn('no "Phase 7 still incomplete (plan #287)" note', r[0])

    def test_still_incomplete_note_replans_once(self):
        s = load()
        doc = next(d for d in s.docs if d["file"] == "docs/design/basic-gameplay.md")
        doc["text"] = "**Phase 7 still incomplete (plan #287).** The cue is missing."
        acts = [a for a in stalls(s) if "#287" in a.reason]
        self.assertEqual([a.kind for a in acts], ["request_replan", "report"])
        self.assertEqual(acts[0].payload, {"file": "docs/design/basic-gameplay.md"})
        self.assertIn("dispatched the planner", acts[1].reason)
        # Reported: not dispatched again on the next tick, even if no plan appeared.
        tick(s, [stalls], FakeBackend())
        self.assertFalse(any("#287" in a.reason for a in stalls(s)))
        # The note must name this plan: an older plan's note doesn't count.
        t = load()
        next(d for d in t.docs if d["file"] == doc["file"])["text"] = \
            "Phase 7 still incomplete (plan #28)."
        self.assertNotIn("request_replan", [a.kind for a in stalls(t)])

    def test_just_closed_plan_waits_for_docs_updater(self):
        s = load()
        plan = next(p for p in s.plans if p["number"] == 287)
        plan["closed_at"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        self.assertFalse(any("#287" in r for r in reasons(s)))
        plan["closed_at"] = "2026-01-01T00:00:00Z"
        self.assertTrue(any("#287" in r for r in reasons(s)))

    def test_open_docs_pr_skips_doc(self):
        s = load()
        s.prs.append({"number": 3, "state": "open", "head": "docs/basic-gameplay-phase-done-287",
                      "files": []})
        self.assertFalse(any("basic-gameplay.md" in r for r in reasons(s)))

    def test_load_docs_reads_the_repo(self):
        docs = {d["file"]: d for d in load_docs(DESIGN_DIR)}
        self.assertNotIn("docs/design/README.md", docs)
        d = docs["docs/design/factory-orchestrator.md"]
        self.assertEqual(d["status"], "accepted")
        self.assertIn("A", d["phases"])
        self.assertIn("## Reconciliation notes", d["text"])

    def test_load_docs_refuses_empty(self):
        with self.assertRaises(RuntimeError):
            load_docs(os.path.join(DESIGN_DIR, "no-such-dir"))

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
        self.assertEqual(len(tick(s, [stalls], FakeBackend())), 5)
        self.assertEqual(tick(s, [stalls], FakeBackend()), [])


if __name__ == "__main__":
    unittest.main()
