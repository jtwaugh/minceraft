"""Phase E replay: the items the maintainer found by hand on 2026-10-20 reach the
needs-you queue. python3 -I test_factory_needs_you_history.py (no network)

The base is fixtures/history.json (stale `docs/*-phase-done-N` branches, an
accepted doc with no phases, which the report lists). The other three items
are reconstructed inline, since the history keeps only merged PRs and closed
questions: a design-doc PR waiting to be merged, a planned ticket nobody
labelled `factory`, and an open decision. (docs/design/factory-orchestrator.md
section 1 "Nobody saw what was waiting", reconciliation note 8)"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_needs_you import LABEL, QUEUE_TITLE  # noqa: E402
from factory_tick import REPORT_TITLE, FakeBackend, Snapshot, passes, tick  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "history.json")
SHA = "a" * 40
ISSUE_KEYS = {"state": "open", "body": "", "body_excerpt": "", "parent": None, "depends_on": []}

UNAPPROVED = {**ISSUE_KEYS, "number": 9001, "title": "Waiting for a label", "labels": ["planned"],
              "kind": "ticket", "parent": 169}
DECISION = {**ISSUE_KEYS, "number": 9002, "title": "How should it work?",
            "labels": ["spec-question", "decision"], "kind": "decision"}
DOCS_PR = {"number": 9003, "head": "docs/ui-phase-done-513", "sha": SHA, "body": "", "labels": [],
           "draft": False, "files": ["docs/design/ui-art-and-inventory.md"]}


def build():
    snap = Snapshot.from_fixture(FIXTURE)
    snap.codeowners, snap.master_ci = ["/.github/"], "success"
    snap.issues += [dict(UNAPPROVED), dict(DECISION)]
    snap.prs.append(dict(DOCS_PR))
    return snap


class Replay(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snap = build()
        cls.first = tick(cls.snap, passes(), FakeBackend())
        cls.second = tick(cls.snap, passes(), FakeBackend())
        cls.queue = next(i for i in cls.snap.issues
                         if i["title"] == QUEUE_TITLE and i["state"] == "open")

    def issue(self, title):
        return next(i for i in self.snap.issues if i["title"] == title and i["state"] == "open")

    def assertQueued(self, ref, text, label_in):
        self.assertIn(LABEL, label_in["labels"])
        self.assertIn(f"- {ref}: {text}", self.queue["body"])

    def test_stale_branches_reach_the_queue_through_the_stalled_work_report(self):
        report = self.issue(REPORT_TITLE)
        self.assertIn("Branch `docs/world-layout-phase-done-169` is left over", report["body"])
        self.assertQueued(f"#{report['number']}", "The tick found stalled work", report)

    def test_merge_waiting_on_the_maintainer(self):
        pr = next(p for p in self.snap.prs if p["number"] == 9003)
        self.assertQueued("#9003", "Not a factory ticket: review and merge it", pr)

    def test_planned_ticket_awaiting_the_factory_label(self):
        t = next(i for i in self.snap.issues if i["number"] == 9001)
        self.assertQueued("#9001", "Not approved: label it `factory`", t)

    def test_unanswered_decision(self):
        q = next(i for i in self.snap.issues if i["number"] == 9002)
        self.assertQueued("#9002", "Question: comment `/decide <option>`", q)

    def test_approved_and_closed_items_are_not_queued(self):
        self.assertNotIn("- #519:", self.queue["body"])  # `factory`: the factory starts it
        self.assertNotIn("- #470:", self.queue["body"])  # decided and closed

    def test_second_tick_does_nothing(self):
        self.assertTrue(self.first)
        self.assertEqual(self.second, [])


if __name__ == "__main__":
    unittest.main()
