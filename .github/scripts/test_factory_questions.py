"""Tests for the stop-marker question pass. python3 -I test_factory_questions.py (no network)."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("GITHUB_REPOSITORY", "o/r")
from factory_blocked import blocked  # noqa: E402
import json  # noqa: E402
from factory_fix import result_marker, results, stop_marker, stops  # noqa: E402
from factory_nits import nits  # noqa: E402
from factory_questions import link_questions, questions  # noqa: E402
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


def result_snap(kind, sha=SHA, options=("X", "Y"), result=None):
    nm = {"kind": kind, "question": "Spec 3.2 says X and Y.", "options": list(options)}
    body = json.dumps(result or {"needs_maintainer": nm})
    s = snap(kind, "", sha=SHA)
    s.prs[0]["comments"] = [{"user": "claude[bot]", "created_at": "2026-01-01T00:00:00Z",
                             "body": f"{result_marker(9, sha)}\n```json\n{body}\n```"}]
    return s


class TestResult(unittest.TestCase):
    def test_each_kind_matches_the_marker_path(self):
        for kind, labels in (("decision", ["spec-question", "decision"]),
                             ("research", ["spec-question", "research"]),
                             ("needs-you", ["needs-you"])):
            s = result_snap(kind)
            old = snap(kind, "Spec 3.2 says X and Y.\n\nA: X\nB: Y")
            run(old)
            self.assertEqual([x.kind for x in run(s)], ["open_question"])
            self.assertEqual(s.issues[-1], old.issues[-1])
            self.assertEqual(s.issues[-1]["labels"], labels)
            self.assertEqual(run(s), [])  # second pass: no second issue
            self.assertEqual(len(s.issues), 2)

    def test_stale_head_ignored(self):
        a = run(result_snap("needs-you", sha="b" * 40))
        self.assertEqual([x.kind for x in a], ["start_fix"])

    def test_invalid_or_pushed_result_asks_nothing(self):
        for r in ({"needs_maintainer": {"kind": "decision", "question": "q", "options": []}},
                  {"pushed": True, "commit": "abc"}):
            s = result_snap("needs-you", result=r)
            self.assertEqual(results(s.prs[0]["comments"]) != [], "pushed" in r)
            self.assertEqual([x.kind for x in run(s)], ["start_fix"])


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

    def test_link_once_and_closing_resumes(self):
        s = snap("needs-you", "?")
        s.issues[0]["blocked_by_links"] = []
        passes = [questions, link_questions, blocked]
        a = tick(s, passes, FakeBackend())  # no fix beside the question
        self.assertEqual([(x.kind, x.target, x.payload) for x in a[1:]],
                         [("add_blocked_by", "issue#7", {"blocker": 8})])
        self.assertEqual(a[0].kind, "open_question")
        self.assertEqual(s.issues[0]["blocked_by_links"], [8])
        self.assertEqual(tick(s, passes, FakeBackend()), [])  # linked once
        s.issues[1]["state"] = "closed"
        self.assertEqual([x.kind for x in tick(s, passes, FakeBackend())], ["start_fix"])
        self.assertEqual(len(s.issues), 2)

    def test_open_question_stops_nits(self):
        s = snap("needs-you", "?", verdict={"kind": "nits", "commit": SHA[:7], "nits": ["x"]},
                 mergeable_state="clean")
        s.issues.append({"number": 8, "title": "q", "state": "open", "labels": ["needs-you"],
                         "body": "<!-- factory-question pr=9 cause=nits -->"})
        self.assertEqual(nits(s), [])
        s.issues[1]["state"] = "closed"
        self.assertEqual([x.kind for x in nits(s)], ["start_fix"])

    def test_parse(self):
        c = [{"body": stop_marker(9, SHA, "research") + "\nq"}]
        self.assertEqual(stops(c), [(9, SHA, "research", "q")])


if __name__ == "__main__":
    unittest.main()
