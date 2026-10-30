"""Replay a new plan through the full pass list with a store. python3 -I test_factory_store_replay.py"""
import functools
import os
import sqlite3
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import factory_store  # noqa: E402
import factory_store_sync  # noqa: E402
from factory_tick import FakeBackend, Snapshot, passes, tick  # noqa: E402

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "store_replay.json")
NOW = 1000.0
DOC = "---\ntitle: Store replay\nstatus: accepted\nphases:\n  1: todo\n---\n"

ACTIONS = {
    ("report", "issue#106"), ("create_issue", "new:stalled-work"), ("create_issue", "new:needs-you"),
    ("start_ticket", "issue#51"), ("start_ticket", "issue#105"), ("start_ticket", "issue#106"),
    ("add_label", "issue#107"), ("add_label", "issue#108"),
    ("add_card", "issue#51"), ("add_card", "issue#101"), ("add_card", "issue#102"), ("add_card", "issue#103"),
    ("add_card", "issue#105"), ("add_card", "issue#106"), ("add_card", "issue#107"),
}

DUMP = """schema_version 1
[attempts] run n outcome
[decisions] issue plan answer ticket
[dependencies] ticket depends_on
  101 | 102
  102 | 104
  103 | 104
[docs] file title status
  docs/design/store-replay.md |  | accepted
[meta] key value
  plan_cutoff | 50
[phases] doc name state
  docs/design/store-replay.md | 1 | todo
[plans] id doc phase state
  100 | docs/design/store-replay.md | Phase 1 | open
[predicted_files] ticket path
  101 | x.py
  102 | y.py
  103 | z.py
  104 | w.py
[runs] id ticket role
[tickets] id plan title state
  101 | 100 | Waiting ticket | waiting
  102 | 100 | In review ticket | in_review
  103 | 100 | Question ticket | blocked_on_question
  104 | 100 | Done ticket | done
[transitions] seq subject kind state reason at
  1 | 101 | transition | waiting | waiting on #102 | 1000.0
  2 | 102 | transition | in_review | PR #200 is open | 1000.0
  3 | 103 | transition | blocked_on_question | question #107 is open | 1000.0
  4 | 104 | transition | done | the issue is closed | 1000.0
  5 | 107 | needs_you | waiting | The factory asked you a question: answer it here | 1000.0
  6 | 108 | needs_you | waiting | The tick found stalled work: see the list | 1000.0
"""


class StoreReplay(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        design = os.path.join(tmp.name, "design")
        os.mkdir(design)
        open(os.path.join(design, "store-replay.md"), "w").write(DOC)
        for patch in (mock.patch.object(factory_store_sync, "sync_docs",
                                        functools.partial(factory_store_sync.sync_docs, design_dir=design)),
                      mock.patch("factory_store_sync.time.time", return_value=NOW),
                      mock.patch("factory_needs_you.clock", return_value=NOW)):
            patch.start()
            self.addCleanup(patch.stop)
        self.snap = Snapshot.from_fixture(FIXTURE)
        self.snap.store, _ = factory_store_sync.open_store(os.path.join(tmp.name, "store.db"), True)
        self.addCleanup(self.snap.store.close)
        # An earlier tick records the cutoff: the highest plan then existing.
        old = Snapshot.from_dict({"issues": [i for i in self.snap.issues if i["number"] <= 50]})
        old.store = self.snap.store
        tick(old, passes(), FakeBackend())

    def tick(self):
        return tick(self.snap, passes(), FakeBackend())

    def test_first_tick_acts_and_stores(self):
        self.assertEqual({(a.kind, a.target) for a in self.tick()}, ACTIONS)
        self.assertEqual(factory_store.dump(self.snap.store), DUMP)

    def test_second_tick_does_nothing(self):
        self.tick()
        before = factory_store.dump(self.snap.store)
        self.assertEqual(self.tick(), [])
        self.assertEqual(factory_store.dump(self.snap.store), before)

    def test_plan_older_than_the_cutoff_is_never_stored(self):
        self.tick()
        self.tick()
        s = self.snap.store
        self.assertEqual([r[0] for r in s.execute("SELECT id FROM plans")], [100])
        stored = {r[0] for r in s.execute("SELECT id FROM tickets")}
        self.assertEqual(stored, {101, 102, 103, 104})  # not #51, #105 (old plan) or #106 (no plan)
        self.assertEqual(s.execute("SELECT COUNT(*) FROM transitions WHERE subject IN (50, 51, 105, 106)")
                         .fetchone()[0], 0)

    def test_a_ticket_cannot_be_stored_without_a_plan(self):
        s = self.snap.store
        for plan in (None, 999):
            with self.assertRaises(sqlite3.IntegrityError):
                s.execute("INSERT INTO tickets (id, plan, title) VALUES (300, ?, 't')", (plan,))


if __name__ == "__main__":
    unittest.main()
