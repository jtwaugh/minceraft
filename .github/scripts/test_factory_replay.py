"""Replay the 2026-10-19/20 stall history through one full tick. python3 -I test_factory_replay.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_backend import GhBackend  # noqa: E402
from factory_tick import Action, FakeBackend, Snapshot, passes, tick  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "history.json")


def keys(actions):
    return {(a.kind, a.target) for a in actions}


class Replay(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snap = Snapshot.from_fixture(FIXTURE)
        cls.first = tick(cls.snap, passes(), FakeBackend())
        cls.second = tick(cls.snap, passes(), FakeBackend())

    def reasons(self):
        return "\n".join(a.reason for a in self.first if a.kind == "report")

    def test_each_case_is_reported(self):
        text = self.reasons()
        for expected in ["orphan-ticket:issue#452", "orphan-ticket:issue#500", "orphan-ticket:issue#521",
                         "plan-as-dependency:issue#519",
                         "accepted-doc-without-phases:docs/design/raft-scene-feedback.md"]:
            self.assertIn(expected, text)

    def test_stalled_phases_are_reported(self):
        stalls = [a for a in self.first if a.kind == "report" and a.payload.get("kind") == "stalled"]
        self.assertTrue(any(a.target.endswith("raft-scene-feedback.md") for a in stalls))

    def test_one_report_issue_and_ready_ticket(self):
        self.assertIn(("create_issue", "new:stalled-work"), keys(self.first))
        self.assertIn(("start_ticket", "issue#521"), keys(self.first))
        self.assertNotIn(("start_ticket", "issue#519"), keys(self.first))  # depends on its plan

    def test_split_off_half_is_adopted(self):
        self.assertIn(("set_parent", "issue#521"), keys(self.first))
        self.assertEqual(next(i for i in self.snap.issues if i["number"] == 521)["parent"], 169)

    def test_second_tick_does_nothing(self):
        self.assertEqual(self.second, [])


class Live(unittest.TestCase):
    """GhBackend with an injected `run` that records requests instead of calling gh."""

    def backend(self):
        calls = []

        def run(args, project=False):
            calls.append(args)
            return "123\n"
        return GhBackend("o/r", run), calls

    def test_untrusted_actor_is_refused_without_a_request(self):
        b, calls = self.backend()
        snap = Snapshot.from_fixture(FIXTURE)
        b.apply(snap, Action("add_label", "issue#521", {"label": "nope", "actor": "mallory"}))
        self.assertEqual((calls, len(b.refused)), ([], 1))
        self.assertNotIn("nope", next(i for i in snap.issues if i["number"] == 521)["labels"])
        b.apply(snap, Action("add_label", "issue#521", {"label": "nope", "actor": "jtwaugh"}))
        self.assertEqual(len(calls), 1)

    def test_satisfied_action_makes_no_request(self):
        b, calls = self.backend()
        snap = Snapshot.from_fixture(FIXTURE)
        snap.issues[0]["labels"] = ["planned"]
        b.apply(snap, Action("add_label", f"issue#{snap.issues[0]['number']}", {"label": "planned"}))
        self.assertEqual(calls, [])

    def test_second_live_tick_writes_nothing(self):
        snap = Snapshot.from_fixture(FIXTURE)
        b, calls = self.backend()
        self.assertTrue(tick(snap, passes(), b))
        self.assertTrue(calls)
        again, calls2 = self.backend()
        self.assertEqual(tick(snap, passes(), again), [])
        self.assertEqual(calls2, [])


META = {"loaded": True, "project": "P", "status_field": "SF", "waiting_field": "WF",
        "options": ["Inbox", "Done"], "option_ids": {"Inbox": "o1", "Done": "o2"}}


class LiveBoard(unittest.TestCase):
    def setUp(self):
        self.calls = []

        def run(args, project=False):
            self.calls.append(args)
            if "graphql" in args:
                return '{"data": {"addProjectV2ItemById": {"item": {"id": "ITEM"}}}}'
            return "NODE\n"
        self.b = GhBackend("o/r", run)
        self.snap = Snapshot(board=[], board_meta=dict(META))

    def test_card_is_added_then_set(self):
        self.b.apply(self.snap, Action("add_card", "issue#5", {"status": "Inbox", "waiting": "#4"}))
        self.assertEqual(len(self.calls), 4)  # node id, add, status, waiting text
        self.assertEqual(self.snap.board[0]["item"], "ITEM")
        self.b.apply(self.snap, Action("set_status", "issue#5", {"status": "Done"}))
        self.assertEqual(len(self.calls), 5)
        self.assertEqual(self.snap.board[0]["status"], "Done")

    def test_satisfied_card_actions_make_no_request(self):
        self.snap.board = [{"item": "I", "kind": "Issue", "number": 5, "state": "OPEN",
                            "status": "Inbox", "waiting": None}]
        for a in [Action("add_card", "issue#5", {"status": "Inbox", "waiting": ""}),
                  Action("set_status", "issue#5", {"status": "Inbox"}),
                  Action("set_waiting", "issue#5", {"text": ""})]:
            self.b.apply(self.snap, a)
        self.assertEqual(self.calls, [])

    def test_only_trusted_comments_are_read(self):
        import json
        from unittest import mock

        def fake(cmd, **kw):
            path = cmd[cmd.index("GET") + 1].split("?")[0]
            data = {"repos/o/r/issues": [{"number": 1, "title": "t", "state": "open", "body": "",
                                          "labels": [{"name": "factory"}]}],
                    "repos/o/r/issues/1/comments": [
                        {"user": {"login": "jtwaugh"}, "body": "ok", "created_at": "1"},
                        {"user": {"login": "mallory"}, "body": "Blocked by #9", "created_at": "2"}]
                    }.get(path, [])
            return mock.Mock(stdout="HTTP/2 200\n\n" + json.dumps(data), stderr="", returncode=0)
        with mock.patch("factory_tick.subprocess.run", fake):
            snap = Snapshot.from_github("o/r")
        self.assertEqual([c["user"] for c in snap.issues[0]["comments"]], ["jtwaugh"])

    def test_loader_reads_parent_deps_and_kind(self):
        import json
        from unittest import mock
        issues = [
            {"number": 1, "title": "Plan: x", "state": "open", "labels": [],
             "body": "Plan.\n<!-- plan-source file=docs/design/d.md section=Phase A (One) -->"},
            {"number": 2, "title": "t", "state": "open", "body": "**Depends on:** #3, #2",
             "labels": [{"name": "planned"}],
             "parent_issue_url": "https://api.github.com/repos/o/r/issues/1"},
            {"number": 3, "title": "q", "state": "open", "body": "",
             "labels": [{"name": "spec-question"}, {"name": "decision"}]},
            {"number": 4, "title": "Idea: y", "state": "open", "body": "", "labels": [{"name": "idea"}]}]

        def fake(cmd, **kw):
            path = cmd[cmd.index("GET") + 1].split("?")[0]
            # what subprocess.run(text=True) hands back: \r\n already turned into \n
            body = json.dumps(issues if path == "repos/o/r/issues" else [])
            return mock.Mock(stdout="HTTP/2.0 200 OK\nEtag: W/\"1\"\n\n" + body, stderr="", returncode=0)
        with mock.patch("factory_tick.subprocess.run", fake):
            snap = Snapshot.from_github("o/r")
        by = {i["number"]: i for i in snap.issues}
        self.assertEqual([by[n]["kind"] for n in (1, 2, 3, 4)], ["plan", "ticket", "decision", "idea"])
        self.assertEqual((by[2]["parent"], by[3]["parent"]), (1, None))
        self.assertEqual(by[2]["depends_on"], [3])
        self.assertEqual(by[1]["plan_source"], {"file": "docs/design/d.md", "section": "Phase A (One)"})

    def test_repo_calls_use_the_write_token(self):
        import factory_backend
        from unittest import mock
        seen = []

        def fake(cmd, **kw):
            seen.append(kw["env"]["GH_TOKEN"])
            return mock.Mock(stdout="", stderr="", returncode=0)
        env = {"GH_TOKEN": "read", "WRITE_TOKEN": "write", "PROJECT_TOKEN": "project"}
        with mock.patch.dict(os.environ, env), mock.patch("factory_backend.subprocess.run", fake):
            factory_backend.gh_run(["issue", "edit", "1"])
            factory_backend.gh_run(["api", "graphql"], project=True)
        self.assertEqual(seen, ["write", "project"])

    def test_loader_fills_new_fields(self):
        import factory_board
        import project_sync
        node = {"id": "I1", "content": {"__typename": "Issue", "number": 5, "state": "OPEN",
                                        "repository": {"nameWithOwner": project_sync.REPO}},
                "status": {"name": "Inbox", "updatedAt": "1"}, "waiting": None}
        orig = project_sync.load_project
        project_sync.load_project = lambda *_: ("P", "SF", {"Inbox": "o1", "Done": "o2"}, [node], "WF")
        try:
            snap = Snapshot()
            factory_board.load_board(snap, "me", 7)
        finally:
            project_sync.load_project = orig
        self.assertEqual(snap.board[0]["item"], "I1")
        for k, v in {"loaded": True, "number": 7, "project": "P", "status_field": "SF",
                     "waiting_field": "WF"}.items():
            self.assertEqual(snap.board_meta[k], v)
        self.assertEqual(snap.board_meta["option_ids"]["Done"], "o2")


if __name__ == "__main__":
    unittest.main()
