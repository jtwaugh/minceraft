"""Tests for the superseded-PR pass. python3 -I test_factory_supersede.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import GhBackend  # noqa: E402
from factory_supersede import superseded  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402


def issue(n, state="closed", kind="ticket"):
    return {"number": n, "title": f"T{n}", "state": state, "labels": ["factory"], "kind": kind, "body": ""}


def pr(n=903, head="claude/issue-900-sea-ice", body="Closes #900", **kw):
    return {"number": n, "head": head, "sha": "ec8aeb7", "body": body, "labels": [], "draft": False, **kw}


def snap(*prs, issues=(issue(900),)):
    return Snapshot(issues=list(issues), prs=list(prs))


class Pass(unittest.TestCase):
    def test_pr_903(self):
        """A local session's PR for #900, left open after the factory's PR closed #900."""
        s = snap(pr())
        a = tick(s, [superseded], FakeBackend())
        self.assertEqual([(x.kind, x.target, x.payload["tickets"]) for x in a],
                         [("supersede_pr", "pr#903", [900])])
        self.assertIn("#900 is already closed", a[0].payload["comment"])
        self.assertEqual((s.prs[0]["draft"], s.prs[0]["labels"]), (True, ["superseded"]))
        self.assertEqual(tick(s, [superseded], FakeBackend()), [])

    def test_marked_ready_again_is_left_alone(self):
        self.assertEqual(superseded(snap(pr(labels=["superseded"]))), [])

    def test_left_alone(self):
        cases = {
            "ticket still open": snap(pr(), issues=[issue(900, state="open")]),
            "already a draft": snap(pr(draft=True)),
            "not a claude/ branch": snap(pr(head="jtwaugh/fix")),
            "no ticket named": snap(pr(head="claude/tidy", body="")),
            "unknown ticket": snap(pr(), issues=[]),
            "a decision, not a ticket": snap(pr(), issues=[issue(900, kind="decision")]),
            "one of two tickets open": snap(pr(head="claude/both", body="Closes #900, closes #901"),
                                            issues=[issue(900), issue(901, state="open")]),
        }
        for why, s in cases.items():
            with self.subTest(why):
                self.assertEqual(superseded(s), [])

    def test_branch_number_wins_over_body(self):
        s = snap(pr(body="Closes #901"), issues=[issue(900), issue(901, state="open")])
        self.assertEqual([a.target for a in superseded(s)], ["pr#903"])


class Backend(unittest.TestCase):
    def test_comments_labels_and_drafts_once(self):
        calls = []

        def run(args, project=False):
            calls.append(args[:3])
            return ""
        s = snap(pr())
        tick(s, [superseded], GhBackend("o/r", run))
        self.assertEqual(calls, [["pr", "comment", "903"],
                                 ["api", "-X", "POST"],
                                 ["pr", "ready", "903"]])
        tick(s, [superseded], GhBackend("o/r", run))
        self.assertEqual(len(calls), 3)

    def test_a_refused_draft_still_parks_it(self):
        def run(args, project=False):
            if args[:2] == ["pr", "ready"]:
                raise RuntimeError("gh pr ready: not permitted")
            return ""
        s = snap(pr())
        b = GhBackend("o/r", run)
        tick(s, [superseded], b)
        self.assertEqual([a.target for a, _ in b.failed], ["pr#903"])
        self.assertEqual(superseded(s), [])


if __name__ == "__main__":
    unittest.main()
