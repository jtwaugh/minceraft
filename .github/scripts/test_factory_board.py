"""Tests for the board pass. python3 -I test_factory_board.py"""
import copy
import os
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import project_board  # noqa: E402
import factory_board  # noqa: E402
from factory_board import blocked_by_pass, board_pass  # noqa: E402
from factory_tick import FakeBackend, Snapshot, tick  # noqa: E402


def issue(n, labels, state="open", **kw):
    return {"number": n, "title": f"T{n}", "state": state, "labels": labels, "kind": "ticket",
            "body_excerpt": "", **kw}


def card(n, status, **kw):
    return {"kind": "Issue", "number": n, "state": "OPEN", "status": status,
            "status_updated": "2026-01-01", "waiting": None, **kw}


def snap(issues, board, prs=(), meta=None):
    s = Snapshot.from_dict({"issues": copy.deepcopy(issues), "prs": list(prs),
                            "board": copy.deepcopy(board)})
    s.board_meta = meta or {}
    return s


def node(typename, n, state, status=None, waiting=None, repo="o/r"):
    return {"id": f"i{n}", "content": {"__typename": typename, "number": n, "state": state,
                                       "repository": {"nameWithOwner": repo}},
            "status": {"name": status, "updatedAt": "t1"} if status else {},
            "waiting": {"text": waiting} if waiting else {}}


class Test(unittest.TestCase):
    def test_loader_fills_board_with_optional_fallbacks(self):
        result = {"user": {"projectV2": {
            "id": "P", "field": {"id": "F", "options": [
                {"id": f"o{n}", "name": n} for n in project_board.STATUSES]},
            "waitingField": {"id": "W", "dataType": "TEXT"},
            "items": {"pageInfo": {"hasNextPage": False, "endCursor": None}, "nodes": [
                node("Issue", 1, "OPEN", "Inbox", "#2"), node("Issue", 9, "OPEN", "Inbox", repo="x/y"),
                {"id": "d", "content": None}]}}}}
        s = snap([issue(1, ["planned"])], [])
        with mock.patch.object(project_board, "REPO", "o/r"), \
                mock.patch.object(project_board, "graphql", return_value=result), \
                mock.patch.object(factory_board.tickets, "comment_blockers", return_value=[(5, "t")]):
            factory_board.load_board(s, "me", 1)
        self.assertEqual(s.board, [{"item": "i1", "kind": "Issue", "number": 1, "state": "OPEN", "status": "Inbox",
                                    "status_updated": "t1", "waiting": "#2"}])
        self.assertEqual(s.board_meta, {
            "loaded": True, "number": 1, "project": "P", "status_field": "F", "waiting_field": "W",
            "options": project_board.STATUSES, "waiting": True,
            "option_ids": {n: f"o{n}" for n in project_board.STATUSES}})
        self.assertEqual(s.issues[0]["comment_blockers"], [5])

    def test_missing_link_gets_one_action_then_none(self):
        s = snap([issue(1, [], body="**Depends on:** #2", blocked_by_links=[]), issue(2, []),
                  issue(3, [], body="**Depends on:** #2")], [])
        acts = tick(s, [blocked_by_pass], FakeBackend())
        self.assertEqual([(a.kind, a.target, a.payload) for a in acts],
                         [("add_blocked_by", "issue#1", {"blocker": 2})])
        self.assertEqual(tick(s, [blocked_by_pass], FakeBackend()), [])

    def test_wrong_status_one_update_and_match_none(self):
        issues = [issue(1, ["planned", "factory"]), issue(2, ["planned"])]
        s = snap(issues, [card(1, "Inbox"), card(2, "Inbox")])
        acts = board_pass(s)
        self.assertEqual([(a.kind, a.target, a.payload) for a in acts],
                         [("set_status", "issue#1", {"status": "Approved"})])

    def test_dragged_to_idea_becomes_label_action(self):
        s = snap([issue(3, ["planned", "factory"])], [card(3, "Idea")])
        acts = board_pass(s)
        self.assertEqual([(a.kind, a.payload["label"]) for a in acts],
                         [("add_label", "idea"), ("remove_label", "factory")])
        tick(s, [board_pass], FakeBackend())
        self.assertEqual(board_pass(s), [])

    def test_label_removed_after_drag_moves_card_out(self):
        s = snap([issue(3, ["planned"], idea_unlabelled_at="2026-02-01")], [card(3, "Idea")])
        self.assertEqual([a.kind for a in board_pass(s)], ["set_status"])

    def test_missing_optional_columns_fall_back(self):
        opts = ["Inbox", "Approved", "Blocked", "Nits", "Ready for Merge", "Done"]
        s = snap([issue(4, ["idea"]), issue(5, ["factory"])],
                 [card(4, "Inbox"), card(5, "Approved")],
                 prs=[{"number": 50, "head": "claude/issue-5-x", "status": ["Submitted", "running"]}],
                 meta={"options": opts, "waiting": True})
        s.board.append({"kind": "PullRequest", "number": 50, "state": "OPEN", "status": "Approved",
                        "status_updated": None, "waiting": None})
        self.assertEqual(board_pass(s), [])

    def test_waiting_text_and_second_run_empty(self):
        issues = [issue(6, ["planned"]),
                  issue(7, ["planned", "factory"], body_excerpt="**Depends on:** #6")]
        s = snap(issues, [card(6, "Inbox"), card(7, "Approved")])
        acts = board_pass(s)
        self.assertEqual([(a.kind, a.payload) for a in acts], [("set_waiting", {"text": "#6"})])
        tick(s, [board_pass], FakeBackend())
        self.assertEqual(board_pass(s), [])

    def test_closed_card_done_and_missing_card_added(self):
        s = snap([issue(8, ["factory"], state="closed"), issue(9, ["factory"])],
                 [card(8, "Approved", state="CLOSED")])
        acts = board_pass(s)
        self.assertEqual([(a.kind, a.target) for a in acts],
                         [("set_status", "issue#8"), ("add_card", "issue#9")])
        tick(s, [board_pass], FakeBackend())
        self.assertEqual(board_pass(s), [])


if __name__ == "__main__":
    unittest.main()
