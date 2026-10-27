"""Tests for the ready pass reading the store. python3 -I test_factory_ready_store.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
from factory_ingest import ingest  # noqa: E402
from factory_ready import ready  # noqa: E402
from factory_tick import Snapshot  # noqa: E402


def issue(n, body="", state="open", labels=("factory",), **kw):
    return {"number": n, "title": f"T{n}", "state": state, "labels": list(labels),
            "kind": "ticket", "body": body, "parent": 10, **kw}


PLAN = {"number": 10, "title": "Plan: x", "state": "open", "labels": [], "kind": "plan"}
OLD = {"number": 1, "title": "Plan: old", "state": "closed", "labels": [], "kind": "plan"}


def stored_snap(issues, prs=()):
    """Ingest once (plan 10 is past the cutoff set by plan 1), then return a
    snapshot sharing the store."""
    conn = factory_store.open(":memory:")
    first = Snapshot.from_dict({"issues": [OLD], "plans": [OLD], "prs": []})
    ingest(conn, first)
    snap = Snapshot.from_dict({"issues": [PLAN, *issues], "plans": [OLD, PLAN], "prs": list(prs)})
    ingest(conn, snap)
    snap.store = conn
    return snap


def started(s):
    return [a.target for a in ready(s)]


class Test(unittest.TestCase):
    def test_open_dependency_blocks_until_closed(self):
        s = stored_snap([issue(11, labels=()), issue(12, "**Depends on:** #11")])
        self.assertEqual(started(s), [])
        s.issues[1]["state"] = "closed"
        self.assertEqual(started(s), ["issue#12"])

    def test_body_edit_ignored_until_reingest(self):
        s = stored_snap([issue(11, labels=()), issue(12, "**Depends on:** #11")])
        s.issues[2]["body"] = "**Depends on:** none"
        self.assertEqual(started(s), [])
        ingest(s.store, s)
        self.assertEqual(started(s), ["issue#12"])

    def test_stored_files_feed_overlap(self):
        pr = {"number": 9, "head": "other", "files": ["crates/game/src/walk.rs"]}
        s = stored_snap([issue(11, "`walk.rs`"), issue(12, "`lib.rs`")], [pr])
        s.issues[1]["body"] = "`lib.rs`"  # not re-ingested: the stored file still wins
        self.assertEqual(started(s), ["issue#12"])

    def test_unstored_ticket_uses_body(self):
        s = stored_snap([issue(11, labels=()), issue(12, "**Depends on:** #11")])
        s.issues.append(issue(99, "**Depends on:** #11", parent=None))
        self.assertEqual(started(s), [])
        s.issues[1]["state"] = "closed"
        self.assertEqual(started(s), ["issue#12", "issue#99"])


if __name__ == "__main__":
    unittest.main()
