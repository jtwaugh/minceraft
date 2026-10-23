"""Tests for the Blocked-fix pass. python3 -I test_factory_blocked.py (no network)."""
import os
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_blocked import RETRY_BUDGET, attempts, blocked, parse_blockers  # noqa: E402
from factory_fix import markers  # noqa: E402
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

    def qs(self, s):
        return [i for i in s.issues if "needs-you" in i["labels"]]

    def test_budget_spent_asks_once_no_extra_fix(self):
        s = snap(mergeable_state="dirty", blockers=[])
        heads = [c * 40 for c in "abcdef"[:RETRY_BUDGET]]
        for sha in heads:  # every attempt ends Blocked again
            s.prs[0]["sha"] = sha
            self.assertEqual([x.kind for x in run(s)], ["start_fix"])
        s.prs[0]["sha"] = "9" * 40
        for _ in range(3):
            run(s)
        self.assertEqual(len(self.qs(s)), 1)
        self.assertEqual([m[2] for m in markers(s.prs[0]["comments"])], ["conflict"] * RETRY_BUDGET)
        body = self.qs(s)[0]["body"]
        for w in ("#7", "PR #9", "conflicts", "A:", "pushed a new head"):
            self.assertIn(w, body)

    def test_lost_run_is_retried_and_counts(self):
        s = snap(mergeable_state="dirty", blockers=[])
        self.assertEqual([x.kind for x in run(s)], ["start_fix"])
        self.assertEqual(run(s), [])  # running: wait
        s.prs[0]["fix_lost"] = [c["created_at"] for c in s.prs[0]["comments"]]
        self.assertEqual([x.kind for x in run(s)], ["start_fix"])  # lost: try again
        self.assertEqual(attempts(s.prs[0])[0][2], "run failed")

    def test_budget_comes_from_config(self):
        with mock.patch("factory_blocked.RETRY_BUDGET", 1):
            s = snap(mergeable_state="dirty")
            run(s)
            s.prs[0]["sha"] = "b" * 40
            run(s)
        self.assertEqual(len(self.qs(s)), 1)

    def test_push_nothing_asks_early(self):
        s = snap(blockers=["a.py:3 breaks it"])
        self.assertEqual([x.kind for x in run(s)], ["start_fix"])
        self.assertEqual(run(s), [])  # still running
        s.prs[0]["fix_finished"] = [SHA]
        run(s)
        self.assertEqual(len(self.qs(s)), 1)
        self.assertIn("> a.py:3 breaks it", self.qs(s)[0]["body"])
        self.assertIn("pushed nothing", self.qs(s)[0]["body"])
        self.assertEqual(run(s), [])

    def test_fix_that_passes_asks_nothing(self):
        s = snap(mergeable_state="dirty")
        run(s)
        s.prs[0].update(sha="b" * 40, mergeable_state="clean")
        self.assertEqual(run(s), [])
        self.assertEqual(self.qs(s), [])
        self.assertEqual(attempts(s.prs[0])[0][2], "pushed a new head")

    def test_question_updated_when_blocker_changes(self):
        s = snap(blockers=["one"])
        run(s)
        s.prs[0]["fix_finished"] = [SHA]
        run(s)
        s.prs[0]["blockers"] = ["two"]
        self.assertEqual([x.kind for x in run(s)], ["update_issue"])
        self.assertIn("> two", self.qs(s)[0]["body"])
        self.assertEqual(len(self.qs(s)), 1)

    def test_in_pass_list(self):
        self.assertIn(blocked, passes())

    def test_parse_blockers(self):
        body = ("Verdict: Blocked\n\n**Blockers**\n1. `a.py:3` wrong,\n   really.\n"
                "2. `b.py:9` no test\n\n**Nits**\n1. rename\n")
        self.assertEqual(parse_blockers(body), ["`a.py:3` wrong, really.", "`b.py:9` no test"])


if __name__ == "__main__":
    unittest.main()
