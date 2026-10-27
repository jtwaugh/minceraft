"""Tests for the views rendered from the store. python3 -I test_factory_views.py"""
import copy
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
from factory_ingest import ingest  # noqa: E402
from factory_store_states import sync_states  # noqa: E402
from factory_tick import Snapshot  # noqa: E402
from factory_views import render  # noqa: E402

FIXTURE = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "store_views.json")))


class Test(unittest.TestCase):
    def build(self):
        d = tempfile.TemporaryDirectory()
        self.addCleanup(d.cleanup)
        db = factory_store.open(os.path.join(d.name, "s.db"))
        self.addCleanup(db.close)
        inp = FIXTURE["input"]
        # the first ingest sets the cutoff at plan #1, so only the later plan is stored
        ingest(db, Snapshot.from_dict({"issues": [i for i in inp["issues"] if i["number"] == 1]}))
        snap = Snapshot.from_dict(inp)
        snap.store = db
        ingest(db, snap)
        sync_states(snap, 1000.0)
        return db

    def test_snapshot_twice(self):
        db = self.build()
        self.assertEqual(render(db), FIXTURE["views"])
        self.assertEqual(render(db), FIXTURE["views"])

    def test_pure(self):
        db = self.build()
        before = factory_store.dump(db)
        render(db)
        self.assertEqual(factory_store.dump(db), before)

    def test_rebuilt_store_renders_the_same(self):
        self.assertEqual(render(self.build()), render(self.build()))

    def test_output_is_independent_of_the_store(self):
        db = self.build()
        out = render(db)
        saved = copy.deepcopy(out)
        out["tickets"]["12"]["labels"].append("x")
        self.assertEqual(render(db), saved)


if __name__ == "__main__":
    unittest.main()
