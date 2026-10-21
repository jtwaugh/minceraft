"""Tests for the board pass. python3 -I test_factory_board.py"""
import copy
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from factory_board import board_pass  # noqa: E402
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


class Test(unittest.TestCase):
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

    def test_blocked_by_links(self):
        issues = [issue(10, ["planned"]),
                  issue(11, ["planned"], body_excerpt="**Depends on:** #10", blocked_by_links=[])]
        s = snap(issues, [])
        acts = [a for a in board_pass(s) if a.kind == "add_blocked_by"]
        self.assertEqual(len(acts), 1)
        tick(s, [board_pass], FakeBackend())
        self.assertEqual(board_pass(s), [])


if __name__ == "__main__":
    unittest.main()
