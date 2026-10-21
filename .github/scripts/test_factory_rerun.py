"""Tests for the rerun pass. python3 -I test_factory_rerun.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import GhBackend  # noqa: E402
from factory_rerun import rerun_failed  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402


def run(i, name, conclusion="failure", attempt=1):
    return {"id": i, "name": name, "conclusion": conclusion, "run_attempt": attempt}


def snap(runs, head="claude/issue-7-20260101-0000", sha="abcdef1234"):
    return Snapshot(
        issues=[{"number": 7, "title": "t", "state": "open", "labels": ["factory"], "body": ""}],
        prs=[{"number": 9, "head": head, "sha": sha, "body": "", "runs": runs}])


class Test(unittest.TestCase):
    def test_rerun_once(self):
        s = snap([run(1, "CI"), run(2, "PR size", "success")])
        a = tick(s, [rerun_failed], FakeBackend())
        self.assertEqual([(x.kind, x.target, x.payload) for x in a],
                         [("rerun_jobs", "pr#9", {"run_id": 1})])
        self.assertIn("CI", a[0].reason)
        self.assertEqual(tick(s, [rerun_failed], FakeBackend()), [])

    def test_failed_again_reports_once(self):
        s = snap([run(1, "CI", attempt=2)])
        a = tick(s, [rerun_failed], FakeBackend())
        self.assertEqual([x.kind for x in a], ["report"])
        self.assertEqual(a[0].reason, "CI failed again after a re-run on abcdef1")
        self.assertEqual(tick(s, [rerun_failed], FakeBackend()), [])

    def test_ignored(self):
        self.assertEqual(rerun_failed(snap([run(1, "Claude reviewer")])), [])
        self.assertEqual(rerun_failed(snap([run(1, "CI")], head="spec/issue-7-x")), [])
        self.assertEqual(rerun_failed(snap([run(1, "CI", "success"), run(2, "CI", None)])), [])

    def test_gh_backend_posts(self):
        calls = []
        s = snap([run(1, "Review gate", "startup_failure")])
        tick(s, [rerun_failed], GhBackend("o/r", run=lambda args, project=False: calls.append(args) or ""))
        self.assertEqual(calls, [["api", "-X", "POST", "repos/o/r/actions/runs/1/rerun-failed-jobs"]])
        self.assertEqual(rerun_failed(s), [])


if __name__ == "__main__":
    unittest.main()
