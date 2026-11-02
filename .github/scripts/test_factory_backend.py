"""Tests for GhBackend's start_fix and open_question. python3 -I test_factory_backend.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import FIX_WORKFLOW, PLAN_WORKFLOW, START_WORKFLOW, GhBackend  # noqa: E402
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
    def test_open_pr_uses_the_app_token_and_nothing_else_does(self):
        from unittest import mock
        seen = []

        def fake(cmd, **kw):
            seen.append((cmd[1:3], kw["env"]["GH_TOKEN"]))
            return mock.Mock(stdout="", stderr="", returncode=0)
        env = {"GH_TOKEN": "read", "WRITE_TOKEN": "write", "PROJECT_TOKEN": "project", "APP_TOKEN": "app"}
        acts = [Action("open_pr", "issue#7", {"branch": "claude/issue-7-x", "title": "T", "body": "B"}, ""),
                Action("add_label", "pr#9", {"label": "x"}, ""),
                Action("merge_pr", "pr#9", {"sha": "abc1234"}, ""),
                Action("start_fix", "pr#9", {"kind": "nits", "head": "abc1234", "task": "t"}, "")]
        for with_app in (True, False):
            seen.clear()
            e = env if with_app else {k: v for k, v in env.items() if k != "APP_TOKEN"}
            with mock.patch.dict(os.environ, e), mock.patch("factory_backend.subprocess.run", fake):
                b = GhBackend("o/r")
                s = snap()
                s.prs[0]["head"] = "other"
                for a in acts:
                    b.apply(s, a)
            self.assertEqual([t for _, t in seen],
                             ["app" if with_app else "write", "write", "project", "write", "write"])

    def test_start_fix_comments_and_dispatches_once(self):
        b, calls = backend()
        s = snap()
        b.apply(s, FIX)
        self.assertEqual(calls, [  # dispatch first: a marker without a run reads as "running"
            ["workflow", "run", FIX_WORKFLOW, "--repo", "o/r", "-f", "pr=9", "-f", "task=do it",
             "-f", 'runner="ubuntu-latest"'],
            ["pr", "comment", "9", "--repo", "o/r", "--body", marker(9, "abc1234", "nits")]])
        b.apply(s, FIX)
        self.assertEqual(len(calls), 2)

    def test_dispatches_pass_the_chosen_runner(self):
        b, calls = backend()
        s = snap(issues=[{"number": 7, "state": "open", "labels": []}])
        s.runner = '"warm"'
        b.apply(s, FIX)
        b.apply(s, Action("start_ticket", "issue#7", {}, ""))
        self.assertEqual(calls[0][-2:], ["-f", 'runner="warm"'])
        self.assertEqual(calls[2], ["workflow", "run", START_WORKFLOW, "--repo", "o/r", "-f", "issue=7",
                                    "-f", 'runner="warm"'])

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

    def test_request_replan_passes_section_and_notes(self):
        b, calls = backend()
        doc = "docs/design/x.md"
        b.apply(snap(), Action("request_replan", doc,
                               {"file": doc, "section": "Phase A", "notes": "N"}, "m"))
        self.assertEqual(calls, [["workflow", "run", PLAN_WORKFLOW, "--repo", "o/r", "-f", f"file={doc}",
                                  "-f", "section=Phase A", "-f", "notes=N"]])

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

    def adopt(self, live, post_error=None):
        """set_parent #3 -> plan #5 on a snapshot saying #3 has no parent,
        with GitHub saying its parent is `live` ("-" for none)."""
        calls = []

        def run(args, project=False):
            calls.append(args)
            if args[:2] == ["api", "repos/o/r/issues/3"]:
                return f"33 {live}\n"
            if post_error:
                raise RuntimeError(post_error)
            return ""
        b = GhBackend("o/r", run)
        s = snap(issues=[{"number": 3, "state": "open", "labels": [], "parent": None}])
        b.apply(s, Action("set_parent", "issue#3", {"parent": 5}, "orphan"))
        return b, s, calls

    def test_set_parent_posts_when_live_issue_has_none(self):
        b, s, calls = self.adopt("-")
        self.assertEqual(calls[1], ["api", "-X", "POST", "repos/o/r/issues/5/sub_issues",
                                    "-F", "sub_issue_id=33"])
        self.assertEqual((s.issues[0]["parent"], b.failed), (5, []))

    def test_set_parent_already_done_live_is_mirrored_without_a_post(self):
        # 2026-11-01: a stale snapshot re-adopted a ticket attached by hand; the 422 crashed nine ticks
        b, s, calls = self.adopt("https://api.github.com/repos/o/r/issues/5")
        self.assertEqual((len(calls), s.issues[0]["parent"], b.failed), (1, 5, []))

    def test_set_parent_under_another_parent_fails_without_a_post(self):
        b, s, calls = self.adopt("https://api.github.com/repos/o/r/issues/8")
        self.assertEqual((len(calls), s.issues[0]["parent"]), (1, None))
        self.assertEqual(b.failed[0][1], "already a sub-issue of #8")

    def test_set_parent_422_is_a_failed_write_not_a_crash(self):
        b, s, calls = self.adopt("-", post_error="gh api -X POST: ... (HTTP 422)")
        self.assertEqual((s.issues[0]["parent"], len(b.failed)), (None, 1))


if __name__ == "__main__":
    unittest.main()
