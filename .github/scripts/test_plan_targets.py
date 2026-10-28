"""Tests for the planner's pick step. python3 -I test_plan_targets.py"""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from plan_targets import still_incomplete  # noqa: E402

DOC = ("---\nstatus: accepted\nphases:\n  B: todo\n---\n\n## Reconciliation notes\n\n"
       "0. **Phase B still incomplete (plan #1173).** The punctuation map is missing.\n")


class StillIncomplete(unittest.TestCase):
    def setUp(self):
        fd, self.path = tempfile.mkstemp(suffix=".md")
        with os.fdopen(fd, "w") as f:
            f.write(DOC)

    def tearDown(self):
        os.remove(self.path)

    def test_note_for_this_plan(self):
        self.assertTrue(still_incomplete(self.path, "Phase B", 1173))

    def test_note_for_an_older_plan_or_other_phase(self):
        self.assertFalse(still_incomplete(self.path, "Phase B", 1280))
        self.assertFalse(still_incomplete(self.path, "Phase C", 1173))

    def test_missing_doc(self):
        self.assertFalse(still_incomplete(self.path + ".gone", "Phase B", 1173))


if __name__ == "__main__":
    unittest.main()
