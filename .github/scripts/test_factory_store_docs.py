"""Tests for the docs mirror. python3 -I test_factory_store_docs.py"""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
from factory_store_sync import sync_docs  # noqa: E402

DOC = "---\ntitle: X\nstatus: accepted\nscope: both\nphases:\n  A: done\n  B: {b}\n---\n\nbody\n"


class Test(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.dir = os.path.join(self.tmp.name, "design")
        os.mkdir(self.dir)
        self.conn = factory_store.open(os.path.join(self.tmp.name, "s.db"))
        self.addCleanup(self.conn.close)

    def write(self, name, b="todo", status="accepted"):
        with open(os.path.join(self.dir, name), "w") as f:
            f.write(DOC.replace("\\\n", "\n").format(b=b).replace("accepted", status, 1))

    def rows(self):
        return (self.conn.execute("SELECT file, status FROM docs ORDER BY file").fetchall(),
                self.conn.execute("SELECT doc, name, state FROM phases ORDER BY doc, name").fetchall())

    def test_mirrors_header(self):
        self.write("x.md")
        open(os.path.join(self.dir, "README.md"), "w").write("no header\n")
        sync_docs(self.conn, self.dir)
        self.assertEqual(self.rows(), ([("docs/design/x.md", "accepted")],
                                       [("docs/design/x.md", "A", "done"), ("docs/design/x.md", "B", "todo")]))

    def test_phase_change_follows_header_and_doc_is_untouched(self):
        self.write("x.md")
        sync_docs(self.conn, self.dir)
        self.write("x.md", b="done")
        before = open(os.path.join(self.dir, "x.md")).read()
        sync_docs(self.conn, self.dir)
        self.assertIn(("docs/design/x.md", "B", "done"), self.rows()[1])
        self.assertEqual(open(os.path.join(self.dir, "x.md")).read(), before)

    def test_second_sync_changes_nothing(self):
        self.write("x.md")
        sync_docs(self.conn, self.dir)
        first = factory_store.dump(self.conn)
        sync_docs(self.conn, self.dir)
        self.assertEqual(factory_store.dump(self.conn), first)

    def test_vanished_doc_is_removed(self):
        self.write("x.md")
        sync_docs(self.conn, self.dir)
        os.remove(os.path.join(self.dir, "x.md"))
        sync_docs(self.conn, self.dir)
        self.assertEqual(self.rows(), ([], []))

    def test_vanished_doc_with_a_plan_becomes_unknown(self):
        self.write("x.md")
        sync_docs(self.conn, self.dir)
        self.conn.execute("INSERT INTO plans (id, doc) VALUES (5, 'docs/design/x.md')")
        os.remove(os.path.join(self.dir, "x.md"))
        sync_docs(self.conn, self.dir)
        self.assertEqual(self.rows(), ([("docs/design/x.md", "unknown")], []))


if __name__ == "__main__":
    unittest.main()
