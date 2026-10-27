"""Tests for store ingest. python3 -I test_factory_ingest.py"""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
from factory_ingest import ingest  # noqa: E402
from factory_tick import Snapshot  # noqa: E402

SRC = {"file": "docs/design/x.md", "section": "Phase 1"}


def plan(n):
    return {"number": n, "title": f"Plan: {n}", "state": "open", "labels": [],
            "kind": "plan", "parent": None, "plan_source": SRC, "body": ""}


def ticket(n, parent, body="", title="t"):
    return {"number": n, "title": title, "state": "open", "labels": [],
            "kind": "ticket", "parent": parent, "body": body}


def snap(*issues):
    return Snapshot.from_dict({"issues": list(issues)})


class Test(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.db = factory_store.open(os.path.join(self.dir.name, "s.db"))
        self.addCleanup(self.db.close)

    def ids(self, table):
        return [r[0] for r in self.db.execute(f"SELECT id FROM {table} ORDER BY id")]

    def first(self):
        ingest(self.db, snap(plan(5), ticket(6, 5)))  # records the cutoff (5)

    def test_cutoff(self):
        self.first()
        self.assertEqual(self.ids("plans"), [])
        self.assertEqual(self.ids("tickets"), [])
        ingest(self.db, snap(plan(5), ticket(6, 5), plan(9), ticket(10, 9), ticket(11, 9)))
        self.assertEqual(self.ids("plans"), [9])
        self.assertEqual(self.ids("tickets"), [10, 11])

    def test_orphan_not_stored(self):
        self.first()
        ingest(self.db, snap(plan(9), ticket(10, None), ticket(11, 77)))
        self.assertEqual(self.ids("tickets"), [])

    def test_plan_dependency_dropped(self):
        self.first()
        ingest(self.db, snap(plan(9), ticket(10, 9), ticket(11, 9, "**Depends on:** #9, #10, #3")))
        rows = self.db.execute("SELECT * FROM dependencies").fetchall()
        self.assertEqual(rows, [(11, 10)])

    def test_files_and_plan_source(self):
        self.first()
        ingest(self.db, snap(plan(9), ticket(10, 9, "**Files:** `crates/x/a.py` (new)")))
        self.assertEqual(self.db.execute("SELECT path FROM predicted_files").fetchall(),
                         [("crates/x/a.py",)])
        self.assertEqual(self.db.execute("SELECT doc, phase FROM plans").fetchall(),
                         [(SRC["file"], SRC["section"])])

    def test_idempotent(self):
        self.first()
        s = snap(plan(9), ticket(10, 9), ticket(11, 9, "**Depends on:** #10"))
        ingest(self.db, s)
        before = factory_store.dump(self.db)
        ingest(self.db, s)
        self.assertEqual(factory_store.dump(self.db), before)

    def test_edits_update(self):
        self.first()
        ingest(self.db, snap(plan(9), ticket(10, 9), ticket(11, 9, "**Depends on:** #10")))
        self.db.execute("UPDATE tickets SET state = 'ready' WHERE id = 11")
        ingest(self.db, snap(plan(9), ticket(10, 9), ticket(12, 9),
                             ticket(11, 9, "**Depends on:** #12", title="new")))
        self.assertEqual(self.db.execute("SELECT title, state FROM tickets WHERE id = 11").fetchone(),
                         ("new", "ready"))
        self.assertEqual(self.db.execute("SELECT * FROM dependencies").fetchall(), [(11, 12)])


if __name__ == "__main__":
    unittest.main()
