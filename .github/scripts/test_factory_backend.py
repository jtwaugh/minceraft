"""Tests for GhBackend's start_fix and open_question. python3 -I test_factory_backend.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import FIX_WORKFLOW, PLAN_WORKFLOW, GhBackend  # noqa: E402
from factory_fix import marker  # noqa: E402
from factory_tick import Action, Snapshot  # noqa: E402

KEY = "<!-- factory-question pr=9 cause=retry-check -->"


def snap(issues=(), comments=()):
    s = Snapshot()
    s.issues = [dict(i) for i in issues]
    s.prs = [{"number": 9, "sha": "abc1234", "comments": list(comments)}]
    return s


def backend():
    calls = []

    def run(args, project=False):
        calls.append(args)
        return ""
    return GhBackend("o/r", run), calls


FIX = Action("start_fix", "pr#9", {"kind": "nits", "head": "abc1234", "task": "do it"}, "")


def question(body="B", **kw):
    return Action("open_question", "issue#7", {
        "label": "needs-you", "key": KEY, "title": "T", "body": f"{KEY}\n{body}", **kw}, "")


class Test(unittest.TestCase):
    def test_start_fix_comments_and_dispatches_once(self):
        b, calls = backend()
        s = snap()
        b.apply(s, FIX)
        self.assertEqual(calls, [  # dispatch first: a marker without a run reads as "running"
            ["workflow", "run", FIX_WORKFLOW, "--repo", "o/r", "-f", "pr=9", "-f", "task=do it"],
            ["pr", "comment", "9", "--repo", "o/r", "--body", marker(9, "abc1234", "nits")]])
        b.apply(s, FIX)
        self.assertEqual(len(calls), 2)

    def test_start_fix_skipped_when_marker_present(self):
        b, calls = backend()
        b.apply(snap(comments=[{"body": marker(9, "abc1234", "nits")}]), FIX)
        self.assertEqual(calls, [])

    def test_start_fix_for_a_new_head_runs_again(self):
        b, calls = backend()
        b.apply(snap(comments=[{"body": marker(9, "old0000", "nits")}]), FIX)
        self.assertEqual(len(calls), 2)

    def test_request_replan_dispatches_planner_once(self):
        b, calls = backend()
        s = snap()
        doc = "docs/design/x.md"
        b.apply(s, Action("request_replan", doc, {"file": doc}, "m"))
        self.assertEqual(calls, [["workflow", "run", PLAN_WORKFLOW, "--repo", "o/r", "-f", f"file={doc}"]])
        s.reported.add((doc, "m"))  # the report beside it
        b.apply(s, Action("request_replan", doc, {"file": doc}, "m"))
        self.assertEqual(len(calls), 1)

    def test_open_question_creates_then_updates(self):
        b, calls = backend()
        s = snap()
        b.apply(s, question())
        self.assertEqual(calls, [["issue", "create", "--repo", "o/r", "--title", "T",
                                  "--body", f"{KEY}\nB", "--label", "needs-you"]])
        b.apply(s, question())
        self.assertEqual(len(calls), 1)
        b.apply(s, question("B2"))
        self.assertEqual(calls[1][:3], ["issue", "edit", str(s.issues[0]["number"])])
        self.assertEqual(len(s.issues), 1)
        b.apply(s, question("B2"))
        self.assertEqual(len(calls), 2)

    def test_open_question_uses_given_labels(self):
        b, calls = backend()
        b.apply(snap(), question(labels=["spec-question", "decision"]))
        self.assertEqual(calls[0][-4:], ["--label", "spec-question", "--label", "decision"])

    def test_closed_question_is_not_reused(self):
        b, calls = backend()
        old = {"number": 3, "title": "T", "state": "closed", "labels": [], "body": f"{KEY}\nB"}
        b.apply(snap([old]), question())
        self.assertEqual(calls[0][:2], ["issue", "create"])

    def test_untrusted_actor_is_refused(self):
        b, calls = backend()
        a = question()
        a.payload["actor"] = "stranger"
        b.apply(snap(), a)
        self.assertEqual((calls, len(b.refused)), ([], 1))


if __name__ == "__main__":
    unittest.main()
