"""Tests for the stop-marker question pass. python3 -I test_factory_questions.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_blocked import blocked  # noqa: E402
from factory_fix import stop_marker, stops  # noqa: E402
from factory_questions import questions  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402

SHA = "a" * 40


def snap(kind, text, sha=SHA, **pr):
    comment = {"user": "claude[bot]", "created_at": "2026-01-01T00:00:00Z",
               "body": f"{stop_marker(9, sha, kind)}\n{text}"}
    base = {"number": 9, "head": "claude/issue-7-20260101-0000", "sha": SHA, "body": "",
            "runs": [], "comments": [comment], "mergeable_state": "dirty",
            "head_time": "2026-01-01T00:00:00Z", "head_author": "claude[bot]",
            "claude_mentions": [], "blockers": []}
    return Snapshot(
        issues=[{"number": 7, "title": "t", "state": "open", "labels": ["factory"], "body": ""}],
        prs=[{**base, **pr}])


def run(s):
    return tick(s, [questions, blocked], FakeBackend())


class Test(unittest.TestCase):
    def asked(self, kind, labels, starts, ends):
        s = snap(kind, "Spec 3.2 says X and Y.\n\nA: X\nB: Y")
        a = run(s)
        self.assertEqual([x.kind for x in a], ["open_question"])  # and no start_fix
        q = s.issues[-1]
        self.assertEqual(q["labels"], labels)
        self.assertTrue(q["body"].startswith(starts), q["body"])
        self.assertTrue(q["body"].rstrip().endswith(ends), q["body"])
        self.assertIn("ticket #7, PR #9", q["body"])
        self.assertIn("A: X", q["body"])
        self.assertEqual(run(s), [])  # one question; the open one blocks any fix
        self.assertEqual(len(s.issues), 2)

    def test_decision(self):
        self.asked("decision", ["spec-question", "decision"], "**File:** SPEC.md",
                   "the spec editor will draft the change to SPEC.md.")

    def test_research(self):
        self.asked("research", ["spec-question", "research"], "<!-- factory-question",
                   "local spec lane.")

    def test_needs_you(self):
        self.asked("needs-you", ["needs-you"], "<!-- factory-question", "once this is closed.")

    def test_decision_names_its_file(self):
        s = snap("decision", "**File:** docs/design/x.md\nWhich?\nA: 1")
        run(s)
        self.assertTrue(s.issues[-1]["body"].startswith("**File:** docs/design/x.md\n"))

    def test_other_head_ignored(self):
        a = run(snap("needs-you", "?", sha="b" * 40))  # a stale marker asks nothing
        self.assertEqual([x.kind for x in a], ["start_fix"])

    def test_closed_question_not_reopened_and_fix_resumes(self):
        s = snap("needs-you", "?")
        run(s)
        s.issues[-1]["state"] = "closed"
        self.assertEqual([x.kind for x in run(s)], ["start_fix"])

    def test_text_change_updates(self):
        s = snap("needs-you", "?")
        run(s)
        s.prs[0]["comments"][0]["body"] += " more"
        self.assertEqual([x.kind for x in run(s)], ["update_issue"])

    def test_parse(self):
        c = [{"body": stop_marker(9, SHA, "research") + "\nq"}]
        self.assertEqual(stops(c), [(9, SHA, "research", "q")])


if __name__ == "__main__":
    unittest.main()
