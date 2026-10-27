"""Tests for the tick's store pass. python3 -I test_factory_store_sync.py"""
import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
import factory_store_sync  # noqa: E402
import factory_tick  # noqa: E402
from factory_tick import FakeBackend, Snapshot, passes, tick  # noqa: E402

SRC = {"file": "docs/design/x.md", "section": "Phase 1"}


def issue(n, kind, parent=None, **extra):
    return {"number": n, "title": f"{kind} {n}", "state": "open", "labels": [], "kind": kind,
            "parent": parent, "body": "", **extra}


OLD = [issue(5, "plan", plan_source=SRC)]
NEW = OLD + [issue(9, "plan", plan_source=SRC), issue(10, "ticket", 9)]


class Test(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.db = os.path.join(self.dir.name, "store.db")
        self.fixture = os.path.join(self.dir.name, "f.json")

    def write(self, issues):
        json.dump({"issues": issues}, open(self.fixture, "w"))

    def run_main(self, *flags):
        env = {"FACTORY_STORE": self.db}
        with mock.patch.dict(os.environ, env), contextlib.redirect_stdout(io.StringIO()):
            return factory_tick._main(["tick", "--fixture", self.fixture, *flags])

    def dump(self):
        conn = factory_store.open(self.db)
        try:
            return factory_store.dump(conn)
        finally:
            conn.close()

    def live_tick(self, issues):
        """What a live tick does, minus GitHub: a writable store, publish at the end."""
        snap = Snapshot.from_dict({"issues": issues})
        snap.store, path = factory_store_sync.open_store(self.db, True)
        tick(snap, passes(), FakeBackend())
        published = mock.Mock(return_value=True)
        factory_store_sync.finish(snap, path, True, published)
        return published

    def test_unset_store_is_skipped(self):
        snap = Snapshot.from_dict({"issues": NEW})
        self.assertIsNone(snap.store)
        self.assertEqual(factory_store_sync.store_sync(snap), [])
        self.assertEqual(factory_store_sync.open_store(None, True), (None, None))
        self.assertFalse(factory_store_sync.finish(snap, None, True, mock.Mock()))

    def test_registered_after_invariants(self):
        names = [p.__name__ for p in passes()]
        self.assertEqual(names.index("store_sync"), names.index("invariants") + 1)

    def test_ingests_once(self):
        self.live_tick(OLD)  # records the cutoff
        before = self.dump()
        self.live_tick(NEW)
        after = self.dump()
        self.assertNotEqual(before, after)
        self.assertIn("10", after)
        published = self.live_tick(NEW)
        self.assertEqual(self.dump(), after)
        published.assert_called_once_with(self.db)

    def test_dry_run_leaves_store_alone(self):
        self.live_tick(OLD)
        before = open(self.db, "rb").read()
        self.write(NEW)
        with mock.patch("factory_state_branch.publish") as publish:
            self.run_main("--dry-run")
        self.assertEqual(open(self.db, "rb").read(), before)
        publish.assert_not_called()

    def test_dry_run_without_a_file_creates_none(self):
        self.write(NEW)
        self.run_main()
        self.assertFalse(os.path.exists(self.db))

    def test_crash_is_recorded_and_others_run(self):
        snap = Snapshot.from_dict({"issues": NEW})
        snap.store, _ = factory_store_sync.open_store(self.db, False)
        ran = []

        def store_sync(s):
            raise RuntimeError("boom")

        def after(s):
            ran.append(1)
            return []
        crashed = []
        tick(snap, [store_sync, after], FakeBackend(), crashed)
        self.assertEqual([n for n, _ in crashed], ["store_sync"])
        self.assertEqual(ran, [1])
        self.assertTrue(factory_tick.crashed_store(crashed))


if __name__ == "__main__":
    unittest.main()
