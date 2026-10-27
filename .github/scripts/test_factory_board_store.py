"""Tests for the board pass reading the store. python3 -I test_factory_board_store.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
from factory_board import board_pass  # noqa: E402
from factory_ingest import ingest  # noqa: E402
from factory_tick import Snapshot  # noqa: E402
from test_factory_board import card, issue  # noqa: E402

PLAN = {"number": 10, "title": "Plan: x", "state": "open", "labels": [], "kind": "plan"}
OLD = {"number": 1, "title": "Plan: old", "state": "closed", "labels": [], "kind": "plan"}


def stored_snap(issues, board):
    conn = factory_store.open(":memory:")
    ingest(conn, Snapshot.from_dict({"issues": [OLD], "plans": [OLD], "prs": []}))
    s = Snapshot.from_dict({"issues": [PLAN, *issues], "plans": [OLD, PLAN], "prs": [], "board": board})
    ingest(conn, s)
    conn.execute("UPDATE tickets SET state = 'approved' WHERE state = 'proposed'")
    s.board_meta = {}
    s.store = conn
    return s


def ticket(n, body="", **kw):
    return issue(n, ["planned", "factory"], body=body, parent=10, **kw)


def apply(s, actions):
    for a in actions:
        n = int(a.target.split("#")[1])
        c = next((c for c in s.board if c["number"] == n), None)
        if a.kind == "add_card":
            s.board.append(card(n, a.payload["status"], waiting=a.payload["waiting"] or None))
        elif a.kind == "set_status":
            c["status"] = a.payload["status"]
        elif a.kind == "set_waiting":
            c["waiting"] = a.payload["text"] or None


class Test(unittest.TestCase):
    def test_stored_ticket_matches_render(self):
        s = stored_snap([ticket(11), ticket(12, "**Depends on:** #11")],
                        [card(11, "Inbox"), card(12, "Approved")])
        got = {(a.target, a.kind): a.payload for a in board_pass(s)}
        self.assertEqual(got, {("issue#11", "set_status"): {"status": "Approved"},
                               ("issue#12", "set_waiting"): {"text": "#11"}})

    def test_stored_view_beats_edited_body(self):
        s = stored_snap([ticket(11), ticket(12, "**Depends on:** #11")], [card(11, "Approved"), card(12, "Approved", waiting="#11")])
        s.issues[2]["body"] = "**Depends on:** none"
        s.issues[2]["body_excerpt"] = ""
        self.assertEqual(board_pass(s), [])

    def test_unstored_ticket_uses_old_path(self):
        s = stored_snap([ticket(11)], [card(99, "Inbox")])
        s.issues.append(issue(99, ["planned", "factory"], body="**Depends on:** #11"))
        got = {(a.target, a.kind): a.payload for a in board_pass(s) if a.target == "issue#99"}
        self.assertEqual(got, {("issue#99", "set_waiting"): {"text": "#11"},
                               ("issue#99", "set_status"): {"status": "Approved"}})

    def test_second_pass_empty(self):
        s = stored_snap([ticket(11), ticket(12, "**Depends on:** #11")], [])
        first = board_pass(s)
        self.assertEqual([a.kind for a in first], ["add_card", "add_card"])
        apply(s, first)
        self.assertEqual(board_pass(s), [])


if __name__ == "__main__":
    unittest.main()
