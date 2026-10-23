"""Tests for the Blocked-fix pass. python3 -I test_factory_blocked.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_blocked import blocked, parse_blockers  # noqa: E402
from factory_rerun import rerun_failed  # noqa: E402
from factory_tick import FakeBackend, Snapshot, passes, tick  # noqa: E402

SHA = "a" * 40


def snap(head="claude/issue-7-20260101-0000", **pr):
    base = {"number": 9, "head": head, "sha": SHA, "body": "", "runs": [], "comments": [],
            "mergeable_state": "clean", "head_time": "2026-01-01T00:00:00Z",
            "head_author": "claude[bot]", "claude_mentions": [], "blockers": []}
    return Snapshot(
        issues=[{"number": 7, "title": "t", "state": "open", "labels": ["factory"], "body": ""}],
        prs=[{**base, **pr}])


def run(s, passes=(blocked,)):
    return tick(s, list(passes), FakeBackend())


def failed(attempt, log="boom"):
    return [{"id": 1, "name": "CI", "conclusion": "failure", "run_attempt": attempt, "log": log}]


class Test(unittest.TestCase):
    def once(self, s, cause, *words):
        a = run(s)
        self.assertEqual([(x.kind, x.target, x.payload["kind"]) for x in a],
                         [("start_fix", "pr#9", cause)])
        for w in words:
            self.assertIn(w, a[0].payload["task"])
        self.assertEqual(run(s), [])

    def test_conflict(self):
        self.once(snap(mergeable_state="dirty"), "conflict", "origin/master")

    def test_check_failed_again(self):
        self.once(snap(runs=failed(2, "assert 1 == 2")), "check", "`CI`", "assert 1 == 2")

    def test_reviewer_block(self):
        self.once(snap(blockers=["a.py:3 breaks it", "b.py:9 no test"]), "review",
                  "1. a.py:3 breaks it", "2. b.py:9 no test")

    def test_first_failure_only_reruns(self):
        s = snap(runs=failed(1))
        run(s, [rerun_failed, blocked])
        self.assertEqual(s.prs[0]["runs"][0]["run_attempt"], 2)
        s.prs[0]["runs"][0]["conclusion"] = "failure"  # failed again
        self.once(s, "check", "`CI`")

    def test_first_failure_alone_gets_no_fix(self):
        self.assertEqual(run(snap(runs=failed(1))), [])

    def test_new_head_gets_a_new_fix(self):
        s = snap(mergeable_state="dirty")
        run(s)
        s.prs[0]["sha"] = "b" * 40
        self.assertEqual(len(run(s)), 1)

    def test_maintainer_activity_suppresses(self):
        self.assertEqual(run(snap(mergeable_state="dirty", head_author="jtwaugh")), [])
        self.assertEqual(run(snap(mergeable_state="dirty",
                                  claude_mentions=["2026-01-01T00:05:00Z"])), [])
        self.assertEqual(len(run(snap(mergeable_state="dirty",
                                      claude_mentions=["2025-12-31T00:00:00Z"]))), 1)

    def test_not_a_factory_pr(self):
        self.assertEqual(run(snap(head="feature/x", mergeable_state="dirty")), [])
        s = snap(head="feature/x", mergeable_state="dirty", body="Closes #7")
        self.assertEqual(run(s), [])

    def test_in_pass_list(self):
        self.assertIn(blocked, passes())

    def test_parse_blockers(self):
        body = ("Verdict: Blocked\n\n**Blockers**\n1. `a.py:3` wrong,\n   really.\n"
                "2. `b.py:9` no test\n\n**Nits**\n1. rename\n")
        self.assertEqual(parse_blockers(body), ["`a.py:3` wrong, really.", "`b.py:9` no test"])


if __name__ == "__main__":
    unittest.main()
