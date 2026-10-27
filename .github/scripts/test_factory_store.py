"""Tests for the state store. python3 -I test_factory_store.py"""
import os
import sqlite3
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402


class Test(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.path = os.path.join(self.dir.name, "store.db")
        self.db = factory_store.open(self.path)
        self.addCleanup(self.db.close)

    def test_create_and_reopen(self):
        self.assertEqual(factory_store.schema_version(self.db), factory_store.SCHEMA_VERSION)
        self.db.execute("INSERT INTO plans (id) VALUES (1)")
        self.db.commit()
        before = factory_store.dump(self.db)
        again = factory_store.open(self.path)
        self.addCleanup(again.close)
        self.assertEqual(factory_store.schema_version(again), factory_store.SCHEMA_VERSION)
        self.assertEqual(factory_store.dump(again), before)

    def test_newer_version_refused(self):
        self.db.execute("PRAGMA user_version = 99")
        self.db.commit()
        with self.assertRaises(RuntimeError):
            factory_store.open(self.path)

    def test_ticket_needs_existing_plan(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO tickets (id, plan) VALUES (10, NULL)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO tickets (id, plan) VALUES (10, 99)")
        self.db.execute("INSERT INTO plans (id) VALUES (1)")
        self.db.execute("INSERT INTO tickets (id, plan) VALUES (10, 1)")

    def test_dependencies(self):
        self.db.execute("INSERT INTO plans (id) VALUES (1)")
        self.db.executemany("INSERT INTO tickets (id, plan) VALUES (?, 1)", [(10,), (11,)])
        self.db.execute("INSERT INTO dependencies VALUES (11, 10)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO dependencies VALUES (11, 1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO dependencies VALUES (10, 10)")

    def test_id_is_plan_or_ticket(self):
        self.db.execute("INSERT INTO plans (id) VALUES (1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO tickets (id, plan) VALUES (1, 1)")
        self.db.execute("INSERT INTO tickets (id, plan) VALUES (10, 1)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO plans (id) VALUES (10)")

    def test_dump_ignores_insert_order(self):
        rows = [("INSERT INTO plans (id) VALUES (?)", [(1,), (2,)]),
                ("INSERT INTO tickets (id, plan, title) VALUES (?, 1, ?)", [(10, "a"), (11, "b"), (12, "")]),
                ("INSERT INTO dependencies VALUES (?, ?)", [(11, 10), (12, 11)])]
        dumps = []
        for i, order in enumerate((1, -1)):
            db = factory_store.open(os.path.join(self.dir.name, f"o{i}.db"))
            self.addCleanup(db.close)
            for sql, params in rows:
                db.executemany(sql, params[::order])
            dumps.append(factory_store.dump(db))
        self.assertEqual(dumps[0], dumps[1])
        self.assertIn("[tickets]", dumps[0])


if __name__ == "__main__":
    unittest.main()
